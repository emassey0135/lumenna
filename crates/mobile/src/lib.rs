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
mod words;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use boltffi::{data, export};
use crux_core::middleware::Layer;
use lumenna_app::{Effect, Notice, SaveFile, ViewModel};
use lumenna_bridge::network::{Network, NetworkHandle};
use lumenna_bridge::{Shell as Left, Stack, Store, shell, stack};
use lumenna_host::{Lumenna, Reach};
use lumenna_desktop::lines::{Lines, lines};
use lumenna_desktop::speech::{self, Clock, Spoken};
use lumenna_records::{Completions, LumennaError, Syntax};

pub use device::{Device, LinkStream};
pub use lumenna_app::ShellEvent;

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

/// What the app left for the shell: what to draw, and what to say once focus is where the view
/// puts it.
#[data]
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
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
        let network = Network::new(Arc::clone(&lumenna), store.clone(), Reach::Internet, platform);
        let handle = network.handle();
        let stack = Arc::new(stack(&store, network));
        let (outbox, delivering) = mpsc::channel();
        let drawing = Arc::clone(&stack);
        let unsent = Arc::new(AtomicUsize::new(0));
        let handed = Arc::clone(&unsent);
        let clock = Arc::new(Spoken(Formatted(locale)));
        let saying = Arc::clone(&clock);
        spawn("lumenna-shell", move || deliver(&delivering, &drawing, &*shell, &handed, &*saying))?;
        let app = App { lumenna, stack, store, network: handle, outbox: Mutex::new(outbox), unsent, clock };
        app.send(ShellEvent::Start);
        Ok(app)
    }

    /// Runs what the person did. Its turns reach the shell as every turn does.
    pub fn send(&self, event: ShellEvent) {
        let outbox = self.outbox.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let later = outbox.clone();
        let counting = Arc::clone(&self.unsent);
        // Counted before the app runs, so the app is never still between its answer and the
        // batch it leaves reaching the outbox.
        self.unsent.fetch_add(1, Ordering::SeqCst);
        let now = self.stack.update(event.into(), move |effects| {
            counting.fetch_add(1, Ordering::SeqCst);
            if later.send(effects).is_err() {
                counting.fetch_sub(1, Ordering::SeqCst);
            }
        });
        if outbox.send(now).is_err() {
            self.unsent.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Whether the app is still: nothing with the store, and every turn handed to the shell.
    /// A shell's tests wait on it, as a person waits for the screen to settle; a pairing or a
    /// sync under way does not count, since those wait on other devices.
    pub fn is_still(&self) -> bool {
        self.store.busy().is_still() && self.unsent.load(Ordering::SeqCst) == 0
    }

    /// What to draw now.
    pub fn view(&self) -> ViewModel {
        self.stack.view()
    }

    /// The lines of what to draw now, in this device's clock.
    pub fn lines(&self) -> Lines {
        lines(&self.stack.view(), &*self.clock)
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

fn spawn(name: &str, run: impl FnOnce() + Send + 'static) -> Result<(), LumennaError> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(run)
        .map(|_| ())
        .map_err(|error| LumennaError::new(error.to_string()))
}

/// Hands the shell a turn for whatever the app has left, until the app is gone. What arrived
/// together goes as one turn, so a burst of answers draws the view once.
fn deliver(outbox: &mpsc::Receiver<Vec<Effect>>, stack: &Stack<Network>, shell: &dyn Shell, unsent: &AtomicUsize, clock: &dyn Clock) {
    let mut seq = 0;
    while let Ok(first) = outbox.recv() {
        let mut effects = first;
        let mut batches = 1;
        while let Ok(more) = outbox.try_recv() {
            effects.extend(more);
            batches += 1;
        }
        let left = self::shell(effects);
        if left.is_empty() {
            unsent.fetch_sub(batches, Ordering::SeqCst);
            continue;
        }
        seq += 1;
        let view = stack.view();
        let mut turn = Turn { lines: lines(&view, clock), view, said: Vec::new(), files: Vec::new(), notices: Vec::new(), seq };
        for one in left {
            match one {
                Left::Announce(text) => turn.said.push(text),
                // "<day>. <summary>", for a day newly shown.
                Left::Heading(text) => turn.said.push(match &turn.view.day {
                    Some(day) => speech::summary(&day.date, &text, clock),
                    None => text,
                }),
                Left::Save(file) => turn.files.push(file),
                Left::Notify(notice) => turn.notices.push(notice),
                Left::Render => {}
            }
        }
        shell.turn(turn);
        // Handed on: the shell counts it from here until it is drawn.
        unsent.fetch_sub(batches, Ordering::SeqCst);
    }
}
