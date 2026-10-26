//! Reminders due: what came due and has not been dealt with, each with the core's buttons.
//!
//! When to look is the shell's (a running app every twenty seconds, a browser tab also on
//! coming back); what came due, what each says and what its buttons do are the core's; which
//! are still listed is the app's. Each new one is said, and shown as the platform shows a
//! notification.

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::{ActionKind, LumennaError};
use lumenna_records::reminders::{DueReminder, DueReminders};
use serde::{Deserialize, Serialize};

use crate::host::HostRequest;
use crate::model::Screen;
use crate::{Effect, Event, Lumenna, host, proposal, say, write};

/// What the person, or the shell's clock, does about reminders due.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DueEvent {
    /// Look for what came due after `since` (RFC 3339) until now.
    Look {
        /// When the shell last looked.
        since: String,
        /// The moment looked at, RFC 3339: the shell's clock, which says when to look; now when
        /// absent.
        #[serde(default)]
        now: Option<String>,
    },
    /// Dismiss one, on every device.
    Dismiss(String),
    /// Snooze one for the core's ten minutes.
    Snooze(String),
    /// Carry the work a block left short on to its next block, and dismiss the reminder.
    Carry(String),
    /// Open Replan from Now for an overrun; the warning is dismissed once something is replanned.
    Replan(String),
}

/// One reminder due, as listed.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Due {
    /// What tells it apart: a notification's tag, the page's element.
    pub id: String,
    /// A notification's title.
    pub title: String,
    /// The whole message.
    pub text: String,
    /// The core's Dismiss.
    pub dismiss: String,
    /// The core's Snooze, but for a summary of what was missed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze: Option<String>,
    /// At a block's end with work left short: the core's Carry Forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carry: Option<String>,
    /// When a block's work outruns its time: the core's Replan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replan: Option<String>,
    /// What carrying forward carries: `<series>@<date>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    /// What dismissing it dismisses: itself, or for a summary, everything missed.
    pub firings: Vec<DueReminder>,
    /// Its buttons, in the order every platform shows them: what it offers of its own (Carry
    /// Forward, Replan) first, then Dismiss and Snooze. Each is handed back as its `DueEvent`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<DueButton>,
}

/// One of a reminder's buttons.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DueButton {
    /// Which: `carry`, `replan`, `dismiss`, `snooze`, as `DueEvent::Carry`, `Replan`, `Dismiss`,
    /// `Snooze`.
    pub id: String,
    /// Its name, the core's, in the shell's capitals.
    pub title: String,
    /// What a notification's own button says once done, where the app is not there to say the
    /// write's own sentence: empty for Replan, which opens a screen.
    pub said: String,
}

/// A reminder's buttons, in order.
fn buttons(carry: Option<&String>, replan: Option<&String>, dismiss: &str, snooze: Option<&String>) -> Vec<DueButton> {
    let button = |id: &str, title: &str, said: String| DueButton { id: id.to_owned(), title: title.to_owned(), said };
    let snoozed = format!("Snoozed for {}", lumenna_records::words::duration(lumenna_records::reminders::SNOOZE_MINS));
    carry
        .map(|title| button("carry", title, String::new()))
        .into_iter()
        .chain(replan.map(|title| button("replan", title, String::new())))
        .chain([button("dismiss", dismiss, "Dismissed".to_owned())])
        .chain(snooze.map(|title| button("snooze", title, snoozed.clone())))
        .collect()
}

/// A notification to show as the platform shows one.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    /// What tells it apart.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its text.
    pub text: String,
}

impl crux_core::capability::Operation for Notice {
    type Output = ();
}

/// What `found` adds to the list, one by one and the missed as one summary.
#[must_use]
pub fn arrived(found: &DueReminders) -> Vec<Due> {
    let mut out = Vec::new();
    if !found.missed_summary.is_empty()
        && let Some(first) = found.missed.first()
    {
        out.push(Due {
            id: format!("missed-{}-{}", first.key, first.occurrence),
            title: "Lumenna".to_owned(),
            text: found.missed_summary.clone(),
            dismiss: first.dismiss.clone(),
            snooze: None,
            carry: None,
            replan: None,
            block: None,
            firings: found.missed.clone(),
            buttons: buttons(None, None, &first.dismiss, None),
        });
    }
    for reminder in &found.due {
        out.push(Due {
            id: format!("{}-{}", reminder.key, reminder.occurrence),
            title: reminder.title.clone(),
            text: reminder.text.clone(),
            dismiss: reminder.dismiss.clone(),
            snooze: Some(reminder.snooze.clone()),
            carry: (!reminder.carry.is_empty()).then(|| reminder.carry.clone()),
            replan: (!reminder.replan.is_empty()).then(|| reminder.replan.clone()),
            block: Some(format!("{}@{}", reminder.target, reminder.occurrence)),
            firings: vec![reminder.clone()],
            buttons: buttons(
                Some(&reminder.carry).filter(|c| !c.is_empty()),
                Some(&reminder.replan).filter(|r| !r.is_empty()),
                &reminder.dismiss,
                Some(&reminder.snooze),
            ),
        });
    }
    out
}


/// Whether the reminder `id` due offers what `has` asks of it.
fn offers(model: &Screen, id: &str, has: impl Fn(&Due) -> bool) -> bool {
    model.due.iter().any(|d| d.id == id && has(d))
}

/// Why a reminder does not offer `kind`.
fn not_offered(kind: ActionKind) -> String {
    lumenna_records::not_offered(kind, lumenna_records::Subject::Reminder, false)
}

/// What a reminder's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deal {
    Dismiss,
    Snooze,
    Carry,
}

/// Whether the store has an acknowledgement — dismissed, snoozed — of `firing`, from this
/// device or another.
fn acknowledged(snapshot: &lumenna_core::snapshot::Snapshot, firing: &DueReminder) -> bool {
    let (Ok(id), Ok(date)) = (firing.key.trim().parse(), firing.occurrence.parse::<jiff::civil::Date>()) else { return false };
    snapshot.acks.contains_key(&(id, date))
}

impl Screen {
    /// Takes off the list every reminder due whose firings have all been dealt with, here or on
    /// another device: the store says so, whoever dealt with it.
    pub(crate) fn drop_acknowledged(&mut self) {
        let Some(store) = &self.store else { return };
        let snapshot = std::sync::Arc::clone(&store.snapshot);
        self.due.retain(|due| due.firings.is_empty() || !due.firings.iter().all(|firing| acknowledged(&snapshot, firing)));
    }
}

impl Lumenna {
    /// An overrun warning's Replan from Now closed: once something was replanned, the warning is
    /// dealt with.
    pub(crate) fn replanned(&self, model: &mut Screen) -> Command<Effect, Event> {
        let Some(id) = model.replanning.take() else { return Command::done() };
        if !model.proposal_made {
            return Command::done();
        }
        self.deal(&id, Deal::Dismiss, model)
    }

    /// Does what the reminder `id`'s button asks; it leaves the list once the host has done it,
    /// and stays, its failure said, if the host could not. A second press meanwhile does nothing.
    fn deal(&self, id: &str, how: Deal, model: &mut Screen) -> Command<Effect, Event> {
        let Some(item) = model.due.iter().find(|d| d.id == id).cloned() else { return Command::done() };
        if model.dealing.iter().any(|(_, dealing)| dealing == id) {
            return Command::done();
        }
        let firings: Vec<(String, String)> = item.firings.iter().map(|f| (f.key.clone(), f.occurrence.clone())).collect();
        let request = match how {
            Deal::Dismiss => HostRequest::Dismiss(firings),
            Deal::Snooze => HostRequest::Snooze(firings),
            Deal::Carry => HostRequest::CarryForward { block: item.block.unwrap_or_default(), firings },
        };
        let (number, sent) = write(model, request, None);
        model.dealing.push((number, item.id));
        sent
    }

    pub(crate) fn due(&self, event: DueEvent, model: &mut Screen) -> Command<Effect, Event> {
        match event {
            // One look with the host at a time; those asked for meanwhile go as one after it,
            // so a burst of them (a clock moved on, a tab come back) is not a queue of them.
            DueEvent::Look { since, now } => {
                if model.looking {
                    let since = model.look_next.take().map_or(since, |(first, _)| first);
                    model.look_next = Some((since, now));
                    return Command::done();
                }
                model.looking = true;
                host(HostRequest::RemindersDue { since, now })
            }
            DueEvent::Dismiss(id) => self.deal(&id, Deal::Dismiss, model),
            DueEvent::Snooze(id) => self.deal(&id, Deal::Snooze, model),
            // A reminder without the button refuses it, in the core's words.
            DueEvent::Carry(id) if !offers(model, &id, |d| d.carry.is_some()) => say(not_offered(ActionKind::CarryRest)),
            // One this screen holds without Replan refuses it. One it never held — the system's
            // notification, on a platform that schedules them and never looks here — opens Replan
            // all the same: its button is what was pressed.
            DueEvent::Replan(id) if offers(model, &id, |d| d.replan.is_none()) => say(not_offered(ActionKind::Replan)),
            DueEvent::Carry(id) => self.deal(&id, Deal::Carry, model),
            // Today, with Replan from Now open over it. Cancelled, the warning stays; something
            // replanned, it is dismissed.
            DueEvent::Replan(id) => {
                let shown = model.show_day(None);
                // Only a warning held here is dismissed once something is replanned: the system
                // dismissed its own as it was pressed.
                model.replanning = model.due.iter().any(|d| d.id == id).then_some(id);
                shown.and(self.propose(proposal::Kind::Replan, model))
            }
        }
    }

    pub(crate) fn found(&self, result: Result<lumenna_records::reminders::DueReminders, LumennaError>, model: &mut Screen) -> Command<Effect, Event> {
        model.looking = false;
        let next = match model.look_next.take() {
            Some((since, now)) => {
                model.looking = true;
                host(HostRequest::RemindersDue { since, now })
            }
            None => Command::done(),
        };
        let Ok(found) = result else { return next };
        let mut said = Command::done();
        for item in arrived(&found) {
            if model.due.iter().any(|d| d.id == item.id) {
                continue;
            }
            // Each is said, always, and shown as a notification where the platform allows.
            let notice: Command<Effect, Event> =
                Command::notify_shell(Notice { id: item.id.clone(), title: item.title.clone(), text: item.text.clone() }).into();
            said = said.and(self::say(item.text.clone())).and(notice);
            model.due.push(item);
        }
        render().and(said).and(next)
    }
}
