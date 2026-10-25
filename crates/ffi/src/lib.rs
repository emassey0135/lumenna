//! The Crux app for the Kotlin apps, through BoltFFI: Android's phone and Wear OS, and the
//! Apple apps at their turn.
//!
//! An [`App`] is the app over one profile, run beside its store as the web runs it: the store's
//! middleware from the bridge on a thread of its own, the network's natively on threads. The
//! shell sends what the person did ([`App::send`]) and gets every turn the app leaves — the view
//! model, what to say, files to save, notifications to show — through its [`Shell`], one at a
//! time, in order, from a thread of the app's own.
//!
//! The records cross typed, by BoltFFI's own encoding: no serialisation of ours, so the serde
//! shapes `lum --json` and the RPC surface keep are no concern here.

mod device;
#[cfg(not(feature = "network"))]
mod offline;
#[cfg(feature = "rpc")]
pub mod rpc;
pub mod words;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use boltffi::{data, export};
// Imported, not named by its path: BoltFFI finds another crate's type only so.
use lumenna_app::glance::GlanceEntry;
use lumenna_app::keys::KeyGroup;
use crux_core::middleware::Layer;
use lumenna_app::{Effect, Notice, SaveFile, ViewModel};
#[cfg(feature = "network")]
use lumenna_bridge::network::{Network, NetworkHandle};
#[cfg(not(feature = "network"))]
use offline::{Network, NetworkHandle};
use lumenna_bridge::{Stack, Store, stack};
use lumenna_host::{Lumenna, Reach};
use lumenna_app::lines::{Lines, lines};
use lumenna_app::speech::{self, Clock, Spoken};
use lumenna_records::assistant::AssistantOperation;
use lumenna_records::launch::LaunchShortcut;
use lumenna_records::{Completions, LumennaError, Syntax};

pub use device::Device;
#[cfg(feature = "network")]
pub use device::LinkStream;
pub use lumenna_app::ShellEvent;
/// Where this device's store lives, by the same rule lum follows, for a shell that finds it
/// before opening it: the Windows app's.
pub use lumenna_host::profile;

/// What the shell's locale writes: a time of day and a date. Saying a day — "Today",
/// "Tomorrow" — and joining a line are the core's.
#[export]
pub trait Locale: Send + Sync {
    /// A time of day as this device writes one: "2:30 PM", or "14:30".
    fn time(&self, hour: i8, minute: i8) -> String;
    /// A date with its weekday and month named, the year too when `with_year`.
    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String;
}

/// The shell's [`Locale`] as the desktop crate's wording reads it.
struct Formatted(Arc<dyn Locale>);

impl speech::Formats for Formatted {
    fn time(&self, hour: i8, minute: i8) -> String {
        self.0.time(hour, minute)
    }

    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
        self.0.date(year, month, day, with_year)
    }
}

/// What the app left for one screen: what to draw, and what to say once focus is where the
/// view puts it.
#[data]
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// The screen it is for: 0, a shell's first, unless it opened others (`App::send_on`).
    pub screen: u32,
    /// What to draw.
    pub view: ViewModel,
    /// The view's lines, in this device's clock.
    pub lines: Lines,
    /// What to say, in order.
    pub said: Vec<String>,
    /// Files to save where the person keeps them: a backup, an export.
    pub files: Vec<SaveFile>,
    /// Notifications to show, as the platform shows them.
    pub notices: Vec<Notice>,
    /// Keep Lumenna running in the background, or stop: what the person chose in Settings, for
    /// the shell to do and answer with `BackgroundEvent`.
    pub background: Option<bool>,
    /// Counted, from one: each turn draws over the one before.
    pub seq: u32,
}

/// The shell, as the app reaches it.
#[export]
pub trait Shell: Send + Sync {
    /// A turn: draw it, then say it. Called from a thread of the app's own, one turn at a time,
    /// in order.
    fn turn(&self, turn: Turn);
}

/// The app over one profile.
pub struct App {
    lumenna: Arc<Lumenna>,
    stack: Arc<Stack<Network>>,
    store: Store,
    network: NetworkHandle,
    outbox: Mutex<mpsc::Sender<Vec<Effect>>>,
    /// What the app has left that the shell has not been handed yet, in batches.
    unsent: Arc<AtomicUsize>,
    /// How this device says times and days.
    clock: Arc<Spoken<Formatted>>,
    /// What this device says it runs, in pairing.
    #[cfg_attr(not(feature = "rpc"), allow(dead_code))]
    platform: String,
}

#[export]
impl App {
    /// Starts the app over `device`'s store: it watches the store, and every turn goes to
    /// `shell`. `platform` is what this device says it runs, in pairing.
    ///
    /// # Errors
    ///
    /// If a thread of the app's cannot be started.
    pub fn new(device: &Device, platform: String, locale: Arc<dyn Locale>, shell: Arc<dyn Shell>) -> Result<App, LumennaError> {
        let lumenna = Arc::clone(&device.lumenna);
        let (store, worker) = Store::new(Arc::clone(&lumenna));
        spawn("lumenna-store", move || futures::executor::block_on(worker.run()))?;
        let network = Network::new(Arc::clone(&lumenna), store.clone(), Reach::Internet, platform.clone());
        let handle = network.handle();
        let stack = Arc::new(stack(&store, network));
        let (outbox, delivering) = mpsc::channel();
        let drawing = Arc::clone(&stack);
        let unsent = Arc::new(AtomicUsize::new(0));
        let handed = Arc::clone(&unsent);
        let clock = Arc::new(Spoken(Formatted(locale)));
        let saying = Arc::clone(&clock);
        spawn("lumenna-shell", move || deliver(&delivering, &drawing, &*shell, &handed, &*saying))?;
        let app = App { lumenna, stack, store, network: handle, outbox: Mutex::new(outbox), unsent, clock, platform };
        app.send(ShellEvent::Start);
        Ok(app)
    }

    /// Runs what the person did, on screen 0. Its turns reach the shell as every turn does.
    pub fn send(&self, event: ShellEvent) {
        self.update(event.into());
    }

    /// Runs what the person did on screen `screen`: a window, an Emacs buffer. A screen opens
    /// with the first event on it, over the store as the others have it.
    pub fn send_on(&self, screen: u32, event: ShellEvent) {
        self.update(lumenna_app::Event::On(screen, Box::new(event.into())));
    }

    /// Closes screen `screen`: what it asked for that comes back afterwards goes nowhere.
    pub fn close_screen(&self, screen: u32) {
        self.update(lumenna_app::Event::CloseScreen(screen));
    }

    /// What to draw now.
    pub fn view(&self) -> ViewModel {
        self.view_on(0)
    }

    /// What screen `screen` shows now.
    pub fn view_on(&self, screen: u32) -> ViewModel {
        self.stack.view().screen(screen)
    }

    /// The lines of what to draw now, in this device's clock.
    pub fn lines(&self) -> Lines {
        self.lines_on(0)
    }

    /// The lines of what screen `screen` shows now, in this device's clock.
    pub fn lines_on(&self, screen: u32) -> Lines {
        lines(&self.view_on(screen), &*self.clock)
    }

    /// What a widget, complication or tile shows for the next `hours` (at most two days),
    /// worded in this device's clock: the moment now, then one wherever that changes, each
    /// whole, for the platform's widget system to be handed as a timeline.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn glance(&self, hours: u32) -> Result<Vec<GlanceEntry>, LumennaError> {
        Ok(self.lumenna.glance(hours)?.into_iter().map(|moment| lumenna_app::glance::worded(moment, &*self.clock)).collect())
    }
}

impl App {
    fn update(&self, event: lumenna_app::Event) {
        let outbox = self.outbox.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let later = outbox.clone();
        let counting = Arc::clone(&self.unsent);
        // Counted before the app runs, so the app is never still between its answer and the
        // batch it leaves reaching the outbox.
        self.unsent.fetch_add(1, Ordering::SeqCst);
        let now = self.stack.update(event, move |effects| {
            counting.fetch_add(1, Ordering::SeqCst);
            if later.send(effects).is_err() {
                counting.fetch_sub(1, Ordering::SeqCst);
            }
        });
        if outbox.send(now).is_err() {
            self.unsent.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[export]
impl App {
    /// Whether the app is still: nothing with the store, and every turn handed to the shell.
    /// A shell's tests wait on it, as a person waits for the screen to settle; a pairing or a
    /// sync under way does not count, since those wait on other devices.
    pub fn is_still(&self) -> bool {
        self.store.busy().is_still() && self.unsent.load(Ordering::SeqCst) == 0
    }

    /// Takes in what changed elsewhere — another process, a sync — or a new minute. The shell
    /// calls this on a timer while it is in front, and when told the store changed.
    pub fn tick(&self) {
        self.store.tick();
    }

    /// What could go at `cursor` (UTF-8 bytes) in a line of `syntax` the shell has open outside
    /// the app's own quick add: a filter. At keystroke rate, so straight from the store.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn complete(&self, text: String, cursor: u32, syntax: Syntax) -> Result<Completions, LumennaError> {
        self.lumenna.complete_text(&text, cursor, syntax)
    }

    /// The app came to the front, or left it: syncing runs only while it is in front.
    pub fn in_front(&self, in_front: bool) {
        self.network.in_front(in_front);
    }
}

#[cfg(feature = "rpc")]
#[export]
impl App {
    /// Serves the command surface at the profile's address whenever this device's sync runs
    /// here, as the daemon would, so Emacs and the BTSpeak app reach a resident app: `app` is
    /// what a client is told is answering ("Lumenna for Mac"), `device_name` what a pairing
    /// asked for over it calls this device.
    pub fn serve_commands(&self, app: String, device_name: String) {
        let apps = Arc::new(rpc::Apps { platform: self.platform.clone(), keeps_in_sync: None });
        self.network.serve(lumenna_bridge::network::Serving { app, device_name, apps: Some(apps) });
    }
}

/// The keyboard commands every desktop app shares (`lumenna_app::keys`), grouped as their
/// menus are, for a keyboard on Android: each command's keys and its name, in sentence case,
/// so Android binds and lists the keys Windows, GTK and the web do.
#[export]
pub fn keyboard_shortcuts() -> Vec<KeyGroup> {
    lumenna_app::keys::grouped(true)
}

/// Every operation an assistant can do (`lumenna_records::assistant`), each with its names,
/// description, category, parameters and what it does to the store: what Siri's and
/// Shortcuts' App Intents and Gemini's App Functions are declared from and checked against, so
/// they offer what `lum mcp` does, in the same words. Run one with `Device::assist`.
#[export]
pub fn assistant_operations() -> Vec<AssistantOperation> {
    lumenna_records::assistant::operations()
}

/// The ways into Lumenna from outside it (`lumenna_records::launch`): the icon's shortcuts,
/// the Dock's menu, a Jump List. A platform lists these in this order and these words, and
/// hands the one chosen back as `ShellEvent::Launch`.
#[export]
pub fn launch_shortcuts() -> Vec<LaunchShortcut> {
    lumenna_records::launch::launch_shortcuts()
}

fn spawn(name: &str, run: impl FnOnce() + Send + 'static) -> Result<(), LumennaError> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(run)
        .map(|_| ())
        .map_err(|error| LumennaError::new(error.to_string()))
}

/// What a batch left one screen: what it heard (a heading, or a sentence), files, notices, and
/// keeping Lumenna running in the background as last asked.
type Left = (Vec<(bool, String)>, Vec<SaveFile>, Vec<Notice>, Option<bool>);

/// Hands the shell a turn for whatever the app has left, until the app is gone: one per screen
/// that has something new, a view it has not drawn or something to say or save. What arrived
/// together goes as one turn, so a burst of answers draws each screen once.
fn deliver(outbox: &mpsc::Receiver<Vec<Effect>>, stack: &Stack<Network>, shell: &dyn Shell, unsent: &AtomicUsize, clock: &dyn Clock) {
    let mut seq = 0;
    // What each screen was last handed, so one a batch left as it was is not drawn again.
    let mut drawn: BTreeMap<u32, ViewModel> = BTreeMap::new();
    while let Ok(first) = outbox.recv() {
        let mut effects = first;
        let mut batches = 1;
        while let Ok(more) = outbox.try_recv() {
            effects.extend(more);
            batches += 1;
        }
        let mut views = stack.view().0;
        drawn.retain(|screen, _| views.contains_key(screen));
        let mut left: BTreeMap<u32, Left> = BTreeMap::new();
        let mut draw = false;
        for effect in effects {
            match effect {
                Effect::Render(_) => draw = true,
                Effect::Announce(request) => {
                    let said = request.operation;
                    left.entry(said.screen).or_default().0.push((said.heading, said.text));
                }
                Effect::Save(request) => left.entry(request.operation.screen).or_default().1.push(request.operation),
                // The device's, shown once: with the first screen's turn.
                Effect::Notify(request) => {
                    let first = views.keys().next().copied().unwrap_or(0);
                    left.entry(first).or_default().2.push(request.operation);
                }
                Effect::Background(request) => left.entry(request.operation.screen).or_default().3 = Some(request.operation.on),
                Effect::Watch(_) | Effect::Host(_) | Effect::Network(_) => {}
            }
        }
        if draw {
            for (screen, view) in &views {
                if drawn.get(screen) != Some(view) {
                    left.entry(*screen).or_default();
                }
            }
        }
        for (screen, (heard, files, notices, background)) in left {
            let view = views.remove(&screen).unwrap_or_default();
            // "<day>. <summary>", for a day newly shown.
            let said = heard
                .into_iter()
                .map(|(heading, text)| match (&view.day, heading) {
                    (Some(day), true) => speech::summary(&day.date, &text, clock),
                    _ => text,
                })
                .collect();
            seq += 1;
            drawn.insert(screen, view.clone());
            shell.turn(Turn { screen, lines: lines(&view, clock), view, said, files, notices, background, seq });
        }
        // Handed on: the shell counts it from here until it is drawn.
        unsent.fetch_sub(batches, Ordering::SeqCst);
    }
}
