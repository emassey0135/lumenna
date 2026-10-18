//! The app itself, beside the store: events in, the view model and what to say out.
//!
//! The page sends what the person did; the session runs it, answering whatever it asks of the
//! store here in the worker, and hands back what to draw and what to say. Nothing about which
//! row is in hand, what is folded or which question is open is the page's.
//!
//! What reaches other devices — syncing, pairing — is answered later, on the browser's event
//! loop (the runner here): each answer runs the app again, and what it leaves is pushed to the
//! page through the listener it gave.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use lumenna_app::{Event, NetworkAnswer, NetworkRequest, Notice, SaveFile, ViewModel};
use lumenna_bridge::{Session, Shell};
use lumenna_records::words::spoken;
use lumenna_host::{Lumenna, Reach, SyncLoop};
use serde::{Deserialize, Serialize};
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use crate::{Core, Out, js};

/// What this browser's sync is doing: the loop while it runs, and whether a pairing in
/// progress has been given up.
#[derive(Default)]
pub(crate) struct Syncing {
    pub(crate) running: RefCell<Option<SyncLoop>>,
    pub(crate) cancelled: std::cell::Cell<bool>,
}

/// Something to say.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
pub struct Saying {
    /// The sentence; for a heading, the day's summary.
    pub text: String,
    /// Whether to say the day's heading, "<day>. <summary>", in this browser's words for the day.
    pub heading: bool,
}

/// What an event left for the page: the view model, and what to say once focus is where it
/// puts it.
#[derive(Debug, Clone, Serialize, Deserialize, Tsify)]
pub struct Turn {
    /// What to draw.
    pub view: ViewModel,
    /// What to say, in order.
    pub said: Vec<Saying>,
    /// Files to save where the person keeps them: a backup, an export.
    pub files: Vec<SaveFile>,
    /// Notifications to show, as the browser shows them.
    pub notices: Vec<Notice>,
    /// Counted: a reply and a push can reach the page in either order, and an older screen is
    /// never drawn over a newer one.
    pub seq: u32,
}

/// What the app and its runner share: the session, the store, and how syncing is going.
struct Inner {
    session: RefCell<Session>,
    lumenna: std::sync::Arc<Lumenna>,
    syncing: Rc<Syncing>,
    /// The page's listener, for what an answer from the network leaves.
    listener: RefCell<Option<js_sys::Function>>,
    /// The pairing's question about the words, while it waits for the person.
    confirm: RefCell<Option<futures::channel::oneshot::Sender<bool>>>,
    /// What starting the app asked, held until the page listens.
    start: RefCell<Vec<Shell>>,
    /// The last turn's number.
    seq: std::cell::Cell<u32>,
}

/// The app over the store this worker holds.
#[wasm_bindgen]
pub struct App {
    inner: Rc<Inner>,
}

fn turn(session: &Session, shell: &[Shell], seq: u32) -> Turn {
    let mut turn = Turn { view: session.view(), said: Vec::new(), files: Vec::new(), notices: Vec::new(), seq };
    for s in shell {
        match s {
            Shell::Announce(text) => turn.said.push(Saying { text: text.clone(), heading: false }),
            Shell::Heading(text) => turn.said.push(Saying { text: text.clone(), heading: true }),
            Shell::Save(file) => turn.files.push(file.clone()),
            Shell::Notify(notice) => turn.notices.push(notice.clone()),
            Shell::Render | Shell::Network { .. } => {}
        }
    }
    turn
}

impl Inner {
    /// Hands the network what `shell` asks of it, and makes the turn it leaves.
    fn handle(self: &Rc<Self>, shell: &[Shell]) -> Turn {
        for s in shell {
            if let Shell::Network { id, request } = s {
                let inner = Rc::clone(self);
                let (id, request) = (*id, request.clone());
                wasm_bindgen_futures::spawn_local(async move { inner.run(id, request).await });
            }
        }
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        turn(&self.session.borrow(), shell, seq)
    }

    /// Answers network request `id`, and pushes what it leaves to the page.
    fn answer(self: &Rc<Self>, id: u64, answer: NetworkAnswer) {
        let shell = self.session.borrow_mut().answer_network(id, answer);
        self.push(&shell);
    }

    /// Pushes what `shell` leaves to the page, if anything.
    fn push(self: &Rc<Self>, shell: &[Shell]) {
        if shell.is_empty() {
            return;
        }
        let turn = self.handle(shell);
        let listener = self.listener.borrow().clone();
        if let (Some(listener), Ok(turn)) = (listener, js(&turn)) {
            let _ = listener.call1(&JsValue::NULL, &turn.into());
        }
    }

    /// What changed elsewhere since: a sync's arrivals.
    fn tick(self: &Rc<Self>) {
        let shell = self.session.borrow_mut().tick();
        self.push(&shell);
    }

    async fn run(self: Rc<Self>, id: u64, request: NetworkRequest) {
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
            NetworkRequest::Pair { code, name } => self.pair(id, code, name).await,
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
        self.answer(id, answer);
    }

    async fn start_sync(self: &Rc<Self>) -> NetworkAnswer {
        if self.syncing.running.borrow().as_ref().is_some_and(SyncLoop::is_running) {
            return NetworkAnswer::Done;
        }
        let weak = Rc::downgrade(self);
        let started = self
            .lumenna
            .keep_in_sync(Reach::Internet, move || {
                // What arrives is in the store already: the app takes it in.
                if let Some(inner) = weak.upgrade() {
                    inner.tick();
                }
            })
            .await;
        match started {
            Ok((handle, looping)) => {
                *self.syncing.running.borrow_mut() = Some(handle);
                wasm_bindgen_futures::spawn_local(looping);
                NetworkAnswer::Done
            }
            Err(error) => NetworkAnswer::Said(error.message().to_owned()),
        }
    }

    async fn sync_now(self: &Rc<Self>) -> NetworkAnswer {
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

    async fn pair(self: &Rc<Self>, id: u64, code: Option<String>, name: String) -> NetworkAnswer {
        self.syncing.cancelled.set(false);
        let showing = Rc::clone(self);
        let asking = Rc::clone(self);
        let watching = Rc::clone(&self.syncing);
        let paired = self
            .lumenna
            .pair_async(
                code,
                Reach::Internet,
                name,
                "web".to_owned(),
                move |code| showing.answer(id, NetworkAnswer::Code(code)),
                async move {
                    while !watching.cancelled.get() {
                        n0_future::time::sleep(Duration::from_millis(250)).await;
                    }
                },
                move |words| async move {
                    let (answer, answered) = futures::channel::oneshot::channel();
                    *asking.confirm.borrow_mut() = Some(answer);
                    asking.answer(id, NetworkAnswer::Words(words));
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
    /// The app, started over this store: it already has the store and shows the place it
    /// starts in. What starting it asks of the network goes to the runner once a listener is
    /// given.
    pub fn app(&self) -> App {
        let (session, start) = Session::new(std::sync::Arc::clone(&self.lumenna));
        App {
            inner: Rc::new(Inner {
                session: RefCell::new(session),
                lumenna: std::sync::Arc::clone(&self.lumenna),
                syncing: Rc::clone(&self.syncing),
                listener: RefCell::new(None),
                confirm: RefCell::new(None),
                start: RefCell::new(start),
                seq: std::cell::Cell::new(0),
            }),
        }
    }
}

#[wasm_bindgen]
impl App {
    /// Runs what the person did.
    pub fn send(&self, event: Ts<Event>) -> Out<Turn> {
        let event = event.to_rust()?;
        let shell = self.inner.session.borrow_mut().send(event);
        js(&self.inner.handle(&shell))
    }

    /// Takes in what changed elsewhere since: another tab, `lum`, a sync, a write made through
    /// the store's own exports, or a new minute. Nothing when nothing did.
    pub fn tick(&self) -> Result<Option<Ts<Turn>>, JsError> {
        let shell = self.inner.session.borrow_mut().tick();
        if shell.is_empty() {
            return Ok(None);
        }
        Ok(Some(js(&self.inner.handle(&shell))?))
    }

    /// What to draw now.
    pub fn view(&self) -> Out<ViewModel> {
        js(&self.inner.session.borrow().view())
    }

    /// Gives what an answer from the network leaves to `listener`, a turn at a time; and
    /// begins what starting the app asked of the network, such as keeping in sync.
    pub fn listen(&self, listener: js_sys::Function) {
        *self.inner.listener.borrow_mut() = Some(listener);
        let start = std::mem::take(&mut *self.inner.start.borrow_mut());
        self.inner.push(&start);
    }
}
