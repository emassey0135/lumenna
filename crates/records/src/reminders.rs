//! Reminders as records: what a notification shows, and a task's or block's list of them.

use lumenna_core::id::ReminderId;
use lumenna_core::model::{Reminder, ReminderAnchor, ReminderTarget, Trigger};
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::error::{LumennaError, Result};
use crate::resolve;
use crate::words::{capitalised, count_line, duration};

/// How old a firing may be and still be said on its own; older ones, from while nothing was
/// running, are folded into one summary rather than read out one by one.
pub const STALE_MINS: i64 = 5;

/// How long a snooze lasts from a notification's own button.
pub const SNOOZE_MINS: u32 = 10;

/// One firing, as a notification shows it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DueReminder {
    /// The reminder, for dismissing or snoozing it: its own identifier, or a default's.
    pub key: String,
    /// Which occurrence, ISO: with `key`, what a dismissal names.
    pub occurrence: String,
    /// When it goes off, RFC 3339.
    pub at: String,
    /// What it is about: a task's or a block series' identifier.
    pub target: String,
    /// `task` or `block`.
    pub kind: String,
    /// What it is about, by name: a notification's title.
    pub title: String,
    /// The whole message, in the verbosity this device's settings ask.
    pub text: String,
    /// The same, said of the past, for a summary of what was missed: "Deep work started".
    pub missed: String,
    /// The notification's button that dismisses it, on every device.
    pub dismiss: String,
    /// The notification's button that snoozes it for [`SNOOZE_MINS`].
    pub snooze: String,
    /// At a block's end with work left short: the button that carries it forward
    /// ([`Lumenna::carry_rest_forward`] of `target@occurrence`). Empty when there is none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "uniffi", uniffi(default = ""))]
    pub carry: String,
    /// When a block's planned work outruns its time: the button that opens Replan from Now.
    /// Empty when there is none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[cfg_attr(feature = "uniffi", uniffi(default = ""))]
    pub replan: String,
}

/// What reminders to give now, or over a while.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DueReminders {
    /// How many, or why none.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// To show one by one, in time order.
    pub due: Vec<DueReminder>,
    /// Those too old to show one by one, from while nothing was running.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missed: Vec<DueReminder>,
    /// What to show for `missed`, as one notification: "2 reminders missed: Deep work
    /// started, Call the bank was due." Empty when nothing was missed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub missed_summary: String,
    /// Whether another process on this device gives reminders, so this one gives none: ask
    /// again every few seconds, to take over when it stops.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub elsewhere: bool,
}

crate::announced!(DueReminders, ReminderList);

/// One reminder a task or block has, its own or a default.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReminderView {
    /// Its identifier: what removing it takes. A default's cannot be removed alone.
    pub id: String,
    /// Whether it is the target's own, rather than a default from settings.
    pub own: bool,
    /// `due`, `start`, `end` or `absolute`.
    pub anchor: String,
    /// Minutes: negative before, positive after.
    pub offset_mins: i32,
    /// For `absolute`, the moment, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// How it reads: "5 minutes before it starts".
    pub words: String,
    /// What can be done to it: Remove, for one of its own; nothing for a default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<crate::actions::Action>,
}

/// The reminders a task or block has.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReminderList {
    /// How many, and whether they are its own or the defaults.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Them, in time order of what they count from.
    pub reminders: Vec<ReminderView>,
    /// What can be done to the list: Add Reminder, which asks for a phrase.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<crate::actions::Action>,
}

/// "in 5 minutes", "now", "5 minutes ago": the gap between a firing and what it is about.
pub fn gap(offset_mins: i32) -> (String, std::cmp::Ordering) {
    let minutes = offset_mins.unsigned_abs();
    (duration(minutes), 0.cmp(&offset_mins))
}

/// What a trigger reads as on a target's list.
pub fn trigger_words(trigger: &Trigger) -> String {
    let (span, when) = gap(trigger.offset_mins);
    let (verb, at) = match trigger.anchor {
        ReminderAnchor::BlockStart => ("it starts", "when it starts"),
        ReminderAnchor::BlockEnd => ("it ends", "when it ends"),
        ReminderAnchor::Due => ("it is due", "when it is due"),
        ReminderAnchor::Absolute(_) => return "at a set time".to_owned(),
    };
    match when {
        std::cmp::Ordering::Greater => format!("{span} before {verb}"),
        std::cmp::Ordering::Equal => at.to_owned(),
        std::cmp::Ordering::Less => format!("{span} after {verb}"),
    }
}

/// A reminder anchor as its word.
pub fn anchor_word(anchor: &ReminderAnchor) -> &'static str {
    match anchor {
        ReminderAnchor::Due => "due",
        ReminderAnchor::BlockStart => "start",
        ReminderAnchor::BlockEnd => "end",
        ReminderAnchor::Absolute(_) => "absolute",
    }
}

/// One reminder as its list shows it.
pub fn view(id: ReminderId, trigger: &Trigger, own: bool) -> ReminderView {
    let words = trigger_words(trigger);
    ReminderView {
        actions: if own { crate::actions::reminder(&id.to_string(), &words) } else { Vec::new() },
        id: id.to_string(),
        own,
        anchor: anchor_word(&trigger.anchor).to_owned(),
        offset_mins: trigger.offset_mins,
        at: match &trigger.anchor {
            ReminderAnchor::Absolute(at) => Some(at.timestamp().to_string()),
            _ => None,
        },
        words,
    }
}

/// The task or block `input` names.
pub fn reminder_target(snapshot: &Snapshot, input: &str) -> Result<ReminderTarget> {
    if let Ok(id) = resolve::task_id(snapshot, input) {
        return Ok(ReminderTarget::Task(id));
    }
    resolve::series_id(snapshot, input)
        .map(ReminderTarget::Block)
        .map_err(|_| LumennaError::new(format!("nothing here matches '{input}'")))
}

/// What a reminder target is called.
pub fn target_name(snapshot: &Snapshot, target: ReminderTarget) -> String {
    match target {
        ReminderTarget::Task(id) => snapshot.tasks.get(&id).map(|t| t.title.clone()),
        ReminderTarget::Block(id) => snapshot.series.get(&id).map(|s| s.title.clone()),
    }
    .unwrap_or_default()
}


/// The reminders a task or block has: its own, or the defaults it takes.
///
/// # Errors
///
/// If nothing matches `target`.
pub fn reminders_of(snapshot: &Snapshot, target: &str) -> Result<ReminderList> {
    let target = reminder_target(snapshot, target)?;
    let mut own: Vec<&Reminder> = snapshot
        .reminders
        .values()
        .filter(|r| r.target == target && r.deleted_at.is_none())
        .collect();
    own.sort_by_key(|r| r.trigger.offset_mins);
    let views: Vec<ReminderView> = if own.is_empty() {
        let defaults = match target {
            ReminderTarget::Task(_) => &snapshot.settings.default_task_reminders,
            ReminderTarget::Block(_) => &snapshot.settings.default_block_reminders,
        };
        defaults
            .iter()
            .map(|trigger| view(lumenna_core::remind::default_key(&target, trigger), trigger, false))
            .collect()
    } else {
        own.iter().map(|r| view(r.id, &r.trigger, true)).collect()
    };
    let mine = views.first().is_some_and(|v| v.own);
    let target_id = match target {
        ReminderTarget::Task(id) => id.to_string(),
        ReminderTarget::Block(id) => id.to_string(),
    };
    let block = matches!(target, ReminderTarget::Block(_));
    Ok(ReminderList {
        announcement: match (views.len(), mine) {
            (0, _) => "No reminders".to_owned(),
            (n, true) => capitalised(&count_line(n, "reminder")),
            (n, false) => format!("{}, the defaults", capitalised(&count_line(n, "reminder"))),
        },
        notices: Vec::new(),
        reminders: views,
        actions: crate::actions::add_reminder(&target_id, &target_name(snapshot, target), block),
    })
}
