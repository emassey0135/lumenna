//! The icon in the tray, through StatusNotifierItem — GTK 4 has no status icon of its
//! own — by way of `ksni`.
//!
//! Never the only way back to the window, since Orca reaches a tray item poorly: the
//! shortcut from anywhere and starting the app again do the same. GNOME shows such items only
//! with an extension; without one there is nothing to show it, and the item waits quietly
//! for a host to appear rather than failing.
//!
//! Its menu is the core's (`keys::tray`), and the timer's control (`Device::timer_control`)
//! beside it, as the phones' Quick Settings tile and Control Center have it: Start Timer on what
//! to work on now, or Stop Timer on the one running.

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use ksni::TrayMethods;
use ksni::menu::{MenuItem, StandardItem};
use lumenna_records::controls::TimerControl;

use crate::core::{Event, post};

/// How often the timer's control is read again: a timer started elsewhere, a minute gone by.
const READ_EVERY: u32 = 2;

struct Tray {
    /// The core's items, a separator's id empty.
    items: Vec<lumenna_app::keys::MenuItem>,
    /// The timer's control, once read.
    timer: Option<TimerControl>,
}

thread_local! {
    static HANDLE: RefCell<Option<ksni::Handle<Tray>>> = const { RefCell::new(None) };
    /// The control as last given to the tray, so an unchanged one is not sent again.
    static SHOWN: RefCell<Option<TimerControl>> = const { RefCell::new(None) };
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        crate::ID.to_owned()
    }

    fn title(&self) -> String {
        "Lumenna".to_owned()
    }

    fn icon_name(&self) -> String {
        crate::ID.to_owned()
    }

    /// The checkout's icons, for a build run without `install.sh`.
    fn icon_theme_path(&self) -> String {
        crate::icons_here().unwrap_or_default().to_owned()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    /// A click on the icon shows the window.
    fn activate(&mut self, _x: i32, _y: i32) {
        post(Event::Command("show".to_owned()));
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut menu = Vec::new();
        let mut timer = self.timer.as_ref().map(timer_item);
        for item in &self.items {
            if item.id.is_empty() {
                // The timer's control goes with the commands, before the first separator.
                menu.extend(timer.take());
                menu.push(MenuItem::Separator);
                continue;
            }
            let id = item.id.clone();
            menu.push(
                StandardItem {
                    label: crate::menus::label(&item.id, &item.title, item.asks),
                    activate: Box::new(move |_: &mut Self| post(Event::Command(id.clone()))),
                    ..Default::default()
                }
                .into(),
            );
        }
        menu.extend(timer);
        menu
    }
}

/// The timer's control as an item: what a press does, and what it is about ("Stop Timer,
/// Write the report"), as the core's lines join a title and its detail; greyed where a press
/// would do nothing.
fn timer_item(control: &TimerControl) -> MenuItem<Tray> {
    let text = if control.detail.is_empty() { control.action.clone() } else { format!("{}, {}", control.action, control.detail) };
    StandardItem {
        label: crate::menus::label("timer", &text, false),
        enabled: control.enabled,
        activate: Box::new(|_: &mut Tray| post(Event::ToggleTimer)),
        ..Default::default()
    }
    .into()
}

/// Puts the icon in the tray, for as long as the app runs, and keeps its timer control current.
pub fn start() {
    gtk::glib::spawn_future_local(async {
        let tray = Tray { items: lumenna_app::keys::tray(false), timer: None };
        // A tray host that is not there yet is waited for, not an error. A sandbox owns no
        // well-known name it was not given, so there the item is served by its connection alone.
        if let Ok(handle) = tray.assume_sni_available(true).disable_dbus_name(crate::sandboxed()).spawn().await {
            HANDLE.set(Some(handle));
            refresh();
        }
    });
    let reading = std::rc::Rc::new(Cell::new(false));
    gtk::glib::timeout_add_seconds_local(READ_EVERY, move || {
        if !reading.replace(true) {
            let reading = std::rc::Rc::clone(&reading);
            gtk::glib::spawn_future_local(async move {
                read().await;
                reading.set(false);
            });
        }
        gtk::glib::ControlFlow::Continue
    });
}

/// Reads the timer's control again now, as after it was pressed.
fn refresh() {
    gtk::glib::spawn_future_local(read());
}

/// Reads the timer's control off the main thread, and gives it to the tray if it changed.
async fn read() {
    let Some(device) = crate::window::app().map(|app| Arc::clone(&app.model.device)) else { return };
    if HANDLE.with_borrow(Option::is_none) {
        return;
    }
    let Ok(control) = crate::off_main(move || device.timer_control()).await else { return };
    if SHOWN.with_borrow(|shown| shown.as_ref() == Some(&control)) {
        return;
    }
    SHOWN.set(Some(control.clone()));
    let Some(handle) = HANDLE.with_borrow(Clone::clone) else { return };
    handle.update(move |tray| tray.timer = Some(control)).await;
}

/// The timer's control pressed: stops the timer running, else starts on what to work on now,
/// and says what it did, or why not.
pub fn toggle_timer(app: &crate::window::App) {
    let device = Arc::clone(&app.model.device);
    gtk::glib::spawn_future_local(async move {
        let done = crate::off_main(move || device.toggle_timer()).await;
        let Some(app) = crate::window::app() else { return };
        match done {
            Ok(change) => {
                app.model.tick();
                app.say(&lumenna_records::words::spoken(&change.announcement, &change.notices));
            }
            Err(error) => app.fail(error.message()),
        }
        refresh();
    });
}
