//! The app itself (`crates/app`, through `lumenna-ffi`), over this device's store: what the
//! screens draw, and where they send what the person does.
//!
//! What is in hand, what is folded, which question is open and what to say are the app's; a
//! screen draws `view` with `lines` and sends events, and decides none of them. Every turn the
//! app leaves comes in on a thread of the app's own and is handed to GTK's main thread, in
//! order (`window::take`): drawn, then said.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use gtk::glib;
use lumenna_app::lines::Lines;
use lumenna_app::{Capitals, ViewModel};
use lumenna_ffi::{App, Device, Locale, Shell, ShellEvent, Turn};
use lumenna_records::{Action, LumennaError};

/// The app over this device's store, and what it last left to draw.
pub struct Model {
    /// The store, for what runs without the screens: reminders, backups.
    pub device: Arc<Device>,
    app: App,
    view: RefCell<ViewModel>,
    lines: RefCell<Lines>,
    seq: Cell<u32>,
}

impl Model {
    /// Starts the app over the profile in `directory`, saying in pairing that it runs Linux.
    pub fn open(directory: &std::path::Path, clock: &crate::clock::Locale) -> Result<Self, LumennaError> {
        let device = Arc::new(Device::new(directory.display().to_string())?);
        let writing = Arc::new(Writing { twelve: Arc::clone(&clock.twelve) });
        let app = App::new(&device, "linux".to_owned(), writing, Arc::new(Relay))?;
        let view = app.view();
        let lines = app.lines();
        app.send(ShellEvent::Clock(clock.style()));
        // Title Case, as GNOME writes buttons and menus.
        app.send(ShellEvent::Capitals(Capitals::Title));
        app.send(ShellEvent::PairingAs { called: "this computer".to_owned(), nearby: true });
        app.send(ShellEvent::PlanFrom(crate::clock::pinned_now()));
        Ok(Self { device, app, view: RefCell::new(view), lines: RefCell::new(lines), seq: Cell::new(0) })
    }

    /// Sends what the person did.
    pub fn send(&self, event: ShellEvent) {
        self.app.send(event);
    }

    /// What to draw: the view model as the last turn left it.
    pub fn view(&self) -> std::cell::Ref<'_, ViewModel> {
        self.view.borrow()
    }

    /// The view's lines, in this desktop's clock.
    pub fn lines(&self) -> std::cell::Ref<'_, Lines> {
        self.lines.borrow()
    }

    /// The view as the app holds it this moment, ahead of any turn still on its way: for what
    /// must not act on a turn made before the person's last answer.
    pub fn current(&self) -> (ViewModel, Lines) {
        (self.app.view(), self.app.lines())
    }

    /// Whether the app is still: nothing with the store, and every turn handed over.
    pub fn is_still(&self) -> bool {
        self.app.is_still()
    }

    /// Serves the command surface at the profile's address whenever this device's sync runs
    /// here, as the daemon would, so Emacs and the BTSpeak app reach the resident app.
    pub fn serve_commands(&self) {
        self.app.serve_commands("Lumenna for Linux".to_owned(), glib::host_name().to_string());
    }

    /// Takes the automatic backup if one is due, off the main thread, and says so only if it
    /// failed.
    pub fn back_up_if_due(&self) {
        let device = Arc::clone(&self.device);
        std::thread::spawn(move || {
            if let Err(error) = device.back_up_if_due() {
                let said = format!("The automatic backup failed. {}", error.message());
                glib::MainContext::default().invoke(move || {
                    if let Some(app) = crate::window::app() {
                        app.say(&said);
                    }
                });
            }
        });
    }

    /// Whether the app keeps the device in sync: it does only while in front.
    pub fn in_front(&self, in_front: bool) {
        self.app.in_front(in_front);
    }

    /// Takes in a change made elsewhere, or a new minute.
    pub fn tick(&self) {
        self.app.tick();
    }

    /// What could go at `cursor` (UTF-8 bytes) in a line of `syntax` outside the app's own
    /// quick add: the filter. At keystroke rate, so straight from the store.
    pub fn complete(&self, text: String, cursor: u32, syntax: lumenna_records::Syntax) -> Result<lumenna_records::Completions, LumennaError> {
        self.app.complete(text, cursor, syntax)
    }

    /// Takes a turn, unless a later one is already in: whether to draw it.
    pub fn take(&self, turn: &Turn) -> bool {
        if turn.seq <= self.seq.get() {
            return false;
        }
        self.seq.set(turn.seq);
        *self.view.borrow_mut() = turn.view.clone();
        *self.lines.borrow_mut() = turn.lines.clone();
        true
    }

    /// What running one of a row's actions sends: the row in hand, then the action.
    pub fn run(&self, action: Action, on: &str) {
        self.send(ShellEvent::Focus(on.to_owned()));
        self.send(ShellEvent::Perform(action));
    }
}

/// The app's turns, from its own thread onto GTK's main one, in order.
struct Relay;

impl Shell for Relay {
    fn turn(&self, turn: Turn) {
        glib::MainContext::default().invoke(move || crate::window::take(turn));
    }
}

/// This desktop's way of writing a time and a date, for the lines the core words. "Today",
/// "Tomorrow" and when to give the year are the core's.
///
/// Called on the app's own thread, so it holds the desktop's clock setting as a number the
/// main thread keeps current (`clock::Locale::twelve`), not the settings object.
struct Writing {
    twelve: Arc<AtomicU8>,
}

impl Locale for Writing {
    fn time(&self, hour: i8, minute: i8) -> String {
        crate::clock::time(i32::from(hour), i32::from(minute), crate::clock::twelve_from(self.twelve.load(Ordering::Relaxed)))
    }

    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
        let format = if with_year { "%A %-d %B %Y" } else { "%A %-d %B" };
        glib::DateTime::from_local(i32::from(year), i32::from(month), i32::from(day), 12, 0, 0.0)
            .and_then(|date| date.format(format))
            .map_or_else(|_| format!("{year}-{month:02}-{day:02}"), |text| text.to_string())
    }
}
