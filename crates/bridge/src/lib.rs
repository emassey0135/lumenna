//! The app beside its store: the app's host effects answered by the host, in one loop.
//!
//! A [`Session`] holds the Crux core and the open store. An event goes in; every effect the
//! app asks of the host is answered here, at once, on the caller's thread, and what is left is
//! the shell's: draw, and say. That is all a shell is handed ([`Shell`]).
//!
//! One loop for every shell that runs the app in the same process as its store: the web
//! worker, which has no threads for Crux's middleware, and the desktop apps, which link it.
//! Operations are milliseconds against local SQLite, so answering inline never holds a frame.
//! [`answer`] is the whole of what the host does for the app, and middleware for a shell that
//! wants the host off its thread wraps the same function.

use std::sync::Arc;

use crux_core::Core;
use crux_core::Request;
use jiff::Zoned;
use lumenna_app::proposal::{Applied, Kind, Pins, Proposal};
use lumenna_app::{Arrival, Effect, Event, HostAnswer, HostRequest, NetworkAnswer, NetworkRequest, ViewModel, Watch};
use lumenna_records::proposals::{DayPins, ReplanPins};
use lumenna_host::Lumenna;
use lumenna_store::ChangeHash;

/// What the shell does, once an event has run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shell {
    /// Draw [`Session::view`] again.
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
    /// Do this on the network, and answer it with [`Session::answer_network`] and its `id` when
    /// it can be answered: a pairing more than once.
    Network {
        /// What answering it names.
        id: u64,
        /// What to do.
        request: NetworkRequest,
    },
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
        HostRequest::Choices { action } => HostAnswer::Offered(lumenna.choices(action)),
        HostRequest::Propose { kind, pins, style } => HostAnswer::Proposed(Box::new(match (kind, pins) {
            (Kind::Replan, pins) => {
                let pins = match pins {
                    Some(Pins::Replan(pins)) => pins,
                    _ => ReplanPins::default(),
                };
                lumenna.replan(style, pins, None).map(Proposal::Replan)
            }
            (Kind::PlanDay(date), pins) => {
                let pins = match pins {
                    Some(Pins::Day(pins)) => pins,
                    _ => DayPins::default(),
                };
                lumenna.propose_day_plan(Some(date), style, pins, None).map(Proposal::Day)
            }
        })),
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

/// A record's sentence, then anything else worth saying.
fn spoken(announcement: &str, notices: &[String]) -> String {
    std::iter::once(announcement).chain(notices.iter().map(String::as_str)).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The app and its store, together.
pub struct Session {
    core: Core<lumenna_app::Lumenna>,
    lumenna: Arc<Lumenna>,
    /// Every watch the app holds open, answered whenever the store moves.
    watching: Vec<Request<Watch>>,
    /// The heads last sent, and the minute: a tick sends again only when either moved.
    sent: Option<(Vec<ChangeHash>, i8)>,
    /// What the network has been asked and not finished answering, by id.
    network: std::collections::BTreeMap<u64, Request<NetworkRequest>>,
    /// The id the next network request gets.
    next_id: u64,
}

impl Session {
    /// The app over this store, started: it is watching the store, and has it.
    #[must_use]
    pub fn new(lumenna: Arc<Lumenna>) -> (Self, Vec<Shell>) {
        let mut session = Self {
            core: Core::new(),
            lumenna,
            watching: Vec::new(),
            sent: None,
            network: std::collections::BTreeMap::new(),
            next_id: 0,
        };
        let shell = session.send(Event::Start);
        (session, shell)
    }

    /// Runs an event, answering everything it asks of the host, and says what is left for the
    /// shell.
    pub fn send(&mut self, event: Event) -> Vec<Shell> {
        let effects = self.core.process_event(event);
        self.run(effects)
    }

    /// Looks for a change made elsewhere — `lum`, another app, a sync — or a new minute, and
    /// tells the app if there is one. A shell calls this on a timer, or when told the store
    /// changed.
    pub fn tick(&mut self) -> Vec<Shell> {
        let Ok((arrival, heads)) = arrival(&self.lumenna) else { return Vec::new() };
        let minute = arrival.now.minute();
        if self.sent.as_ref().is_some_and(|(sent, at)| *sent == heads && *at == minute) {
            return Vec::new();
        }
        self.sent = Some((heads, minute));
        let mut effects = Vec::new();
        let mut watching = std::mem::take(&mut self.watching);
        for handle in &mut watching {
            if let Ok(more) = self.core.resolve(handle, arrival.clone()) {
                effects.extend(more);
            }
        }
        self.watching = watching;
        self.run(effects)
    }

    /// Answers the network request `id`; a pairing's stream is answered until it ends.
    pub fn answer_network(&mut self, id: u64, answer: NetworkAnswer) -> Vec<Shell> {
        let ends = !matches!(answer, NetworkAnswer::Code(_) | NetworkAnswer::Words(_));
        let Some(mut request) = self.network.remove(&id) else { return Vec::new() };
        let effects = self.core.resolve(&mut request, answer).unwrap_or_default();
        if !ends {
            self.network.insert(id, request);
        }
        self.run(effects)
    }

    /// What to draw.
    #[must_use]
    pub fn view(&self) -> ViewModel {
        self.core.view()
    }

    fn run(&mut self, mut effects: Vec<Effect>) -> Vec<Shell> {
        let mut shell = Vec::new();
        while !effects.is_empty() {
            let mut next = Vec::new();
            for effect in effects {
                match effect {
                    Effect::Render(_) => {
                        if shell.last() != Some(&Shell::Render) {
                            shell.push(Shell::Render);
                        }
                    }
                    Effect::Announce(request) => {
                        let said = request.operation;
                        shell.push(if said.heading { Shell::Heading(said.text) } else { Shell::Announce(said.text) });
                    }
                    Effect::Save(request) => shell.push(Shell::Save(request.operation)),
                    Effect::Notify(request) => shell.push(Shell::Notify(request.operation)),
                    Effect::Network(request) => {
                        let id = self.next_id;
                        self.next_id += 1;
                        shell.push(Shell::Network { id, request: request.operation.clone() });
                        self.network.insert(id, request);
                    }
                    Effect::Watch(mut request) => {
                        if let Ok((arrival, heads)) = arrival(&self.lumenna) {
                            self.sent = Some((heads, arrival.now.minute()));
                            if let Ok(more) = self.core.resolve(&mut request, arrival) {
                                next.extend(more);
                            }
                        }
                        self.watching.push(request);
                    }
                    Effect::Host(request) => {
                        let (operation, mut handle) = request.split();
                        let found = answer(&self.lumenna, operation);
                        if let HostAnswer::Wrote(_, Some(_)) | HostAnswer::Applied(_, Some(_)) | HostAnswer::Imported(_, Some(_)) = &found
                            && let Ok((_, heads)) = self.lumenna.snapshot()
                        {
                            // The app has the store as this write left it.
                            self.sent = Some((heads, Zoned::now().minute()));
                        }
                        if let Ok(more) = self.core.resolve(&mut handle, found) {
                            next.extend(more);
                        }
                    }
                }
            }
            effects = next;
        }
        // Drawing once, after everything else, is enough.
        if let Some(at) = shell.iter().position(|s| *s == Shell::Render) {
            shell.remove(at);
            shell.insert(0, Shell::Render);
        }
        shell
    }
}
