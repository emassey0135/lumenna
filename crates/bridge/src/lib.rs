//! The app beside its store: the app's store effects answered by the host, as Crux middleware.
//!
//! The app asks two things of the store: to watch it ([`Watch`], a stream answered at once and
//! whenever the store moves) and to write or read what only the host can answer
//! ([`HostRequest`]). Two middlewares take them, [`Watching`] and [`Writing`], and hand both to
//! one [`StoreWorker`], which answers them in order off the caller's thread: a thread of its
//! own natively, a task on the event loop in a browser. What reaches other devices is a third
//! middleware, the platform's ([`stack`] takes it), since syncing runs on each platform's own
//! runtime. What is left after all three is the shell's: draw, say, save a file, show a
//! notification ([`Shell`]).
//!
//! [`answer`] is the whole of what the host does for the app. [`Busy`] counts the store's
//! work under way, so a shell or a test can wait until the app is still. No shell waits for it
//! on the thread it draws on: each draws what the app leaves as it arrives.

use std::sync::{Arc, Condvar, Mutex, PoisonError};

use crux_core::Core;
use crux_core::middleware::{EffectMiddleware, EffectResolver, HandleEffectLayer, Layer};
use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use jiff::Zoned;
use lumenna_app::proposal::{Applied, Proposal};
use lumenna_app::{Arrival, Effect, HostAnswer, HostRequest, NetworkRequest, Watch};
use lumenna_host::Lumenna;
use lumenna_records::words::spoken;
use lumenna_store::ChangeHash;

/// What the shell does, once an event has run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shell {
    /// Draw the view again.
    Render,
    /// Say this, once focus is where the view model puts it.
    Announce(String),
    /// Say the day's heading, "<day>. <summary>", the day in the shell's own words: what a
    /// day newly shown says. This is the summary.
    Heading(String),
    /// Save this file where the person keeps files: a backup, an export.
    Save(lumenna_app::SaveFile),
    /// Show this notification, as the platform shows one.
    Notify(lumenna_app::Notice),
}

/// What is left of `effects` for the shell, drawing once and first: everything the
/// middlewares answer is gone by now.
#[must_use]
pub fn shell(effects: Vec<Effect>) -> Vec<Shell> {
    let mut shell = Vec::new();
    let mut draw = false;
    for effect in effects {
        match effect {
            Effect::Render(_) => draw = true,
            Effect::Announce(request) => {
                let said = request.operation;
                shell.push(if said.heading { Shell::Heading(said.text) } else { Shell::Announce(said.text) });
            }
            Effect::Save(request) => shell.push(Shell::Save(request.operation)),
            Effect::Notify(request) => shell.push(Shell::Notify(request.operation)),
            Effect::Watch(_) | Effect::Host(_) | Effect::Network(_) => {}
        }
    }
    if draw {
        shell.insert(0, Shell::Render);
    }
    shell
}

/// The store as it stands now, and the heads it was read at.
///
/// # Errors
///
/// If the store cannot be read.
pub fn arrival(lumenna: &Lumenna) -> lumenna_host::Result<(Arrival, Vec<ChangeHash>)> {
    let (snapshot, heads) = lumenna.snapshot()?;
    Ok((Arrival { snapshot: Arc::new(snapshot), now: Zoned::now() }, heads))
}

/// What the host answers to one of the app's requests: the whole of the host's part.
pub fn answer(lumenna: &Lumenna, request: HostRequest) -> HostAnswer {
    let wrote = |result: lumenna_host::Result<lumenna_records::Change>| {
        let after = arrival(lumenna).ok().map(|(arrival, _)| arrival);
        HostAnswer::Wrote(Box::new(result), after)
    };
    match request {
        HostRequest::Act { action, answer } => wrote(lumenna.act(action, answer)),
        HostRequest::AddTask { text } => wrote(lumenna.add_task(&text)),
        HostRequest::EditTask { id, edit } => wrote(lumenna.edit_task(&id, edit)),
        HostRequest::AddBlock(block) => wrote(lumenna.add_block(block)),
        HostRequest::EditBlock { series, edit, scope } => wrote(lumenna.edit_block(&series, edit, scope)),
        HostRequest::AddFilter { name, query } => wrote(lumenna.add_filter(&name, &query)),
        HostRequest::Settings => HostAnswer::Listed(lumenna.settings(None)),
        HostRequest::SetSetting { key, value } => wrote(lumenna.set_setting(&key, &value)),
        HostRequest::Backup => HostAnswer::Saved(Box::new(lumenna.backup_file().map(|file| {
            let said = spoken(&file.announcement, &file.notices);
            (lumenna_app::SaveFile { name: file.name, bytes: file.bytes, kind: "application/octet-stream".to_owned() }, said)
        }))),
        HostRequest::Export(format) => HostAnswer::Saved(Box::new(lumenna.export(format, None, false).map(|exported| {
            let name = lumenna_records::exports::export_name(format, Zoned::now().date());
            let said = format!("Exported as {name}");
            let bytes = exported.content.unwrap_or_default().into_bytes();
            (lumenna_app::SaveFile { name, bytes, kind: "text/plain;charset=utf-8".to_owned() }, said)
        }))),
        HostRequest::Import { name, bytes } => {
            let done = lumenna.import_bytes(&name, bytes).map(|imported| match imported {
                lumenna_records::types::Imported::Export { done } => spoken(&done.announcement, &done.notices),
                lumenna_records::types::Imported::Backup { done } => spoken(&done.announcement, &done.notices),
            });
            let after = arrival(lumenna).ok().map(|(arrival, _)| arrival);
            HostAnswer::Imported(Box::new(done), after)
        }
        HostRequest::Devices => HostAnswer::Devices(lumenna.devices()),
        HostRequest::RemindersDue { since, now } => HostAnswer::Found(lumenna.reminders_due(since, now)),
        HostRequest::Dismiss(firings) => wrote(each(&firings, |(key, occurrence)| lumenna.dismiss_reminder(key.clone(), occurrence.clone()))),
        HostRequest::Snooze(firings) => wrote(each(&firings, |(key, occurrence)| lumenna.snooze_reminder(key.clone(), occurrence.clone(), None))),
        HostRequest::CarryForward { block, firings } => wrote(lumenna.carry_rest_forward(&block).and_then(|carried| {
            each(&firings, |(key, occurrence)| lumenna.dismiss_reminder(key.clone(), occurrence.clone()))?;
            Ok(carried)
        })),
        HostRequest::Undo => wrote(lumenna.undo()),
        HostRequest::Redo => wrote(lumenna.redo()),
        HostRequest::Apply { proposal, accepted } => {
            let applied = match proposal {
                Proposal::Replan(replan) => {
                    let changes = accepted.iter().filter_map(|at| replan.changes.get(*at).cloned()).collect();
                    lumenna.apply_replan(replan, changes, None).map(|made| Applied {
                        announcement: made.announcement,
                        notices: made.notices,
                        next: Proposal::Replan(made.next),
                    })
                }
                Proposal::Day(plan) => {
                    let sittings = accepted.iter().filter_map(|at| plan.sittings.get(*at).cloned()).collect();
                    lumenna.apply_day_plan(plan, sittings, None).map(|made| Applied {
                        announcement: made.announcement,
                        notices: made.notices,
                        next: Proposal::Day(made.next),
                    })
                }
            };
            let after = arrival(lumenna).ok().map(|(arrival, _)| arrival);
            HostAnswer::Applied(Box::new(applied), after)
        }
    }
}

/// Runs `one` over every firing, answering with the last change: what a summary's Dismiss says.
fn each(
    firings: &[(String, String)],
    one: impl Fn(&(String, String)) -> lumenna_host::Result<lumenna_records::Change>,
) -> lumenna_host::Result<lumenna_records::Change> {
    let mut last = lumenna_records::Change::unchanged("");
    for firing in firings {
        last = one(firing)?;
    }
    Ok(last)
}


/// The store's work under way: what has been handed to the [`StoreWorker`] and not yet answered.
/// The app is still when it is none: everything an event led to has been drawn and said, but for
/// what waits on the network.
#[derive(Clone, Default)]
pub struct Busy(Arc<Count>);

#[derive(Default)]
struct Count {
    under_way: Mutex<usize>,
    still: Condvar,
    hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl Busy {
    fn start(&self) {
        *self.0.under_way.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    }

    fn end(&self) {
        let still = {
            let mut under_way = self.0.under_way.lock().unwrap_or_else(PoisonError::into_inner);
            *under_way = under_way.saturating_sub(1);
            *under_way == 0
        };
        if still {
            self.0.still.notify_all();
            let hook = self.0.hook.lock().unwrap_or_else(PoisonError::into_inner).clone();
            if let Some(hook) = hook {
                hook();
            }
        }
    }

    /// Whether nothing is under way.
    #[must_use]
    pub fn is_still(&self) -> bool {
        *self.0.under_way.lock().unwrap_or_else(PoisonError::into_inner) == 0
    }

    /// Calls `hook` each time the last of the work under way is answered, after what it led to
    /// has been handed to the shell.
    pub fn when_still(&self, hook: impl Fn() + Send + Sync + 'static) {
        *self.0.hook.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::new(hook));
    }

    /// Waits until nothing is under way: for tests. Never on a browser's one thread, which would
    /// wait for itself.
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    pub fn wait(&self) {
        let mut under_way = self.0.under_way.lock().unwrap_or_else(PoisonError::into_inner);
        while *under_way > 0 {
            under_way = self.0.still.wait(under_way).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// What the store worker is asked.
enum Job {
    Host(Box<HostRequest>, EffectResolver<HostAnswer>),
    Watch(EffectResolver<Arrival>),
    Tick,
}

/// The way to the store worker, shared by both middlewares and whoever says the store may have
/// moved.
#[derive(Clone)]
pub struct Store {
    jobs: UnboundedSender<Job>,
    busy: Busy,
}

impl Store {
    /// A store's way in, and the worker that answers it: run [`StoreWorker::run`] wherever the
    /// platform runs work off the caller's thread.
    #[must_use]
    pub fn new(lumenna: Arc<Lumenna>) -> (Self, StoreWorker) {
        let (jobs, inbox) = unbounded();
        let busy = Busy::default();
        let worker = StoreWorker { lumenna, inbox, watching: Vec::new(), sent: None, busy: busy.clone() };
        (Self { jobs, busy }, worker)
    }

    /// The work under way.
    #[must_use]
    pub fn busy(&self) -> &Busy {
        &self.busy
    }

    /// Looks for a change made elsewhere — `lum`, another app, a sync — or a new minute, and
    /// tells the app if there is one. A shell calls this on a timer, or when told the store
    /// changed.
    pub fn tick(&self) {
        self.send(Job::Tick);
    }

    fn send(&self, job: Job) {
        self.busy.start();
        if self.jobs.unbounded_send(job).is_err() {
            self.busy.end();
        }
    }
}

/// The middleware that watches the store for the app.
pub struct Watching(Store);

impl EffectMiddleware for Watching {
    type Op = Watch;

    fn try_process_effect(&self, _: Watch, resolver: EffectResolver<Arrival>) {
        self.0.send(Job::Watch(resolver));
    }
}

/// The middleware that answers the app's requests of the host.
pub struct Writing(Store);

impl EffectMiddleware for Writing {
    type Op = HostRequest;

    fn try_process_effect(&self, request: HostRequest, resolver: EffectResolver<HostAnswer>) {
        self.0.send(Job::Host(Box::new(request), resolver));
    }
}

/// Answers the store's jobs in the order they came, one at a time.
pub struct StoreWorker {
    lumenna: Arc<Lumenna>,
    inbox: UnboundedReceiver<Job>,
    /// Every watch the app holds open, answered whenever the store moves.
    watching: Vec<EffectResolver<Arrival>>,
    /// The heads last sent, and the minute: a tick sends again only when either moved.
    sent: Option<(Vec<ChangeHash>, i8)>,
    busy: Busy,
}

impl StoreWorker {
    /// Answers jobs until every way in has gone.
    pub async fn run(mut self) {
        while let Some(job) = self.inbox.next().await {
            self.answer(job);
            self.busy.end();
        }
    }

    fn answer(&mut self, job: Job) {
        match job {
            Job::Host(request, mut resolver) => {
                let found = answer(&self.lumenna, *request);
                if let HostAnswer::Wrote(_, Some(_)) | HostAnswer::Applied(_, Some(_)) | HostAnswer::Imported(_, Some(_)) = &found
                    && let Ok((_, heads)) = self.lumenna.snapshot()
                {
                    // The app has the store as this write left it.
                    self.sent = Some((heads, Zoned::now().minute()));
                }
                resolver.resolve(found);
            }
            Job::Watch(mut resolver) => {
                if let Ok((arrival, heads)) = arrival(&self.lumenna) {
                    self.sent = Some((heads, arrival.now.minute()));
                    resolver.resolve(arrival);
                }
                self.watching.push(resolver);
            }
            Job::Tick => {
                let Ok((arrival, heads)) = arrival(&self.lumenna) else { return };
                let minute = arrival.now.minute();
                if self.sent.as_ref().is_some_and(|(sent, at)| *sent == heads && *at == minute) {
                    return;
                }
                self.sent = Some((heads, minute));
                for resolver in &mut self.watching {
                    resolver.resolve(arrival.clone());
                }
            }
        }
    }
}

/// The app with the store's middlewares and the platform's network under it.
pub type Stack<N> = HandleEffectLayer<HandleEffectLayer<HandleEffectLayer<Core<lumenna_app::Lumenna>, Watching>, Writing>, N>;

/// The app over `store`, the network answered by `network`.
pub fn stack<N>(store: &Store, network: N) -> Stack<N>
where
    N: EffectMiddleware<Op = NetworkRequest> + 'static,
{
    Core::<lumenna_app::Lumenna>::new()
        .handle_effects_using(Watching(store.clone()))
        .handle_effects_using(Writing(store.clone()))
        .handle_effects_using(network)
}
