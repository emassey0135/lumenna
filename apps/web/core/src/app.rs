//! The app itself, beside the store: events in, the view model and what to say out.
//!
//! The page sends what the person did; the app runs it over the bridge's middleware, which
//! answers what it asks of the store on a task of its own here in the worker, and what reaches
//! other devices — syncing, pairing — is this module's middleware, on the browser's event loop.
//! Whatever the app leaves for the page, at once or after an answer, goes out one way: through
//! the outbox, turned into a [`Turn`] and handed to the [`Listener`] the page gave. Nothing
//! about which row is in hand, what is folded or which question is open is the page's.
//!
//! Exported through BoltFFI, typed as Android's are. What only a browser holds — the store's
//! `Rc` state, its promises — is not `Send`, which BoltFFI asks of every export; [`Local`] says
//! it is, which a browser's one thread makes true.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use boltffi::{data, export};
use lumenna_records::{Completions, Syntax};

use crux_core::middleware::{EffectMiddleware, EffectResolver, Layer};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::channel::oneshot;
use lumenna_app::{Effect, Event, NetworkAnswer, NetworkRequest, Notice, SaveFile, ShellEvent, ViewModel};
use lumenna_bridge::{Shell, Stack, Store, shell, stack};
use lumenna_host::{Lumenna, Reach, SyncLoop};
use lumenna_records::LumennaError;
use lumenna_records::words::spoken;
use wasm_bindgen_futures::spawn_local;

use lumenna_app::lines::{Lines, lines};

use crate::clock::CLOCK;
/// What only a browser holds, said to be `Send` and `Sync` for BoltFFI.
pub(crate) struct Local<T>(pub(crate) T);

// SAFETY: wasm32-unknown-unknown, built without the atomics feature, has exactly one thread, so
// nothing can move or share a value to another. The web core runs nowhere else: the workspace
// compiles it natively to check it, and nothing runs that build.
unsafe impl<T> Send for Local<T> {}
// SAFETY: as for `Send`.
unsafe impl<T> Sync for Local<T> {}

impl<F: Future> Future for Local<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        // SAFETY: the future inside is never moved out of its pin.
        unsafe { self.map_unchecked_mut(|local| &mut local.0) }.poll(cx)
    }
}

/// The page, as the app reaches it: every turn, one at a time, in order.
#[export]
pub trait Listener: Send + Sync {
    /// A turn: draw it, then say it.
    fn turn(&self, turn: Turn);
}

/// What this browser's sync is doing: the loop while it runs, and whether a pairing in
/// progress has been given up.
#[derive(Default)]
pub(crate) struct Syncing {
    pub(crate) running: RefCell<Option<SyncLoop>>,
    pub(crate) cancelled: Cell<bool>,
}

/// A backup for a folder: what it is called there, and the whole of it.
#[data]
#[derive(Debug, Clone, PartialEq)]
pub struct FolderBackup {
    /// `lumenna-<when>.lumbak`.
    pub name: String,
    /// Every document's history, the trash included.
    pub bytes: Vec<u8>,
}

/// What the app left for the page: the view model, and what to say once focus is where it
/// puts it.
#[data]
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    /// What to draw.
    pub view: ViewModel,
    /// The view's lines, in this browser's clock.
    pub lines: Lines,
    /// What to say, in order.
    pub said: Vec<String>,
    /// Files to save where the person keeps them: a backup, an export.
    pub files: Vec<SaveFile>,
    /// Notifications to show, as the browser shows them.
    pub notices: Vec<Notice>,
    /// Counted: an older screen is never drawn over a newer one.
    pub seq: u32,
}

/// The network's requests, handed from the middleware to the runner on the event loop: the
/// middleware must be `Send`, and what syncs here is not.
struct Network(UnboundedSender<(NetworkRequest, EffectResolver<NetworkAnswer>)>);

impl EffectMiddleware for Network {
    type Op = NetworkRequest;

    fn try_process_effect(&self, request: NetworkRequest, resolver: EffectResolver<NetworkAnswer>) {
        let _ = self.0.unbounded_send((request, resolver));
    }
}

/// What reaches the outbox: what the app left, or word that the store has nothing under way.
enum Outgoing {
    Effects(Vec<Effect>),
    Still,
}

/// What the app, the outbox and the runner share.
struct Inner {
    stack: Stack<Network>,
    store: Store,
    outbox: UnboundedSender<Outgoing>,
    /// The page's listener, for every turn.
    listener: RefCell<Option<Arc<dyn Listener>>>,
    /// Who waits for the app to be still.
    waiting: RefCell<Vec<oneshot::Sender<()>>>,
    /// The last turn's number.
    seq: Cell<u32>,
}

/// The app over the store this worker holds.
pub struct App {
    lumenna: Arc<Lumenna>,
    inner: Local<Rc<Inner>>,
}

impl Inner {
    /// Where what the app leaves later goes: into the outbox, in the order it was left.
    fn later(&self) -> impl Fn(Vec<Effect>) + Send + Sync + 'static {
        let outbox = self.outbox.clone();
        move |effects| {
            let _ = outbox.unbounded_send(Outgoing::Effects(effects));
        }
    }

    /// Runs `event`; what it leaves at once goes out after anything left before it.
    fn send(&self, event: Event) {
        let now = self.stack.update(event, self.later());
        let _ = self.outbox.unbounded_send(Outgoing::Effects(now));
    }

    /// Once the store has nothing under way and the outbox has handed on everything before.
    fn settled(&self) -> impl Future<Output = ()> + use<> {
        let (still, settled) = oneshot::channel();
        self.waiting.borrow_mut().push(still);
        let _ = self.outbox.unbounded_send(Outgoing::Still);
        async move {
            let _ = settled.await;
        }
    }

    /// The view as it stands, as a turn with nothing to say: what a reply carries.
    fn current(&self) -> Turn {
        let view = self.stack.view().screen(0);
        Turn { lines: lines(&view, &CLOCK), view, said: Vec::new(), files: Vec::new(), notices: Vec::new(), seq: self.seq.get() }
    }

    /// Hands what the outbox holds to the page, a turn per batch, until the app is gone.
    async fn deliver(self: Rc<Self>, mut outbox: UnboundedReceiver<Outgoing>) {
        while let Some(out) = outbox.next().await {
            match out {
                Outgoing::Effects(effects) => self.push(&shell(effects)),
                Outgoing::Still => {
                    if self.store.busy().is_still() {
                        for still in self.waiting.take() {
                            let _ = still.send(());
                        }
                    }
                }
            }
        }
    }

    fn push(&self, shell: &[Shell]) {
        if shell.is_empty() {
            return;
        }
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let mut turn = Turn { seq, ..self.current() };
        for s in shell {
            match s {
                Shell::Announce(text) => turn.said.push(text.clone()),
                // "<day>. <summary>", for a day newly shown.
                Shell::Heading(text) => turn.said.push(match &turn.view.day {
                    Some(day) => lumenna_app::speech::summary(&day.date, text, &CLOCK),
                    None => text.clone(),
                }),
                Shell::Save(file) => turn.files.push(file.clone()),
                Shell::Notify(notice) => turn.notices.push(notice.clone()),
                // Never asked here: a browser keeps nothing running once its tab is closed, so the
                // web offers no background setting.
                Shell::Background(_) | Shell::Render => {}
            }
        }
        let listener = self.listener.borrow().clone();
        if let Some(listener) = listener {
            listener.turn(turn);
        }
    }
}

/// Answers the network's requests on the browser's event loop.
struct Runner {
    lumenna: Arc<Lumenna>,
    store: Store,
    syncing: Rc<Syncing>,
    /// The pairing's question about the words, while it waits for the person.
    confirm: RefCell<Option<oneshot::Sender<bool>>>,
}

impl Runner {
    /// Takes each request as it comes, each on a task of its own: a pairing waits for minutes,
    /// and the answer about its words comes in meanwhile.
    async fn run(self: Rc<Self>, mut requests: UnboundedReceiver<(NetworkRequest, EffectResolver<NetworkAnswer>)>) {
        while let Some((request, resolver)) = requests.next().await {
            let runner = Rc::clone(&self);
            spawn_local(async move { runner.answer(request, resolver).await });
        }
    }

    async fn answer(self: Rc<Self>, request: NetworkRequest, mut resolver: EffectResolver<NetworkAnswer>) {
        let answer = match request {
            NetworkRequest::StartSync => self.start_sync().await,
            NetworkRequest::SyncNow => self.sync_now().await,
            NetworkRequest::Status => {
                let running = self.syncing.running.borrow().as_ref().is_some_and(SyncLoop::is_running);
                match self.lumenna.sync_status_with(running) {
                    Ok(status) => NetworkAnswer::Said(spoken(&status.announcement, &status.notices)),
                    Err(error) => NetworkAnswer::Said(error.message().to_owned()),
                }
            }
            NetworkRequest::Pair { code, name } => {
                // A pairing answers more than once: its code, its words, how it ended.
                let resolver = Rc::new(RefCell::new(resolver));
                let answer = self.pair(Rc::clone(&resolver), code, name).await;
                resolver.borrow_mut().resolve(answer);
                return;
            }
            NetworkRequest::Confirm(matched) => {
                if let Some(answer) = self.confirm.borrow_mut().take() {
                    let _ = answer.send(matched);
                }
                NetworkAnswer::Done
            }
            NetworkRequest::CancelPairing => {
                self.syncing.cancelled.set(true);
                NetworkAnswer::Done
            }
        };
        resolver.resolve(answer);
    }

    async fn start_sync(&self) -> NetworkAnswer {
        if self.syncing.running.borrow().as_ref().is_some_and(SyncLoop::is_running) {
            return NetworkAnswer::Done;
        }
        let store = self.store.clone();
        // What arrives is in the store already: the app takes it in.
        let started = self.lumenna.keep_in_sync(Reach::Internet, move || store.tick()).await;
        match started {
            Ok((handle, looping)) => {
                *self.syncing.running.borrow_mut() = Some(handle);
                spawn_local(looping);
                NetworkAnswer::Done
            }
            Err(error) => NetworkAnswer::Said(error.message().to_owned()),
        }
    }

    async fn sync_now(&self) -> NetworkAnswer {
        let running = self.syncing.running.borrow().clone();
        let Some(handle) = running.filter(SyncLoop::is_running) else {
            return NetworkAnswer::Said("Sync is not running.".to_owned());
        };
        match handle.sync_now().await {
            Ok(report) => {
                let failures: Vec<String> =
                    report.peers.iter().filter_map(|p| p.error.as_ref().map(|e| format!("{}: {e}", p.name))).collect();
                NetworkAnswer::Said(spoken(&report.announcement, &failures))
            }
            Err(error) => NetworkAnswer::Said(error.message().to_owned()),
        }
    }

    async fn pair(
        self: &Rc<Self>,
        resolver: Rc<RefCell<EffectResolver<NetworkAnswer>>>,
        code: Option<String>,
        name: String,
    ) -> NetworkAnswer {
        self.syncing.cancelled.set(false);
        let showing = Rc::clone(&resolver);
        let asking = Rc::clone(self);
        let watching = Rc::clone(&self.syncing);
        let paired = self
            .lumenna
            .pair_async(
                code,
                Reach::Internet,
                name,
                "web".to_owned(),
                move |code| showing.borrow_mut().resolve(NetworkAnswer::Code(code)),
                async move {
                    while !watching.cancelled.get() {
                        n0_future::time::sleep(Duration::from_millis(250)).await;
                    }
                },
                move |words| async move {
                    let (answer, answered) = oneshot::channel();
                    *asking.confirm.borrow_mut() = Some(answer);
                    resolver.borrow_mut().resolve(NetworkAnswer::Words(words));
                    answered.await.unwrap_or(false)
                },
            )
            .await;
        NetworkAnswer::Paired(match paired {
            Ok(paired) => Ok(spoken(&paired.announcement, &paired.notices)),
            Err(error) => Err(error.message().to_owned()),
        })
    }
}

/// Opens the profile called `name`, creating it on first use — SQLite in OPFS, durable across
/// reloads — and starts the app over it. The app has the store once [`App::settled`] answers;
/// what it leaves before the page listens is not handed on, since the page draws what it finds.
///
/// # Errors
///
/// If the browser's storage, or the profile, cannot be opened.
#[export]
pub async fn open(name: String) -> Result<App, LumennaError> {
    Local(async move {
        let lumenna = Arc::new(crate::open_store(&name).await?);
        Ok(App { inner: Local(start(&lumenna)), lumenna })
    })
    .await
}

/// The app, started over `lumenna`.
fn start(lumenna: &Arc<Lumenna>) -> Rc<Inner> {
    let (store, worker) = Store::new(Arc::clone(lumenna));
    spawn_local(worker.run());
    let (requests, asked) = unbounded();
    let runner = Rc::new(Runner {
        lumenna: Arc::clone(lumenna),
        store: store.clone(),
        syncing: Rc::default(),
        confirm: RefCell::new(None),
    });
    spawn_local(runner.run(asked));
    let (outbox, delivering) = unbounded();
    let still = outbox.clone();
    store.busy().when_still(move || {
        let _ = still.unbounded_send(Outgoing::Still);
    });
    let inner = Rc::new(Inner {
        stack: stack(&store, Network(requests)),
        store,
        outbox,
        listener: RefCell::new(None),
        waiting: RefCell::new(Vec::new()),
        seq: Cell::new(0),
    });
    spawn_local(Rc::clone(&inner).deliver(delivering));
    inner.send(Event::Start);
    inner
}

#[export]
impl App {
    /// Runs what the person did. Every turn it leaves goes to the listener; the answer is the
    /// view once the app is still, for a field that keeps its own value while typing is on its
    /// way.
    pub async fn send(&self, event: ShellEvent) -> Turn {
        self.inner.0.send(event.into());
        self.reply().await
    }

    /// Takes in what changed elsewhere since: another tab, `lum`, a sync, or a new minute. What
    /// it leaves goes to the listener.
    pub async fn tick(&self) -> Turn {
        self.inner.0.store.tick();
        self.reply().await
    }

    /// Answers once the app is still: everything sent so far answered and handed on.
    pub async fn settled(&self) {
        Local(self.inner.0.settled()).await;
    }

    /// What to draw now, with its lines, as a turn with nothing to say.
    pub fn now(&self) -> Turn {
        self.inner.0.current()
    }

    /// Gives every turn from now on to `listener`.
    pub fn listen(&self, listener: Arc<dyn Listener>) {
        *self.inner.0.listener.borrow_mut() = Some(listener);
    }

    /// What could go at `cursor` (UTF-8 bytes) in `text` written in `syntax`: a keystroke-rate
    /// question of the store, as every app's filter field asks it.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn complete(&self, text: String, cursor: u32, syntax: Syntax) -> Result<Completions, LumennaError> {
        self.lumenna.complete_text(&text, cursor, syntax)
    }

    /// A backup taken now, for a folder the person gave this browser: its name and bytes.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn backup_file(&self) -> Result<FolderBackup, LumennaError> {
        let file = self.lumenna.backup_file()?;
        Ok(FolderBackup { name: file.name, bytes: file.bytes })
    }

    /// Whether a backup is due in a folder holding the files `names`, at `now_ms`: the core's
    /// naming and the default every day.
    pub fn backup_due(&self, names: Vec<String>, now_ms: f64) -> bool {
        // A JavaScript time is milliseconds, exactly.
        #[allow(clippy::cast_possible_truncation)]
        let now = jiff::Timestamp::from_millisecond(now_ms as i64).unwrap_or_else(|_| jiff::Timestamp::now());
        lumenna_host::folder_backup_due(&names, now)
    }

    /// Which of the files `names` in a backup folder to remove: the oldest backups beyond the
    /// number kept, never anything else.
    pub fn backups_to_remove(&self, names: Vec<String>) -> Vec<String> {
        lumenna_host::folder_backup_surplus(&names)
    }

    /// Every backup among the files `names` in a backup folder: what to delete there once the
    /// store has moved on to another generation, since each holds what was erased.
    pub fn folder_backups(&self, names: Vec<String>) -> Vec<String> {
        lumenna_host::folder_backups(&names)
    }

    /// Which generation of the store's history this browser holds, empty for the first: a real
    /// erasure, here or taken in from another device, begins another. The folder's backups are
    /// replaced when it is not the one they were taken of, as every other device replaces its
    /// own.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn generation(&self) -> Result<String, LumennaError> {
        self.lumenna.generation()
    }

    /// A number that moves whenever another tab or process has written.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn outside_version(&self) -> Result<f64, LumennaError> {
        // A JavaScript number holds this exactly: it is SQLite's data_version, a counter.
        Ok(self.lumenna.outside_version()? as f64)
    }
}

impl App {
    /// The view once the app is still.
    fn reply(&self) -> impl Future<Output = Turn> + Send + use<'_> {
        let settled = Local(self.inner.0.settled());
        async move {
            settled.await;
            self.inner.0.current()
        }
    }
}
