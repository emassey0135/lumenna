//! Reminders: what fires and what it says, setting them, and dealing with one.
//!
//! The core works out *when* ([`lumenna_core::remind`]); this words each firing as the whole
//! message ("Deep work starts in 5 minutes — 3 tasks assigned, first is Review PR"), never
//! "Upcoming block", and answers the two questions apps ask: what is due since they last
//! looked, for a process that runs (the desktop apps, `lum notify`, BTSpeak's service), and
//! what fires over the next while, for a system that schedules notifications itself (iOS,
//! Android). Delivery is each app's; deciding, wording and acknowledging are here.

use jiff::{SignedDuration, Timestamp, Zoned};
use lumenna_core::edit;
use lumenna_core::id::{NodeId, ReminderId};
use lumenna_core::model::{
    Delivery, Reminder, ReminderAck, ReminderAction, ReminderAnchor, ReminderTarget, Trigger, Verbosity,
};
use lumenna_core::remind::{Firing, firings};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::words::{count_line, duration};
use crate::{Lumenna, repaired, resolve};
use lumenna_records::reminders::{gap, reminder_target, target_name, trigger_words};
use lumenna_records::words::capitalised;
pub use lumenna_records::reminders::{DueReminder, DueReminders, ReminderList, ReminderView, SNOOZE_MINS, STALE_MINS};




/// A firing's whole message, and the same said of the past.
fn words(firing: &Firing, snapshot: &Snapshot, verbosity: Verbosity) -> (String, String, String) {
    let (span, when) = gap(firing.trigger.offset_mins);
    let title = match firing.target {
        ReminderTarget::Task(id) => snapshot.tasks.get(&id).map(|t| t.title.clone()),
        ReminderTarget::Block(id) => snapshot.series.get(&id).map(|s| s.title.clone()),
    }
    .unwrap_or_else(|| "A reminder".to_owned());
    use std::cmp::Ordering::{Equal, Greater, Less};
    if let Some(overrun) = firing.overrun {
        let said = format!(
            "{title} has {} left and {} of work planned, more than it holds",
            duration(overrun.left),
            duration(overrun.work)
        );
        return (title.clone(), said, format!("{title} ran short of time"));
    }
    let (said, missed) = match (&firing.trigger.anchor, when) {
        (ReminderAnchor::BlockStart, Greater) => (format!("{title} starts in {span}"), format!("{title} started")),
        (ReminderAnchor::BlockStart, Equal) => (format!("{title} is starting"), format!("{title} started")),
        (ReminderAnchor::BlockStart, Less) => (format!("{title} started {span} ago"), format!("{title} started")),
        (ReminderAnchor::BlockEnd, Greater) => (format!("{title} ends in {span}"), format!("{title} ended")),
        (ReminderAnchor::BlockEnd, Equal) => (format!("{title} is ending"), format!("{title} ended")),
        (ReminderAnchor::BlockEnd, Less) => (format!("{title} ended {span} ago"), format!("{title} ended")),
        (ReminderAnchor::Due, _) if firing.all_day => {
            let day = lumenna_core::time::day_words(firing.occurrence, firing.at.to_zoned(jiff::tz::TimeZone::system()).date());
            (format!("{title} is due {day}"), format!("{title} was due"))
        }
        (ReminderAnchor::Due, Greater) => (format!("{title} is due in {span}"), format!("{title} was due")),
        (ReminderAnchor::Due, Equal) => (format!("{title} is due now"), format!("{title} was due")),
        (ReminderAnchor::Due, Less) => (format!("{title} was due {span} ago"), format!("{title} was due")),
        (ReminderAnchor::Absolute(_), _) => (format!("Reminder: {title}"), format!("Reminder: {title}")),
    };
    // What a block holds, said with its start in full: the point of being told is what to do.
    let detail = match (&firing.trigger.anchor, &firing.target, verbosity) {
        (ReminderAnchor::BlockStart, ReminderTarget::Block(series), Verbosity::Full) => {
            sittings_detail(snapshot, *series, firing.occurrence)
        }
        _ => None,
    };
    let text = detail.map_or_else(|| said.clone(), |detail| format!("{said} — {detail}"));
    (title, text, missed)
}

/// What a block's end says of the work it left short, and whether any of it can go on.
fn carry_words(firing: &Firing, snapshot: &Snapshot) -> Option<(String, bool)> {
    let ReminderTarget::Block(series) = firing.target else { return None };
    if !firing.carry {
        return None;
    }
    let now = firing.at.to_zoned(jiff::tz::TimeZone::system());
    let offers = crate::carry::offers(snapshot, series, firing.occurrence, &now);
    let words = crate::carry::offer_words(&offers, now.date())?;
    Some((words, offers.iter().any(|o| o.to.is_some())))
}

/// "3 tasks assigned, first is Review PR", or nothing for a block that takes no tasks.
fn sittings_detail(snapshot: &Snapshot, series: lumenna_core::SeriesId, date: jiff::civil::Date) -> Option<String> {
    let block = snapshot.series.get(&series)?;
    if !block.flags.accepts_tasks {
        return None;
    }
    let reference = if block.is_recurring() {
        lumenna_core::model::BlockRef::Occurrence(series, date)
    } else {
        lumenna_core::model::BlockRef::OneOff(series)
    };
    let mut assigned: Vec<_> = snapshot.assignments.values().filter(|a| a.block_ref == reference).collect();
    assigned.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
    let first = assigned.first().and_then(|a| snapshot.tasks.get(&a.task_id)).map(|t| t.title.clone());
    Some(match first {
        None => "nothing assigned yet".to_owned(),
        Some(first) if assigned.len() == 1 => format!("1 task assigned, {first}"),
        Some(first) => format!("{} assigned, first is {first}", count_line(assigned.len(), "task")),
    })
}

fn notification(firing: &Firing, snapshot: &Snapshot, verbosity: Verbosity) -> DueReminder {
    let (title, text, missed) = words(firing, snapshot, verbosity);
    let carried = carry_words(firing, snapshot);
    let text = match &carried {
        Some((offer, _)) => format!("{}. {offer}", text.trim_end_matches('.')),
        None => text,
    };
    let (target, kind) = match firing.target {
        ReminderTarget::Task(id) => (id.to_string(), "task"),
        ReminderTarget::Block(id) => (id.to_string(), "block"),
    };
    DueReminder {
        key: firing.key.to_string(),
        occurrence: firing.occurrence.to_string(),
        at: firing.at.to_string(),
        target,
        kind: kind.to_owned(),
        title,
        text,
        missed,
        dismiss: "Dismiss".to_owned(),
        snooze: format!("Snooze {}", crate::actions::title_case(&duration(SNOOZE_MINS))),
        carry: if carried.is_some_and(|(_, movable)| movable) { "Carry Forward".to_owned() } else { String::new() },
        replan: if firing.overrun.is_some() { "Replan".to_owned() } else { String::new() },
    }
}

fn timestamp(text: &str) -> Result<Timestamp> {
    text.trim()
        .parse::<Timestamp>()
        .map_err(|e| LumennaError::new(format!("'{text}' is not a moment in time, such as 2026-10-10T14:00:00Z ({e})")))
}

impl Lumenna {
    /// Whether reminders are on for this device: its own setting, never synced.
    fn reminders_on(&self) -> bool {
        self.device_setting("reminders").is_none_or(|v| v != "false")
    }

    /// This device's key, where it has one; the watch has none and fires what every device does.
    fn reminder_device(&self) -> Option<NodeId> {
        #[cfg(feature = "sync")]
        {
            crate::sync::this_node(&self.shared()).ok()
        }
        #[cfg(not(feature = "sync"))]
        {
            None
        }
    }

    fn notifications(&self, from: Timestamp, until: Timestamp, now: Timestamp) -> Result<DueReminders> {
        if !self.reminders_on() {
            return Ok(DueReminders {
                announcement: "Reminders are off on this device".to_owned(),
                notices: Vec::new(),
                due: Vec::new(),
                missed: Vec::new(),
                missed_summary: String::new(),
                elsewhere: false,
            });
        }
        let device = self.reminder_device();
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let verbosity = snapshot.settings.verbosity;
            let lumenna_core::remind::Firings { firings: fired, unreadable } =
                firings(&snapshot, from, until, &jiff::tz::TimeZone::system(), device);
            let notices = unreadable.into_iter().map(|series| lumenna_records::reminders::unreadable_words(&snapshot, series)).collect();
            let stale = now.checked_sub(SignedDuration::from_mins(STALE_MINS)).unwrap_or(now);
            let (missed, due): (Vec<_>, Vec<_>) = fired
                .iter()
                .map(|firing| (firing.at < stale, notification(firing, &snapshot, verbosity)))
                .partition(|(old, _)| *old);
            let missed: Vec<DueReminder> = missed.into_iter().map(|(_, n)| n).collect();
            let due: Vec<DueReminder> = due.into_iter().map(|(_, n)| n).collect();
            let missed_summary = if missed.is_empty() {
                String::new()
            } else {
                format!(
                    "{} missed: {}.",
                    capitalised(&count_line(missed.len(), "reminder")),
                    missed.iter().map(|n| n.missed.as_str()).collect::<Vec<_>>().join(", ")
                )
            };
            Ok(DueReminders {
                announcement: count_line(due.len() + missed.len(), "reminder"),
                notices,
                due,
                missed,
                missed_summary,
                elsewhere: false,
            })
        })
    }
}

impl Lumenna {
    /// [`Lumenna::reminders_missed`] at a given moment, for tests.
    #[doc(hidden)]
    pub fn missed_at(&self, shown: Vec<String>, kept: bool, now: Timestamp) -> Result<DueReminders> {
        let last = self.device_setting("reminders-caught-up").and_then(|t| t.parse::<Timestamp>().ok());
        self.write_device_setting("reminders-caught-up", Some(&now.to_string()))?;
        let empty = || DueReminders {
            announcement: "No reminders".to_owned(),
            notices: Vec::new(),
            due: Vec::new(),
            missed: Vec::new(),
            missed_summary: String::new(),
            elsewhere: false,
        };
        let Some(last) = last else { return Ok(empty()) };
        // Never further back than a day, as delivery does.
        let since = last.max(now.checked_sub(SignedDuration::from_hours(24)).unwrap_or(now));
        let until = now.checked_sub(SignedDuration::from_mins(1)).unwrap_or(now);
        if until <= since {
            return Ok(empty());
        }
        let armed = self.device_setting("reminders-armed-until").and_then(|t| t.parse::<Timestamp>().ok());
        let mut found = self.notifications(since, until, now)?;
        let given = |n: &DueReminder| {
            let at = n.at.parse::<Timestamp>().ok();
            shown.iter().any(|s| *s == format!("{}@{}", n.key, n.occurrence))
                || (kept && matches!((at, armed), (Some(at), Some(armed)) if at <= armed))
        };
        found.due.retain(|n| !given(n));
        found.missed.retain(|n| !given(n));
        found.missed_summary = if found.missed.is_empty() {
            String::new()
        } else {
            format!(
                "{} missed: {}.",
                capitalised(&count_line(found.missed.len(), "reminder")),
                found.missed.iter().map(|n| n.missed.as_str()).collect::<Vec<_>>().join(", ")
            )
        };
        found.announcement = count_line(found.due.len() + found.missed.len(), "reminder");
        Ok(found)
    }
}


impl Lumenna {
    /// The reminders that came due after `since` (RFC 3339) up to `now` (now when absent):
    /// what a running process asks each time it looks. Those from more than a few minutes
    /// ago come as `missed`, to show as one summary, not one by one.
    ///
    /// # Errors
    ///
    /// If a moment cannot be read, or the store cannot.
    pub fn reminders_due(&self, since: String, now: Option<String>) -> Result<DueReminders> {
        let now = now.map_or_else(|| Ok(Timestamp::now()), |n| timestamp(&n))?;
        self.notifications(timestamp(&since)?, now, now)
    }

    /// Every reminder from now until `until` (RFC 3339): what a system that schedules
    /// notifications itself is handed, and asks again when the store changes.
    ///
    /// # Errors
    ///
    /// If `until` cannot be read, or the store cannot.
    pub fn upcoming_reminders(&self, until: String) -> Result<DueReminders> {
        let now = Timestamp::now();
        // A moment ago, so a firing exactly now is still handed over.
        let from = now.checked_sub(SignedDuration::from_secs(1)).unwrap_or(now);
        let until = timestamp(&until)?;
        let mut found = self.notifications(from, until, from)?;
        // How far ahead the system was given reminders, for `reminders_missed` to know what
        // it may have missed.
        self.write_device_setting("reminders-armed-until", Some(&until.to_string()))?;
        found.due.append(&mut found.missed);
        found.missed_summary.clear();
        Ok(found)
    }

    /// What a system that schedules notifications itself missed since this was last asked,
    /// to say on launch, on returning to the front, and after a restart: the device was off,
    /// or the app went unopened past the window it last handed over. Recent ones come as
    /// `due`, older ones as one summary; the first time, nothing.
    ///
    /// `kept` says the system shows what it was handed even after a restart (iOS, watchOS), so
    /// anything within the window last handed over counts as given. Otherwise (Android, Wear
    /// OS, whose alarms a restart clears) `shown` names what the app did show, each as
    /// `<key>@<occurrence>`, and every other firing since counts as missed. A firing in the
    /// last minute is left to the system, which may be showing it now.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn reminders_missed(&self, shown: Vec<String>, kept: bool) -> Result<DueReminders> {
        self.missed_at(shown, kept, Timestamp::now())
    }

    /// The reminders a task or block has: its own, or the defaults it takes.
    ///
    /// # Errors
    ///
    /// If nothing matches `target`.
    pub fn reminders_of(&self, target: String) -> Result<ReminderList> {
        self.with(|store| {
            store.load_all_years()?;
            lumenna_records::reminders::reminders_of(&repaired(store), &target)
        })
    }

    /// Gives a task or block a reminder of its own: `anchor` is `due` for a task, `start` or
    /// `end` for a block; `offset_mins` negative before, positive after. Its first reminder of
    /// its own replaces the defaults for it.
    ///
    /// # Errors
    ///
    /// If nothing matches `target`, or the anchor means nothing for it.
    pub fn add_reminder(&self, target: String, anchor: String, offset_mins: i32) -> Result<Change> {
        let anchor = match anchor.trim() {
            "due" => ReminderAnchor::Due,
            "start" => ReminderAnchor::BlockStart,
            "end" => ReminderAnchor::BlockEnd,
            other => return Err(LumennaError::new(format!("'{other}' is not due, start or end"))),
        };
        self.add_trigger(&target, Trigger { anchor, offset_mins })
    }

    fn add_trigger(&self, target: &str, trigger: Trigger) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let target = reminder_target(&snapshot, target)?;
            let reminder = Reminder::new(target, trigger.clone()).map_err(|_| {
                LumennaError::new(match target {
                    ReminderTarget::Task(_) => "a task's reminder counts from when it is due, or is at a set moment",
                    ReminderTarget::Block(_) => "a block's reminder counts from when it starts or ends, or is at a set moment",
                })
            })?;
            let name = target_name(&snapshot, target);
            // Terse leaves out the name, which the reminder list it was added from says.
            let said = if snapshot.settings.verbosity == lumenna_core::model::Verbosity::Terse {
                format!("Reminding {}", trigger_words(&trigger))
            } else {
                format!("Reminding of {name} {}", trigger_words(&trigger))
            };
            let change = edit::create_reminder(reminder, said);
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }

    /// Gives a task or block a reminder of its own from a phrase: "5m before start", "at end",
    /// "30m before" (for a task, its due time), or a moment, "tomorrow at 9am".
    ///
    /// # Errors
    ///
    /// If nothing matches `target`, or the phrase does not read.
    pub fn add_reminder_saying(&self, target: String, phrase: String) -> Result<Change> {
        let (block, week_start) = self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            Ok((matches!(reminder_target(&snapshot, &target)?, ReminderTarget::Block(_)), snapshot.settings.week_start))
        })?;
        let trigger = parse_reminder(&phrase, block, &Zoned::now(), week_start)?;
        self.add_trigger(&target, trigger)
    }

    /// Sets which devices one of a task's or block's own reminders fires on: `all`,
    /// `only:<device>` or `except:<device>`, a device by its identifier.
    ///
    /// # Errors
    ///
    /// If no reminder of its own matches `id`, or `choice` names no paired device.
    pub fn deliver_reminder(&self, id: String, choice: String) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id: ReminderId = id.trim().parse().map_err(|_| LumennaError::new(format!("'{id}' is not a reminder of its own")))?;
            let reminder = snapshot.reminders.get(&id).ok_or_else(|| LumennaError::new("no such reminder"))?;
            let device = |node: &str| -> Result<std::collections::BTreeSet<NodeId>> {
                let node: NodeId = node.parse().map_err(|_| LumennaError::new(format!("'{node}' is not a device")))?;
                if !snapshot.devices.contains_key(&node) {
                    return Err(LumennaError::new("that device is not paired"));
                }
                Ok(std::collections::BTreeSet::from([node]))
            };
            let delivery = match choice.split_once(':') {
                None if choice == "all" => Delivery::AllDevices,
                Some(("only", node)) => Delivery::OnlyDevices(device(node)?),
                Some(("except", node)) => Delivery::ExceptDevices(device(node)?),
                _ => return Err(LumennaError::new(format!("'{choice}' is not all, only:<device> or except:<device>"))),
            };
            let name = target_name(&snapshot, reminder.target);
            let place = lumenna_records::reminders::delivery_words(&snapshot, &delivery);
            let place = if place.is_empty() { "on every device".to_owned() } else { place };
            let change = edit::deliver_reminder(
                &snapshot,
                id,
                delivery,
                format!("Reminding of {name} {} {place}", trigger_words(&reminder.trigger)),
            )?;
            crate::tasks::record_or(store, &change, "it fires there already")
        })
    }

    /// Removes one of a task's or block's own reminders.
    ///
    /// # Errors
    ///
    /// If no reminder of its own matches `id`.
    pub fn remove_reminder(&self, id: String) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id: ReminderId = id
                .trim()
                .parse()
                .map_err(|_| LumennaError::new(format!("'{id}' is not a reminder of its own; a default is changed in settings")))?;
            let reminder = snapshot.reminders.get(&id).ok_or_else(|| LumennaError::new("no such reminder"))?;
            let name = target_name(&snapshot, reminder.target);
            let said = if snapshot.settings.verbosity == lumenna_core::model::Verbosity::Terse {
                format!("No longer reminding {}", trigger_words(&reminder.trigger))
            } else {
                format!("No longer reminding of {name} {}", trigger_words(&reminder.trigger))
            };
            let change = edit::trash_reminder(&snapshot, id, said)?;
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }

    /// Dismisses one firing of a reminder (`key` and `occurrence` as its notification gave
    /// them), on every device as soon as they hear of it.
    ///
    /// # Errors
    ///
    /// If `key` or `occurrence` cannot be read.
    pub fn dismiss_reminder(&self, key: String, occurrence: String) -> Result<Change> {
        self.acknowledge(&key, &occurrence, ReminderAction::Dismissed, "Dismissed".to_owned())
    }

    /// Snoozes one firing of a reminder for `minutes` (the notification button's ten when
    /// absent), on every device.
    ///
    /// # Errors
    ///
    /// If `key` or `occurrence` cannot be read, or `minutes` is zero.
    pub fn snooze_reminder(&self, key: String, occurrence: String, minutes: Option<u32>) -> Result<Change> {
        let minutes = minutes.unwrap_or(SNOOZE_MINS);
        if minutes == 0 {
            return Err(LumennaError::new("a snooze lasts at least a minute"));
        }
        let until = Zoned::now()
            .checked_add(SignedDuration::from_mins(i64::from(minutes)))
            .map_err(|e| LumennaError::new(e.to_string()))?;
        self.acknowledge(&key, &occurrence, ReminderAction::Snoozed { until }, format!("Snoozed for {}", duration(minutes)))
    }
}

impl Lumenna {
    fn acknowledge(&self, key: &str, occurrence: &str, action: ReminderAction, said: String) -> Result<Change> {
        let reminder_id: ReminderId = key.trim().parse().map_err(|_| LumennaError::new(format!("'{key}' is not a reminder")))?;
        let occurrence_date: jiff::civil::Date =
            occurrence.trim().parse().map_err(|_| LumennaError::new(format!("'{occurrence}' is not a day")))?;
        let acked_by = self.reminder_device().unwrap_or_else(|| NodeId::from_bytes([0; 32]));
        self.told(|store| {
            let snapshot = repaired(store);
            let ack = ReminderAck { reminder_id, occurrence_date, action, acked_at: lumenna_core::time::now(), acked_by };
            let change = edit::acknowledge(&snapshot, ack, said);
            if change.is_empty() {
                return Ok(Change::unchanged("it was already dealt with"));
            }
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}

/// Held by the one process on this device that gives reminders: two running at once (the
/// Mac app and `lum notify`, say) would each say every one. Dropped, it lets another take
/// over.
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
#[derive(Debug)]
pub struct ReminderLock {
    _file: std::fs::File,
}

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
impl Lumenna {
    /// Takes the right to give this device's reminders, or `None` while another process
    /// holds it; ask again now and then, as a sync service waits its turn.
    ///
    /// # Errors
    ///
    /// If the lock file cannot be opened.
    pub fn take_reminders(&self) -> Result<Option<std::sync::Arc<ReminderLock>>> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.directory.join("reminders.lock"))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(std::sync::Arc::new(ReminderLock { _file: file }))),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
        }
    }

    /// What to give now, for the process holding `lock`: everything since it last gave
    /// reminders (kept on this device, so a restart knows what came while nothing ran), with
    /// what is stale folded into `missed_summary`. The first time, from now. `now` is for
    /// tests (RFC 3339); a running process leaves it out.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn deliver_reminders(&self, lock: std::sync::Arc<ReminderLock>, now: Option<String>) -> Result<DueReminders> {
        let _held = lock;
        let now = now.map_or_else(|| Ok(Timestamp::now()), |n| timestamp(&n))?;
        let since = self
            .device_setting("reminders-delivered")
            .and_then(|text| text.parse::<Timestamp>().ok())
            // Never further back than a day: a laptop shut for a month is not owed a month.
            .map(|last| last.max(now.checked_sub(SignedDuration::from_hours(24)).unwrap_or(now)))
            .unwrap_or(now);
        let found = self.notifications(since, now, now)?;
        self.write_device_setting("reminders-delivered", Some(&now.to_string()))?;
        Ok(found)
    }
}


/// A trigger as the phrase `parse_trigger` reads back: "5m before start", "at end".
#[must_use]
pub fn trigger_phrase(trigger: &Trigger) -> String {
    if let ReminderAnchor::Absolute(at) = &trigger.anchor {
        return at.strftime("on %Y-%m-%d at %H:%M").to_string();
    }
    let anchor = match trigger.anchor {
        ReminderAnchor::BlockStart => "start",
        ReminderAnchor::BlockEnd => "end",
        ReminderAnchor::Due | ReminderAnchor::Absolute(_) => "due",
    };
    let minutes = trigger.offset_mins.unsigned_abs();
    let span = match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m}m"),
    };
    match trigger.offset_mins {
        0 => format!("at {anchor}"),
        n if n < 0 => format!("{span} before {anchor}"),
        _ => format!("{span} after {anchor}"),
    }
}

/// A reminder as a person types it: "5m before start", "at end", "1h after" (for a task the
/// anchor is its due time, which needs no saying), or a moment still to come, "tomorrow at
/// 9am", in `now`'s zone. `block` says which it is for.
///
/// # Errors
///
/// If it does not read, names an anchor the target lacks, or a moment past or without a time.
pub fn parse_reminder(text: &str, block: bool, now: &Zoned, week_start: jiff::civil::Weekday) -> Result<Trigger> {
    let relative = parse_trigger(text, block);
    if relative.is_ok() {
        return relative;
    }
    // Not counted from the block or the due time: a moment of its own, "tomorrow at 9am",
    // "on friday at 15:00". It needs a time; a day alone says nothing of when.
    let phrase = text.trim();
    let phrase = phrase.strip_prefix("on ").unwrap_or(phrase);
    let Ok(Some(due)) = resolve::due(phrase, now, week_start) else { return relative };
    if due.recurrence.is_some() {
        return Err(LumennaError::new("a reminder at a set moment happens once; give a day and a time, such as tomorrow at 9am"));
    }
    let Some(time) = due.time else {
        return Err(LumennaError::new(format!("say a time as well as the day, such as {} at 9am", phrase)));
    };
    let at = due
        .date
        .to_datetime(time)
        .to_zoned(now.time_zone().clone())
        .map_err(|e| LumennaError::new(e.to_string()))?;
    if at.timestamp() <= now.timestamp() {
        return Err(LumennaError::new("that moment has passed; give one still to come"));
    }
    Ok(Trigger { anchor: ReminderAnchor::Absolute(at), offset_mins: 0 })
}

/// A reminder counted from what it is about, as [`parse_reminder`] reads it without a moment:
/// the defaults' phrases are only these.
///
/// # Errors
///
/// If it does not read, or names an anchor the target lacks.
pub fn parse_trigger(text: &str, block: bool) -> Result<Trigger> {
    let lower = text.trim().to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let anchor = |word: Option<&&str>| -> Result<ReminderAnchor> {
        match (word.copied(), block) {
            (None, true) | (Some("start" | "starts"), true) => Ok(ReminderAnchor::BlockStart),
            (Some("end" | "ends"), true) => Ok(ReminderAnchor::BlockEnd),
            (None, false) | (Some("due"), false) => Ok(ReminderAnchor::Due),
            (Some(other), true) => Err(LumennaError::new(format!("a block's reminder counts from its start or end, not '{other}'"))),
            (Some(other), false) => Err(LumennaError::new(format!("a task's reminder counts from when it is due, not '{other}'"))),
        }
    };
    let example = if block { "such as 5m before start, at end, or tomorrow at 9am" } else { "such as 30m before, at due, or tomorrow at 9am" };
    match words.as_slice() {
        ["at", rest @ ..] if rest.len() <= 1 => Ok(Trigger { anchor: anchor(rest.first())?, offset_mins: 0 }),
        [span, way @ ("before" | "after"), rest @ ..] if rest.len() <= 1 => {
            let minutes = i32::try_from(resolve::minutes(span)?).unwrap_or(i32::MAX);
            let offset_mins = if *way == "before" { -minutes } else { minutes };
            Ok(Trigger { anchor: anchor(rest.first())?, offset_mins })
        }
        _ => Err(LumennaError::new(format!("'{}' is not a reminder; {example}", text.trim()))),
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trigger_reads_as_a_person_says_it() {
        assert_eq!(trigger_words(&Trigger::before(ReminderAnchor::BlockStart, 5)), "5 minutes before it starts");
        assert_eq!(trigger_words(&Trigger::at(ReminderAnchor::BlockEnd)), "when it ends");
        assert_eq!(trigger_words(&Trigger::after(ReminderAnchor::Due, 90)), "1 hour 30 minutes after it is due");
    }
}
