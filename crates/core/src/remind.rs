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
    /// Whether, at a block's end, sittings in it fell short of their time with their tasks
    /// still open, so the rest can be carried forward: said with the end, or fired by itself.
    pub carry: bool,
    /// Whether this is a block's work outrunning its time, and by how much at `at`.
    pub overrun: Option<Overrun>,
}

/// A block's planned work outrunning the time left in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overrun {
    /// The minutes left in the block.
    pub left: u32,
    /// The minutes of planned work still to do in it.
    pub work: u32,
}

/// Below this, an overrun is not worth saying: a few minutes over is ordinary.
pub const FEW_MINS: i64 = 5;

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

/// The zone a due time or a block is held to, else the device's own: what every instant
/// made of one is made in.
#[must_use]
pub fn zone_of(held: Option<&crate::model::TzName>, local: &TimeZone) -> TimeZone {
    held.and_then(|name| name.get().ok()).unwrap_or_else(|| local.clone())
}

/// What the reminder `id` is for: an explicit reminder's target, or the task or block whose
/// default reminder, carried-forward offer or overrun warning fires under `id`, with the default
/// reminders as they are now. `None` when nothing in `snapshot` fires under it.
#[must_use]
pub fn target_of(snapshot: &Snapshot, id: ReminderId) -> Option<ReminderTarget> {
    if let Some(reminder) = snapshot.reminders.get(&id) {
        return Some(reminder.target);
    }
    let settings = &snapshot.settings;
    let task = snapshot
        .tasks
        .keys()
        .map(|task| ReminderTarget::Task(*task))
        .find(|target| settings.default_task_reminders.iter().any(|trigger| default_key(target, trigger) == id));
    task.or_else(|| {
        snapshot.series.keys().find_map(|series| {
            let target = ReminderTarget::Block(*series);
            let fires = carry_key(*series) == id
                || overrun_key(*series) == id
                || settings.default_block_reminders.iter().any(|trigger| default_key(&target, trigger) == id);
            fires.then_some(target)
        })
    })
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

/// The key a block's end fires under when it fires only to offer carrying work forward: fixed
/// forever, as [`default_key`] is.
#[must_use]
pub fn carry_key(series: crate::id::SeriesId) -> ReminderId {
    default_key(&ReminderTarget::Block(series), &Trigger { anchor: ReminderAnchor::BlockEnd, offset_mins: i32::MIN })
}

/// The key a block's overrun warning fires under: fixed forever, as [`default_key`] is.
#[must_use]
pub fn overrun_key(series: crate::id::SeriesId) -> ReminderId {
    default_key(&ReminderTarget::Block(series), &Trigger { anchor: ReminderAnchor::BlockEnd, offset_mins: i32::MIN + 1 })
}

/// When `block` (from `start` to `end`) first has more planned work left than time, by more
/// than `threshold` minutes, and by how much then; `None` when it never does.
///
/// The work left is each open sitting's planned time less what is logged. With no timer
/// running the shortfall grows a minute a minute, so its moment is fixed by the records;
/// with one running, work and time shrink together and the shortfall holds, so it either
/// already outruns from when the timer started or never does. A plan too big from the start
/// is said as the block starts.
#[must_use]
pub fn overrun_at(
    snapshot: &Snapshot,
    facts: &crate::state::Facts<'_>,
    block: crate::model::BlockRef,
    start: Timestamp,
    end: Timestamp,
    threshold_mins: i64,
) -> Option<(Timestamp, Overrun)> {
    use crate::model::AssignmentStatus as S;
    let sittings: Vec<_> = snapshot
        .assignments
        .values()
        .filter(|a| {
            a.block_ref == block
                && matches!(a.status, S::Planned | S::InProgress)
                && snapshot.tasks.get(&a.task_id).is_some_and(|t| !t.is_deleted() && !facts.is_completed(t))
        })
        .collect();
    let work: i64 = sittings
        .iter()
        .map(|a| i64::from(a.planned_mins.unwrap_or(0).saturating_sub(a.accumulated_mins)))
        .sum();
    if work == 0 {
        return None;
    }
    let mins = |minutes: i64| SignedDuration::from_mins(minutes);
    let at = match sittings.iter().filter_map(|a| a.running_since).min() {
        None => end.checked_sub(mins(work - threshold_mins)).ok()?.max(start),
        Some(since) => {
            let since = since.max(start);
            let shortfall = work - end.duration_since(since).as_mins();
            if shortfall <= threshold_mins {
                return None;
            }
            since
        }
    };
    if at >= end {
        return None;
    }
    let left = end.duration_since(at).as_mins();
    let done_since = sittings.iter().filter_map(|a| a.running_since).min().map_or(0, |since| at.duration_since(since.max(start)).as_mins());
    let overrun = Overrun {
        left: u32::try_from(left).unwrap_or(0),
        work: u32::try_from((work - done_since).max(0)).unwrap_or(0),
    };
    Some((at, overrun))
}

/// Whether `block`'s sittings include one left short of its planned time, its task still open.
#[must_use]
pub fn falls_short(snapshot: &Snapshot, facts: &crate::state::Facts<'_>, block: crate::model::BlockRef) -> bool {
    use crate::model::AssignmentStatus as S;
    snapshot.assignments.values().any(|a| {
        a.block_ref == block
            && matches!(a.status, S::Planned | S::InProgress | S::Worked)
            && a.planned_mins.is_some_and(|planned| a.accumulated_mins < planned)
            && snapshot.tasks.get(&a.task_id).is_some_and(|t| !t.is_deleted() && !facts.is_completed(t))
    })
}

/// What fires in a window, and what could not be looked at.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Firings {
    /// Every firing, in time order.
    pub firings: Vec<Firing>,
    /// Each block series whose rule cannot be expanded: none of its reminders are in
    /// `firings`, and the person is told so rather than left to wonder.
    pub unreadable: Vec<crate::id::SeriesId>,
}

/// Every firing in `(from, until]`, in time order, for `device` (`None` for a device with no
/// key of its own, such as the Apple Watch, which fires only what every device does).
///
/// What fires: each open task's own reminders, or the default task reminders if it has
/// none; each block occurrence's series' own, or the default block reminders; and each
/// reminder at a fixed moment. A cancelled occurrence fires nothing, a completed or trashed
/// task nothing, a dismissed firing nothing, and a snoozed one when its snooze ends. A series
/// whose rule cannot be expanded fires nothing and is named in [`Firings::unreadable`]; every
/// other block's reminders still fire.
#[must_use]
pub fn firings(
    snapshot: &Snapshot,
    from: Timestamp,
    until: Timestamp,
    local: &TimeZone,
    device: Option<NodeId>,
) -> Firings {
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
        let zone = zone_of(due.timezone.as_ref(), local);
        let (time, all_day) = due.time.map_or((snapshot.settings.all_day_reminder_hour, true), |t| (t, false));
        let anchor_at = instant(due.date, time, &zone);
        let target = ReminderTarget::Task(task.id);
        for (key, trigger, is_own) in triggers(target, &snapshot.settings.default_task_reminders) {
            if !matches!(trigger.anchor, ReminderAnchor::Due) {
                continue;
            }
            let at = anchor_at.saturating_add(offset(&trigger)).unwrap_or(anchor_at);
            consider(Firing {
                key,
                own: is_own,
                target,
                occurrence: due.date,
                trigger,
                anchor_at,
                at,
                snoozed: false,
                all_day,
                carry: false,
                overrun: None,
            });
        }
    }

    // Blocks: every occurrence in reach; a cancelled one is not expanded at all.
    let (occurrences, unreadable) = crate::recur::expand_readable(&snapshot.series, &snapshot.exceptions, &(first..=last));
    for occurrence in &occurrences {
        let Some(series) = snapshot.series.get(&occurrence.series_id) else { continue };
        if series.deleted_at.is_some() {
            continue;
        }
        let zone = zone_of(series.timezone.as_ref(), local);
        let start = instant(occurrence.date, occurrence.start_time, &zone);
        // Its real length, across a change of the clocks too.
        let end = start
            .saturating_add(SignedDuration::from_mins(i64::from(occurrence.duration_mins)))
            .unwrap_or(start);
        let target = ReminderTarget::Block(occurrence.series_id);
        // Work left short is offered on at the end: with the first reminder at or after the
        // end, or, without one, by a firing of its own then. A scheduling device arms it from
        // the sittings as they stand, and re-arms as they change.
        let reference = if series.is_recurring() {
            crate::model::BlockRef::Occurrence(occurrence.series_id, occurrence.date)
        } else {
            crate::model::BlockRef::OneOff(occurrence.series_id)
        };
        let mut carry = series.flags.accepts_tasks && falls_short(snapshot, &facts, reference);
        // The earliest reminder at or after the end carries it, whatever order they were made
        // in: "at end" before "10m after end", never the other way round.
        let mut triggers = triggers(target, &snapshot.settings.default_block_reminders);
        triggers.sort_by_key(|(key, trigger, _)| (!matches!(trigger.anchor, ReminderAnchor::BlockEnd), trigger.offset_mins, *key));
        for (key, trigger, is_own) in triggers {
            let anchor_at = match trigger.anchor {
                ReminderAnchor::BlockStart => start,
                ReminderAnchor::BlockEnd => end,
                _ => continue,
            };
            let at = anchor_at.saturating_add(offset(&trigger)).unwrap_or(anchor_at);
            let carries = carry && matches!(trigger.anchor, ReminderAnchor::BlockEnd) && trigger.offset_mins >= 0;
            carry &= !carries;
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
                carry: carries,
                overrun: None,
            });
        }
        // Planned work outrunning its time is said before the clock says it, unless the
        // overrun fits in the free time after the block, or is only a few minutes.
        if series.flags.accepts_tasks {
            let ends = occurrence.date.to_datetime(occurrence.start_time)
                + SignedDuration::from_mins(i64::from(occurrence.duration_mins));
            let next = occurrences
                .iter()
                .filter(|o| o.date == occurrence.date && o.date.to_datetime(o.start_time) >= ends)
                .map(|o| o.date.to_datetime(o.start_time))
                .min();
            let day_end = occurrence.date.to_datetime(snapshot.settings.day_window.1);
            let gap = next.unwrap_or(day_end).duration_since(ends).as_mins().max(0);
            if let Some((at, overrun)) = overrun_at(snapshot, &facts, reference, start, end, gap.max(FEW_MINS)) {
                consider(Firing {
                    key: overrun_key(occurrence.series_id),
                    own: false,
                    target,
                    occurrence: occurrence.date,
                    trigger: Trigger::at(ReminderAnchor::BlockEnd),
                    anchor_at: end,
                    at,
                    snoozed: false,
                    all_day: false,
                    carry: false,
                    overrun: Some(overrun),
                });
            }
        }
        if carry {
            consider(Firing {
                key: carry_key(occurrence.series_id),
                own: false,
                target,
                occurrence: occurrence.date,
                trigger: Trigger::at(ReminderAnchor::BlockEnd),
                anchor_at: end,
                at: end,
                snoozed: false,
                all_day: false,
                carry: true,
                overrun: None,
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
                carry: false,
                overrun: None,
            });
        }
    }

    found.sort_by_key(|f| (f.at, f.key));
    Firings { firings: found, unreadable: unreadable.into_iter().map(|(id, _)| id).collect() }
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
