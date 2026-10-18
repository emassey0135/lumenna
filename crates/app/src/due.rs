//! Reminders due: what came due and has not been dealt with, each with the core's buttons.
//!
//! When to look is the shell's (a running app every twenty seconds, a browser tab also on
//! coming back); what came due, what each says and what its buttons do are the core's; which
//! are still listed is the app's. Each new one is said, and shown as the platform shows a
//! notification.

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::LumennaError;
use lumenna_records::reminders::{DueReminder, DueReminders};
use serde::{Deserialize, Serialize};

use crate::host::HostRequest;
use crate::model::Model;
use crate::{Effect, Event, Lumenna, host, proposal, say};

/// What the person, or the shell's clock, does about reminders due.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
}

/// A notification to show as the platform shows one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
        });
    }
    out
}


/// What a reminder's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Deal {
    Dismiss,
    Snooze,
    Carry,
}

impl Lumenna {
    /// An overrun warning's Replan from Now closed: once something was replanned, the warning is
    /// dealt with.
    pub(crate) fn replanned(&self, model: &mut Model) -> Command<Effect, Event> {
        let Some(id) = model.replanning.take() else { return Command::done() };
        if !model.proposal_made {
            return Command::done();
        }
        self.deal(&id, Deal::Dismiss, model)
    }

    /// Takes the reminder `id` off the list and does what its button asks.
    fn deal(&self, id: &str, how: Deal, model: &mut Model) -> Command<Effect, Event> {
        let Some(at) = model.due.iter().position(|d| d.id == id) else { return Command::done() };
        let item = model.due.remove(at);
        let firings: Vec<(String, String)> = item.firings.iter().map(|f| (f.key.clone(), f.occurrence.clone())).collect();
        let request = match how {
            Deal::Dismiss => HostRequest::Dismiss(firings),
            Deal::Snooze => HostRequest::Snooze(firings),
            Deal::Carry => HostRequest::CarryForward { block: item.block.unwrap_or_default(), firings },
        };
        render().and(host(request))
    }

    pub(crate) fn due(&self, event: DueEvent, model: &mut Model) -> Command<Effect, Event> {
        match event {
            DueEvent::Look { since, now } => host(HostRequest::RemindersDue { since, now }),
            DueEvent::Dismiss(id) => self.deal(&id, Deal::Dismiss, model),
            DueEvent::Snooze(id) => self.deal(&id, Deal::Snooze, model),
            DueEvent::Carry(id) => self.deal(&id, Deal::Carry, model),
            // Cancelled, the warning stays; something replanned, it is dismissed.
            DueEvent::Replan(id) => {
                model.replanning = Some(id);
                self.propose(proposal::Kind::Replan, model)
            }
        }
    }

    pub(crate) fn found(&self, result: Result<lumenna_records::reminders::DueReminders, LumennaError>, model: &mut Model) -> Command<Effect, Event> {
        let Ok(found) = result else { return Command::done() };
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
        render().and(said)
    }
}
