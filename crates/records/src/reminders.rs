//! Reminders as records: what a notification shows, and a task's or block's list of them.

use lumenna_core::id::ReminderId;
use lumenna_core::model::{Delivery, Reminder, ReminderAnchor, ReminderTarget, Trigger};
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
    pub carry: String,
    /// When a block's planned work outruns its time: the button that opens Replan from Now.
    /// Empty when there is none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub replan: String,
}

/// What reminders to give now, or over a while.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl DueReminders {
    /// Nothing given, since another process on this device gives reminders: ask again soon.
    #[must_use]
    pub fn elsewhere() -> Self {
        Self {
            announcement: "Another app on this device gives reminders".to_owned(),
            notices: Vec::new(),
            due: Vec::new(),
            missed: Vec::new(),
            missed_summary: String::new(),
            elsewhere: true,
        }
    }
}

/// One reminder a task or block has, its own or a default.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// For `absolute`, its day, ISO, and its time, `HH:MM`, in the moment's own zone: for the
    /// app to say in its clock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<String>,
    /// See `day`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    /// Where it fires, when that is not every device: "only on Kitchen iPad". Empty for all.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub delivery: String,
    /// How it reads: "5 minutes before it starts".
    pub words: String,
    /// What can be done to it: Remove, for one of its own; nothing for a default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<crate::actions::Action>,
    /// What can be done to it, in short, as a view offers it (see [`crate::actions::Offered`]);
    /// empty outside a view, where `actions` are whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<crate::actions::Offered>,
}

/// The reminders a task or block has.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// What can be done to the list, in short, as a view offers it (see [`crate::actions::Offered`]);
    /// empty outside a view, where `actions` are whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<crate::actions::Offered>,
}

/// "in 5 minutes", "now", "5 minutes ago": the gap between a firing and what it is about.
pub fn gap(offset_mins: i32) -> (String, std::cmp::Ordering) {
    let minutes = offset_mins.unsigned_abs();
    (duration(minutes), 0.cmp(&offset_mins))
}

/// What a trigger reads as on a target's list.
pub fn trigger_words(trigger: &Trigger) -> String {
    let (span, when) = gap(trigger.offset_mins);
    let (verb, at) = match &trigger.anchor {
        ReminderAnchor::BlockStart => ("it starts", "when it starts"),
        ReminderAnchor::BlockEnd => ("it ends", "when it ends"),
        ReminderAnchor::Due => ("it is due", "when it is due"),
        ReminderAnchor::Absolute(at) => return at.strftime("on %A %-d %B at %H:%M").to_string(),
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

/// Where a reminder fires, in words; empty for every device.
pub fn delivery_words(snapshot: &Snapshot, delivery: &Delivery) -> String {
    let names = |set: &std::collections::BTreeSet<lumenna_core::id::NodeId>| {
        let names: Vec<String> =
            set.iter().map(|id| snapshot.devices.get(id).map_or_else(|| "a device not paired now".to_owned(), |d| d.name.clone())).collect();
        crate::words::listed(&names)
    };
    match delivery {
        Delivery::AllDevices => String::new(),
        Delivery::OnlyDevices(set) => format!("only on {}", names(set)),
        Delivery::ExceptDevices(set) => format!("on every device but {}", names(set)),
    }
}

/// What a reminder's Deliver On offers: every device, then each paired device alone and each
/// left out, the way it fires now marked.
pub fn delivery_choices(snapshot: &Snapshot, delivery: &Delivery) -> Vec<crate::actions::Choice> {
    let now = |id: &str| {
        let current = match delivery {
            Delivery::AllDevices => "all".to_owned(),
            Delivery::OnlyDevices(set) if set.len() == 1 => format!("only:{}", set.iter().next().map(ToString::to_string).unwrap_or_default()),
            Delivery::ExceptDevices(set) if set.len() == 1 => format!("except:{}", set.iter().next().map(ToString::to_string).unwrap_or_default()),
            _ => String::new(),
        };
        (current == id).then(|| "as it is now".to_owned())
    };
    let mut choices = vec![crate::actions::Choice { id: "all".to_owned(), title: "Every device".to_owned(), detail: now("all"), ..Default::default() }];
    for (way, words) in [("only", "Only on"), ("except", "Every device but")] {
        for device in snapshot.devices.values() {
            let id = format!("{way}:{}", device.node_id);
            choices.push(crate::actions::Choice { detail: now(&id), id, title: format!("{words} {}", device.name), ..Default::default() });
        }
    }
    choices
}

/// One reminder as its list shows it.
pub fn view(snapshot: &Snapshot, id: ReminderId, trigger: &Trigger, delivery: Option<&Delivery>) -> ReminderView {
    let words = trigger_words(trigger);
    let own = delivery.is_some();
    let devices = snapshot.devices.len() > 1;
    ReminderView {
        actions: if own { crate::actions::reminder(&id.to_string(), &words, devices) } else { Vec::new() },
        offered: Vec::new(),
        delivery: delivery.map(|d| delivery_words(snapshot, d)).unwrap_or_default(),
        id: id.to_string(),
        own,
        anchor: anchor_word(&trigger.anchor).to_owned(),
        offset_mins: trigger.offset_mins,
        at: match &trigger.anchor {
            ReminderAnchor::Absolute(at) => Some(at.timestamp().to_string()),
            _ => None,
        },
        day: match &trigger.anchor {
            ReminderAnchor::Absolute(at) => Some(at.date().to_string()),
            _ => None,
        },
        time: match &trigger.anchor {
            ReminderAnchor::Absolute(at) => Some(at.time().strftime("%H:%M").to_string()),
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
    // A task with no due date has nothing for the defaults to count from.
    let due = match target {
        ReminderTarget::Task(id) => snapshot.tasks.get(&id).is_some_and(|t| t.due.is_some()),
        ReminderTarget::Block(_) => true,
    };
    let views: Vec<ReminderView> = if own.is_empty() {
        let none = Vec::new();
        let defaults = match target {
            ReminderTarget::Task(_) if !due => &none,
            ReminderTarget::Task(_) => &snapshot.settings.default_task_reminders,
            ReminderTarget::Block(_) => &snapshot.settings.default_block_reminders,
        };
        defaults
            .iter()
            .map(|trigger| view(snapshot, lumenna_core::remind::default_key(&target, trigger), trigger, None))
            .collect()
    } else {
        own.iter().map(|r| view(snapshot, r.id, &r.trigger, Some(&r.delivery))).collect()
    };
    let mine = views.first().is_some_and(|v| v.own);
    let target_id = match target {
        ReminderTarget::Task(id) => id.to_string(),
        ReminderTarget::Block(id) => id.to_string(),
    };
    let block = matches!(target, ReminderTarget::Block(_));
    Ok(ReminderList {
        offered: Vec::new(),
        announcement: match (views.len(), mine) {
            (0, _) => "No reminders".to_owned(),
            (n, true) => capitalised(&count_line(n, "reminder")),
            (n, false) => format!("{}, the defaults", capitalised(&count_line(n, "reminder"))),
        },
        notices: if due {
            Vec::new()
        } else {
            vec!["It has no due date, so a reminder for it is a moment of its own, such as tomorrow at 9am.".to_owned()]
        },
        reminders: views,
        actions: crate::actions::add_reminder(&target_id, &target_name(snapshot, target), block, due),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumenna_core::id::NodeId;
    use lumenna_core::model::{Device, Task};

    fn device(byte: u8, name: &str) -> Device {
        Device {
            node_id: NodeId::from_bytes([byte; 32]),
            name: name.to_owned(),
            platform: "test".to_owned(),
            paired_at: jiff::Timestamp::UNIX_EPOCH,
            last_seen: jiff::Timestamp::UNIX_EPOCH,
            schema: 0,
        }
    }

    #[test]
    fn a_reminder_of_its_own_chooses_its_devices_once_there_are_two_and_says_where_it_fires() {
        let mut snapshot = Snapshot::default();
        let task = Task::new(lumenna_core::id::ProjectId::INBOX, "water the plants", lumenna_core::order::OrderKey::middle());
        let target = ReminderTarget::Task(task.id);
        snapshot.tasks.insert(task.id, task.clone());
        let mut reminder = Reminder::new(target, Trigger::at(ReminderAnchor::Due)).unwrap();
        snapshot.reminders.insert(reminder.id, reminder.clone());
        snapshot.devices.insert(NodeId::from_bytes([1; 32]), device(1, "Mac"));
        let offers = |snapshot: &Snapshot| {
            reminders_of(snapshot, &task.id.to_string()).unwrap().reminders[0].actions.iter().map(|a| a.title.clone()).collect::<Vec<_>>()
        };
        assert_eq!(offers(&snapshot), ["Remove Reminder"], "one device has nowhere else to choose");

        snapshot.devices.insert(NodeId::from_bytes([2; 32]), device(2, "Kitchen iPad"));
        assert_eq!(offers(&snapshot), ["Deliver On", "Remove Reminder"]);
        let choices = delivery_choices(&snapshot, &reminder.delivery);
        let titles: Vec<&str> = choices.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["Every device", "Only on Mac", "Only on Kitchen iPad", "Every device but Mac", "Every device but Kitchen iPad"]);
        assert_eq!(choices[0].detail.as_deref(), Some("as it is now"));

        reminder.delivery = Delivery::OnlyDevices([NodeId::from_bytes([2; 32])].into());
        snapshot.reminders.insert(reminder.id, reminder.clone());
        let list = reminders_of(&snapshot, &task.id.to_string()).unwrap();
        assert_eq!(list.reminders[0].delivery, "only on Kitchen iPad");
        let choices = delivery_choices(&snapshot, &reminder.delivery);
        assert_eq!(choices.iter().find(|c| c.detail.is_some()).unwrap().title, "Only on Kitchen iPad");
    }
}
