//! The app itself, beside the store: events in, the view model and what to say out.
//!
//! The page sends what the person did; the app runs it over the bridge's middleware, which
//! answers what it asks of the store on a task of its own here in the worker, and what reaches
//! other devices — syncing, pairing — is this module's middleware, on the browser's event loop.
//! Whatever the app leaves for the page, at once or after an answer, goes out one way: through
//! the outbox, turned into a [`Turn`] and handed to the listener the page gave. Nothing about
//! which row is in hand, what is folded or which question is open is the page's.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crux_core::middleware::{EffectMiddleware, EffectResolver, Layer};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures::channel::oneshot;
use lumenna_app::{Effect, Event, NetworkAnswer, NetworkRequest, Notice, SaveFile, ViewModel};
use lumenna_bridge::{Shell, Stack, Store, shell, stack};
use lumenna_host::{Lumenna, Reach, SyncLoop};
use lumenna_records::words::spoken;
use serde::{Deserialize, Serialize};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, spawn_local};

use lumenna_desktop::lines::{Lines, lines};

use crate::clock::CLOCK;
use crate::{Core, Out, js};

/// What this browser's sync is doing: the loop while it runs, and whether a pairing in
/// progress has been given up.
#[derive(Default)]
pub(crate) struct Syncing {
    pub(crate) running: RefCell<Option<SyncLoop>>,
    pub(crate) cancelled: Cell<bool>,
}

/// What the app left for the page: the view model, and what to say once focus is where it
/// puts it.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
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
    listener: RefCell<Option<js_sys::Function>>,
    /// Who waits for the app to be still.
    waiting: RefCell<Vec<oneshot::Sender<()>>>,
    /// The last turn's number.
    seq: Cell<u32>,
}

/// The app over the store this worker holds.
#[wasm_bindgen]
pub struct App {
    inner: Rc<Inner>,
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
        let view = self.stack.view();
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
                    Some(day) => lumenna_desktop::speech::summary(&day.date, text, &CLOCK),
                    None => text.clone(),
                }),
                Shell::Save(file) => turn.files.push(file.clone()),
                Shell::Notify(notice) => turn.notices.push(notice.clone()),
                Shell::Render => {}
            }
        }
        let listener = self.listener.borrow().clone();
        if let (Some(listener), Ok(turn)) = (listener, js(&turn)) {
            let _ = listener.call1(&JsValue::NULL, &turn.into());
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

#[wasm_bindgen]
impl Core {
    /// The app, started over this store. It has the store once [`App::settled`] answers; what it
    /// leaves before the page listens is not handed on, since the page draws the view it finds.
    pub fn app(&self) -> App {
        let (store, worker) = Store::new(Arc::clone(&self.lumenna));
        spawn_local(worker.run());
        let (requests, asked) = unbounded();
        let runner = Rc::new(Runner {
            lumenna: Arc::clone(&self.lumenna),
            store: store.clone(),
            syncing: Rc::clone(&self.syncing),
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
        App { inner }
    }
}

#[wasm_bindgen]
impl App {
    /// Runs what the person did. Every turn it leaves goes to the listener; the answer is the
    /// view once the app is still, for a field that keeps its own value while typing is on its
    /// way.
    pub fn send(&self, event: Ts<Event>) -> Result<js_sys::Promise, JsError> {
        let event = event.to_rust()?;
        self.inner.send(event);
        Ok(self.reply())
    }

    /// Takes in what changed elsewhere since: another tab, `lum`, a sync, a write made through
    /// the store's own exports, or a new minute. What it leaves goes to the listener.
    pub fn tick(&self) -> js_sys::Promise {
        self.inner.store.tick();
        self.reply()
    }

    /// Answers once the app is still: everything sent so far answered and handed on.
    pub fn settled(&self) -> js_sys::Promise {
        let settled = self.inner.settled();
        future_to_promise(async move {
            settled.await;
            Ok(JsValue::UNDEFINED)
        })
    }

    /// What to draw now, with its lines, as a turn with nothing to say.
    pub fn now(&self) -> Out<Turn> {
        js(&self.inner.current())
    }

    /// Gives every turn from now on to `listener`.
    pub fn listen(&self, listener: js_sys::Function) {
        *self.inner.listener.borrow_mut() = Some(listener);
    }

    fn reply(&self) -> js_sys::Promise {
        let settled = self.inner.settled();
        let inner = Rc::clone(&self.inner);
        future_to_promise(async move {
            settled.await;
            Ok(js(&inner.current())?.into())
        })
    }
}
