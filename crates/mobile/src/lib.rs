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

mod events;
mod words;

use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use boltffi::{data, export};
use crux_core::middleware::Layer;
use lumenna_app::{Effect, Notice, SaveFile, ViewModel};
use lumenna_bridge::network::{Network, NetworkHandle};
use lumenna_bridge::{Shell as Left, Stack, Store, shell, stack};
use lumenna_host::{Lumenna, Reach};
use lumenna_records::{Completions, LumennaError, Syntax};

pub use events::ShellEvent;

/// Something to say.
#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saying {
    /// The sentence; for a heading, the day's summary.
    pub text: String,
    /// Whether to say the day's heading, "<day>. <summary>", in the shell's own words for the
    /// day.
    pub heading: bool,
}

/// What the app left for the shell: what to draw, and what to say once focus is where the view
/// puts it.
#[data]
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// What to draw.
    pub view: ViewModel,
    /// What to say, in order.
    pub said: Vec<Saying>,
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
}

#[export]
impl App {
    /// Opens the profile in `directory` and starts the app over it: it watches the store, and
    /// every turn goes to `shell`. `platform` is what this device says it runs, in pairing.
    ///
    /// # Errors
    ///
    /// If the profile cannot be opened.
    pub fn new(directory: String, platform: String, shell: Arc<dyn Shell>) -> Result<App, LumennaError> {
        let lumenna = Lumenna::open(&directory)?;
        let (store, worker) = Store::new(Arc::clone(&lumenna));
        spawn("lumenna-store", move || futures::executor::block_on(worker.run()))?;
        let network = Network::new(Arc::clone(&lumenna), store.clone(), Reach::Internet, platform);
        let handle = network.handle();
        let stack = Arc::new(stack(&store, network));
        let (outbox, delivering) = mpsc::channel();
        let drawing = Arc::clone(&stack);
        spawn("lumenna-shell", move || deliver(&delivering, &drawing, &*shell))?;
        let app = App { lumenna, stack, store, network: handle, outbox: Mutex::new(outbox) };
        app.send(ShellEvent::Start);
        Ok(app)
    }

    /// Runs what the person did. Its turns reach the shell as every turn does.
    pub fn send(&self, event: ShellEvent) {
        let outbox = self.outbox.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let later = outbox.clone();
        let now = self.stack.update(event.into(), move |effects| {
            let _ = later.send(effects);
        });
        let _ = outbox.send(now);
    }

    /// What to draw now.
    pub fn view(&self) -> ViewModel {
        self.stack.view()
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
fn deliver(outbox: &mpsc::Receiver<Vec<Effect>>, stack: &Stack<Network>, shell: &dyn Shell) {
    let mut seq = 0;
    while let Ok(first) = outbox.recv() {
        let mut effects = first;
        while let Ok(more) = outbox.try_recv() {
            effects.extend(more);
        }
        let left = self::shell(effects);
        if left.is_empty() {
            continue;
        }
        seq += 1;
        let mut turn = Turn { view: stack.view(), said: Vec::new(), files: Vec::new(), notices: Vec::new(), seq };
        for one in left {
            match one {
                Left::Announce(text) => turn.said.push(Saying { text, heading: false }),
                Left::Heading(text) => turn.said.push(Saying { text, heading: true }),
                Left::Save(file) => turn.files.push(file),
                Left::Notify(notice) => turn.notices.push(notice),
                Left::Render => {}
            }
        }
        shell.turn(turn);
    }
}
