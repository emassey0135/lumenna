//! When reminders fire: every firing between two instants, from the synced records alone.
//!
//! Every device asks this of its own copy of the store and schedules its own notifications;
//! nothing coordinates them. A firing dismissed on one device is an acknowledgement that
//! syncs, so the others stop too, as soon as they hear of it.
//!
//! **Where a civil time meets a real one** is decided here, once: [`instant`]. A time that a
//! daylight-saving change skips (2:30 on the spring day) is moved forward by the gap, to 3:30;
//! a time that happens twice (1:30 in autumn) is the first of the two. That is RFC 5545's
//! rule, so a calendar export agrees. A block keeps its real length across a change: its end
//! is its start plus its minutes, not a civil time read again. Expansion itself stays in UTC
//! ([`crate::recur`]); only here does an occurrence become an instant.

use std::collections::BTreeMap;

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp, civil};
use uuid::Uuid;

use crate::id::{NodeId, ReminderId};
use crate::model::{Delivery, ReminderAction, ReminderAnchor, ReminderTarget, Trigger};
use crate::recur::RecurError;
use crate::snapshot::Snapshot;

/// One time a reminder goes off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Firing {
    /// The reminder: its own identifier, or for a default, one derived from what it is about,
    /// so a dismissal of it holds on every device.
    pub key: ReminderId,
    /// Whether it is the target's own reminder, rather than a default from settings.
    pub own: bool,
    /// What it is about.
    pub target: ReminderTarget,
    /// Which occurrence: the block's day, or the task's due date.
    pub occurrence: civil::Date,
    /// What it counts from, and by how much.
    pub trigger: Trigger,
    /// When the thing itself happens: the block's start or end, the task's due time.
    pub anchor_at: Timestamp,
    /// When it goes off, after any snooze.
    pub at: Timestamp,
    /// Whether this firing is a snooze coming due.
    pub snoozed: bool,
    /// Whether the task's due date has no time, so `anchor_at` is the all-day hour.
    pub all_day: bool,
}

/// The instant a civil date and time name in `zone`: forward across a skipped hour, the
/// first of a repeated one (RFC 5545).
#[must_use]
pub fn instant(date: civil::Date, time: civil::Time, zone: &TimeZone) -> Timestamp {
    // jiff's `to_zoned` is the "compatible" disambiguation: later in a gap, earlier in a fold.
    let civil = date.to_datetime(time);
    // Only a date past jiff's range fails; read it in UTC rather than lose the firing.
    civil
        .to_zoned(zone.clone())
        .or_else(|_| civil.to_zoned(TimeZone::UTC))
        .map_or(Timestamp::UNIX_EPOCH, |zoned| zoned.timestamp())
}

/// The key a default reminder fires under: stable on every device, from the target and the
/// trigger, so dismissing it anywhere dismisses it everywhere.
#[must_use]
pub fn default_key(target: &ReminderTarget, trigger: &Trigger) -> ReminderId {
    // FNV-1a over a fixed encoding: defined to the bit, so every build and device agrees.
    let mut hash: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u128::from(*byte);
            hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_0000_013b);
        }
    };
    match target {
        ReminderTarget::Task(id) => {
            feed(b"task");
            feed(id.as_uuid().as_bytes());
        }
        ReminderTarget::Block(id) => {
            feed(b"block");
            feed(id.as_uuid().as_bytes());
        }
    }
    let anchor: &[u8] = match &trigger.anchor {
        ReminderAnchor::Due => b"due",
        ReminderAnchor::BlockStart => b"start",
        ReminderAnchor::BlockEnd => b"end",
        ReminderAnchor::Absolute(_) => b"absolute",
    };
    feed(anchor);
    feed(&trigger.offset_mins.to_le_bytes());
    ReminderId::from_uuid(Uuid::from_u128(hash))
}

/// Every firing in `(from, until]`, in time order, for `device` (`None` for a device with no
/// key of its own, such as the Apple Watch, which fires only what every device does).
///
/// What fires: each open task's own reminders, or the default task reminders if it has
/// none; each block occurrence's series' own, or the default block reminders; and each
/// reminder at a fixed moment. A cancelled occurrence fires nothing, a completed or trashed
/// task nothing, a dismissed firing nothing, and a snoozed one when its snooze ends.
///
/// # Errors
///
/// If a block series carries a rule that cannot be expanded.
pub fn firings(
    snapshot: &Snapshot,
    from: Timestamp,
    until: Timestamp,
    local: &TimeZone,
    device: Option<NodeId>,
) -> Result<Vec<Firing>, RecurError> {
    let facts = snapshot.facts();
    let delivered = |delivery: &Delivery| match device {
        Some(device) => delivery.includes(&device),
        None => matches!(delivery, Delivery::AllDevices),
    };
    // Each target's own reminders, live and for this device.
    let mut own: BTreeMap<ReminderTarget, Vec<(ReminderId, &Trigger)>> = BTreeMap::new();
    let mut absolute = Vec::new();
    for reminder in snapshot.reminders.values().filter(|r| r.deleted_at.is_none()) {
        if !delivered(&reminder.delivery) {
            // Not this device's to fire, but still the target's own: it keeps the defaults off.
            own.entry(reminder.target).or_default();
            continue;
        }
        match &reminder.trigger.anchor {
            ReminderAnchor::Absolute(at) => {
                own.entry(reminder.target).or_default();
                absolute.push((reminder.id, reminder.target, &reminder.trigger, at.timestamp(), at.date()));
            }
            _ => own.entry(reminder.target).or_default().push((reminder.id, &reminder.trigger)),
        }
    }
    let triggers = |target: ReminderTarget, defaults: &[Trigger]| -> Vec<(ReminderId, Trigger, bool)> {
        match own.get(&target) {
            Some(list) => list.iter().map(|(id, t)| (*id, (*t).clone(), true)).collect(),
            None => defaults.iter().map(|t| (default_key(&target, t), t.clone(), false)).collect(),
        }
    };

    let mut found = Vec::new();
    let mut consider = |firing: Firing| {
        let key = (firing.key, firing.occurrence);
        let mut firing = firing;
        match snapshot.acks.get(&key).map(|ack| &ack.action) {
            Some(ReminderAction::Dismissed) => return,
            Some(ReminderAction::Snoozed { until: again }) => {
                firing.at = again.timestamp();
                firing.snoozed = true;
            }
            None => {}
        }
        if firing.at > from && firing.at <= until {
            found.push(firing);
        }
    };
    let offset = |trigger: &Trigger| SignedDuration::from_mins(i64::from(trigger.offset_mins));
    // How far back and forward to look for what fires in the window: the largest offset, a
    // day either side for zones and blocks that run past midnight, and a snooze's reach.
    let reach_mins = snapshot
        .reminders
        .values()
        .map(|r| i64::from(r.trigger.offset_mins).abs())
        .chain(snapshot.settings.default_task_reminders.iter().map(|t| i64::from(t.offset_mins).abs()))
        .chain(snapshot.settings.default_block_reminders.iter().map(|t| i64::from(t.offset_mins).abs()))
        .max()
        .unwrap_or(0);
    let margin = SignedDuration::from_mins(reach_mins) + SignedDuration::from_hours(48);
    let first = from.saturating_sub(margin).unwrap_or(from).to_zoned(local.clone()).date();
    let last = until.saturating_add(margin).unwrap_or(until).to_zoned(local.clone()).date();
    // Snoozes carry a firing past its own window; look across every acknowledged date too.
    let snoozed_dates = snapshot.acks.values().filter_map(|ack| match &ack.action {
        ReminderAction::Snoozed { until: again } if again.timestamp() > from && again.timestamp() <= until => {
            Some(ack.occurrence_date)
        }
        _ => None,
    });
    let first = snoozed_dates.fold(first, std::cmp::Ord::min);

    // Tasks: each open one with a due date, at its current occurrence.
    for task in snapshot.tasks.values().filter(|t| !t.is_deleted() && !facts.is_completed(t)) {
        let Some(due) = &task.due else { continue };
        if due.date < first || due.date > last {
            continue;
        }
        let zone = due.timezone.as_ref().and_then(|name| name.get().ok()).unwrap_or_else(|| local.clone());
        let (time, all_day) = due.time.map_or((snapshot.settings.all_day_reminder_hour, true), |t| (t, false));
        let anchor_at = instant(due.date, time, &zone);
        let target = ReminderTarget::Task(task.id);
        for (key, trigger, is_own) in triggers(target, &snapshot.settings.default_task_reminders) {
            if !matches!(trigger.anchor, ReminderAnchor::Due) {
                continue;
            }
            let at = anchor_at.saturating_add(offset(&trigger)).unwrap_or(anchor_at);
            consider(Firing { key, own: is_own, target, occurrence: due.date, trigger, anchor_at, at, snoozed: false, all_day });
        }
    }

    // Blocks: every occurrence in reach; a cancelled one is not expanded at all.
    for occurrence in snapshot.occurrences(&(first..=last))? {
        let Some(series) = snapshot.series.get(&occurrence.series_id) else { continue };
        if series.deleted_at.is_some() {
            continue;
        }
        let zone = series.timezone.as_ref().and_then(|name| name.get().ok()).unwrap_or_else(|| local.clone());
        let start = instant(occurrence.date, occurrence.start_time, &zone);
        // Its real length, across a change of the clocks too.
        let end = start
            .saturating_add(SignedDuration::from_mins(i64::from(occurrence.duration_mins)))
            .unwrap_or(start);
        let target = ReminderTarget::Block(occurrence.series_id);
        for (key, trigger, is_own) in triggers(target, &snapshot.settings.default_block_reminders) {
            let anchor_at = match trigger.anchor {
                ReminderAnchor::BlockStart => start,
                ReminderAnchor::BlockEnd => end,
                _ => continue,
            };
            let at = anchor_at.saturating_add(offset(&trigger)).unwrap_or(anchor_at);
            consider(Firing {
                key,
                own: is_own,
                target,
                occurrence: occurrence.date,
                trigger,
                anchor_at,
                at,
                snoozed: false,
                all_day: false,
            });
        }
    }

    // Reminders at a fixed moment, while what they are about still stands.
    for (key, target, trigger, at, date) in absolute {
        let stands = match target {
            ReminderTarget::Task(id) => snapshot.tasks.get(&id).is_some_and(|t| !t.is_deleted() && !facts.is_completed(t)),
            ReminderTarget::Block(id) => snapshot.series.get(&id).is_some_and(|s| s.deleted_at.is_none()),
        };
        if stands {
            consider(Firing {
                key,
                own: true,
                target,
                occurrence: date,
                trigger: trigger.clone(),
                anchor_at: at,
                at,
                snoozed: false,
                all_day: false,
            });
        }
    }

    found.sort_by_key(|f| (f.at, f.key));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(name: &str) -> TimeZone {
        TimeZone::get(name).unwrap()
    }

    #[test]
    fn a_skipped_time_moves_forward_by_the_gap_and_a_repeated_one_is_the_first() {
        let new_york = zone("America/New_York");
        // 8 March 2026: 2:00 becomes 3:00.
        let spring = instant(civil::date(2026, 3, 8), civil::time(2, 30, 0, 0), &new_york);
        assert_eq!(spring.to_zoned(new_york.clone()).time(), civil::time(3, 30, 0, 0));
        // 1 November 2026: 1:00 to 2:00 happens twice; the first is daylight time.
        let autumn = instant(civil::date(2026, 11, 1), civil::time(1, 30, 0, 0), &new_york);
        assert_eq!(autumn.to_zoned(new_york).offset().seconds(), -4 * 3600);
    }

    #[test]
    fn a_defaults_key_is_the_same_wherever_it_is_worked_out() {
        let target = ReminderTarget::Block(crate::id::SeriesId::from_uuid(Uuid::from_u128(7)));
        let trigger = Trigger::before(ReminderAnchor::BlockStart, 5);
        assert_eq!(default_key(&target, &trigger), default_key(&target, &trigger));
        assert_ne!(default_key(&target, &trigger), default_key(&target, &Trigger::at(ReminderAnchor::BlockEnd)));
        assert_eq!(
            default_key(&target, &trigger).to_string(),
            "c3c295f8-4bf6-c7d5-91c8-920a7ca0b8e1",
            "fixed forever: a dismissal synced from an older build must still match"
        );
    }
}
