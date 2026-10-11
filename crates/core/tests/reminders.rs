//! When reminders fire, worked out from the synced records alone.

use jiff::civil::{date, time};
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use lumenna_core::id::{NodeId, ProjectId, TaskId};
use lumenna_core::model::{
    BlockException, BlockKind, BlockSeries, Delivery, Due, ExceptionAction, Project, Reminder,
    ReminderAck, ReminderAction, ReminderAnchor, ReminderTarget, Task, TaskCompletion, Trigger,
};
use lumenna_core::order::OrderKey;
use lumenna_core::remind::{default_key, firings};
use lumenna_core::snapshot::Snapshot;
use lumenna_core::SeriesId;

fn zone() -> TimeZone {
    TimeZone::get("America/New_York").unwrap()
}

fn at(y: i16, m: i8, d: i8, h: i8, min: i8) -> Timestamp {
    date(y, m, d).at(h, min, 0, 0).to_zoned(zone()).unwrap().timestamp()
}

struct World {
    snapshot: Snapshot,
    project: ProjectId,
}

impl World {
    fn new() -> Self {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let project = inbox.id;
        snapshot.projects.insert(inbox.id, inbox);
        // No defaults unless a test asks for them, so each test says what fires.
        snapshot.settings.default_block_reminders = Vec::new();
        snapshot.settings.default_task_reminders = Vec::new();
        Self { snapshot, project }
    }

    fn task(&mut self, title: &str, due: Due) -> TaskId {
        let mut task = Task::new(self.project, title, OrderKey::middle());
        task.due = Some(due);
        let id = task.id;
        self.snapshot.tasks.insert(id, task);
        id
    }

    fn daily(&mut self, title: &str, h: i8, m: i8, minutes: u32) -> SeriesId {
        let mut series = BlockSeries::one_off(title, BlockKind::Work, date(2026, 3, 1), time(h, m, 0, 0), minutes).unwrap();
        series.rrule = Some("FREQ=DAILY".to_owned());
        series.end_date = None;
        let id = series.id;
        self.snapshot.series.insert(id, series);
        id
    }

    fn remind(&mut self, target: ReminderTarget, trigger: Trigger) -> Reminder {
        let reminder = Reminder::new(target, trigger).unwrap();
        self.snapshot.reminders.insert(reminder.id, reminder.clone());
        reminder
    }

    fn fired(&self, from: Timestamp, until: Timestamp) -> Vec<Timestamp> {
        firings(&self.snapshot, from, until, &zone(), None).unwrap().into_iter().map(|f| f.at).collect()
    }
}

#[test]
fn a_block_reminder_fires_before_each_occurrence_and_not_on_a_cancelled_one() {
    let mut world = World::new();
    let deep = world.daily("Deep work", 9, 0, 120);
    world.remind(ReminderTarget::Block(deep), Trigger::before(ReminderAnchor::BlockStart, 5));
    world.snapshot.exceptions.insert(
        (deep, date(2026, 5, 7)),
        BlockException { series_id: deep, original_date: date(2026, 5, 7), action: ExceptionAction::Cancelled },
    );
    let fired = world.fired(at(2026, 5, 6, 0, 0), at(2026, 5, 9, 0, 0));
    assert_eq!(fired, [at(2026, 5, 6, 8, 55), at(2026, 5, 8, 8, 55)], "not on the 7th, which was cancelled");
}

#[test]
fn a_block_without_reminders_of_its_own_takes_the_defaults_and_one_with_them_does_not() {
    let mut world = World::new();
    world.snapshot.settings.default_block_reminders = vec![Trigger::before(ReminderAnchor::BlockStart, 5)];
    world.daily("Deep work", 9, 0, 120);
    let lunch = world.daily("Lunch", 12, 0, 30);
    world.remind(ReminderTarget::Block(lunch), Trigger::at(ReminderAnchor::BlockEnd));
    let fired = world.fired(at(2026, 5, 6, 0, 0), at(2026, 5, 7, 0, 0));
    assert_eq!(fired, [at(2026, 5, 6, 8, 55), at(2026, 5, 6, 12, 30)]);
}

#[test]
fn a_task_reminder_counts_from_its_due_time_or_the_all_day_hour() {
    let mut world = World::new();
    let call = world.task("Call the bank", Due::at(date(2026, 5, 6), time(15, 0, 0, 0)));
    let rent = world.task("Pay rent", Due::on(date(2026, 5, 6)));
    world.remind(ReminderTarget::Task(call), Trigger::before(ReminderAnchor::Due, 30));
    world.remind(ReminderTarget::Task(rent), Trigger::at(ReminderAnchor::Due));
    let fired = world.fired(at(2026, 5, 6, 0, 0), at(2026, 5, 7, 0, 0));
    assert_eq!(fired, [at(2026, 5, 6, 9, 0), at(2026, 5, 6, 14, 30)], "the all-day hour is 9:00");
}

#[test]
fn a_completed_or_trashed_task_fires_nothing() {
    let mut world = World::new();
    let done = world.task("Done already", Due::at(date(2026, 5, 6), time(15, 0, 0, 0)));
    let trashed = world.task("Thrown away", Due::at(date(2026, 5, 6), time(16, 0, 0, 0)));
    world.remind(ReminderTarget::Task(done), Trigger::at(ReminderAnchor::Due));
    world.remind(ReminderTarget::Task(trashed), Trigger::at(ReminderAnchor::Due));
    let completion = TaskCompletion::new(done, None);
    world.snapshot.completions.insert(completion.id, completion);
    world.snapshot.tasks.get_mut(&trashed).unwrap().deleted_at = Some(Timestamp::now());
    assert!(world.fired(at(2026, 5, 6, 0, 0), at(2026, 5, 7, 0, 0)).is_empty());
}

#[test]
fn a_dismissal_stops_one_firing_and_a_snooze_moves_it() {
    let mut world = World::new();
    let deep = world.daily("Deep work", 9, 0, 120);
    let reminder = world.remind(ReminderTarget::Block(deep), Trigger::at(ReminderAnchor::BlockStart));
    let ack = |date, action| ReminderAck {
        reminder_id: reminder.id,
        occurrence_date: date,
        action,
        acked_at: Timestamp::now(),
        acked_by: NodeId::from_bytes([1; 32]),
    };
    world.snapshot.acks.insert((reminder.id, date(2026, 5, 6)), ack(date(2026, 5, 6), ReminderAction::Dismissed));
    let later = date(2026, 5, 7).at(9, 10, 0, 0).to_zoned(zone()).unwrap();
    world.snapshot.acks.insert((reminder.id, date(2026, 5, 7)), ack(date(2026, 5, 7), ReminderAction::Snoozed { until: later }));
    let fired = world.fired(at(2026, 5, 6, 0, 0), at(2026, 5, 8, 0, 0));
    assert_eq!(fired, [at(2026, 5, 7, 9, 10)], "the 6th dismissed, the 7th snoozed ten minutes");
}

#[test]
fn a_default_dismissed_on_one_device_is_dismissed_on_all() {
    let mut world = World::new();
    world.snapshot.settings.default_block_reminders = vec![Trigger::before(ReminderAnchor::BlockStart, 5)];
    let deep = world.daily("Deep work", 9, 0, 120);
    let key = default_key(&ReminderTarget::Block(deep), &Trigger::before(ReminderAnchor::BlockStart, 5));
    world.snapshot.acks.insert(
        (key, date(2026, 5, 6)),
        ReminderAck {
            reminder_id: key,
            occurrence_date: date(2026, 5, 6),
            action: ReminderAction::Dismissed,
            acked_at: Timestamp::now(),
            acked_by: NodeId::from_bytes([2; 32]),
        },
    );
    assert!(world.fired(at(2026, 5, 6, 0, 0), at(2026, 5, 7, 0, 0)).is_empty());
}

#[test]
fn a_reminder_for_other_devices_fires_nowhere_else_and_still_keeps_the_defaults_off() {
    let mut world = World::new();
    world.snapshot.settings.default_block_reminders = vec![Trigger::before(ReminderAnchor::BlockStart, 5)];
    let deep = world.daily("Deep work", 9, 0, 120);
    let laptop = NodeId::from_bytes([1; 32]);
    let mut reminder = world.remind(ReminderTarget::Block(deep), Trigger::at(ReminderAnchor::BlockStart));
    reminder.delivery = Delivery::OnlyDevices([laptop].into_iter().collect());
    world.snapshot.reminders.insert(reminder.id, reminder);
    let window = (at(2026, 5, 6, 0, 0), at(2026, 5, 7, 0, 0));
    let on = |device| firings(&world.snapshot, window.0, window.1, &zone(), device).unwrap().len();
    assert_eq!(on(Some(laptop)), 1);
    assert_eq!(on(Some(NodeId::from_bytes([2; 32]))), 0, "and no default in its place");
    assert_eq!(on(None), 0);
}

#[test]
fn a_block_in_the_springs_skipped_hour_starts_after_it_and_keeps_its_length() {
    let mut world = World::new();
    let early = world.daily("Early", 2, 30, 60);
    world.remind(ReminderTarget::Block(early), Trigger::at(ReminderAnchor::BlockStart));
    world.remind(ReminderTarget::Block(early), Trigger::at(ReminderAnchor::BlockEnd));
    // 8 March 2026: 2:00 becomes 3:00 in New York.
    let fired = world.fired(at(2026, 3, 8, 0, 0), at(2026, 3, 8, 12, 0));
    let local: Vec<_> = fired.iter().map(|t| t.to_zoned(zone()).time()).collect();
    assert_eq!(local, [time(3, 30, 0, 0), time(4, 30, 0, 0)], "moved past the gap, an hour long");
}

#[test]
fn a_block_across_the_autumn_change_lasts_its_real_minutes() {
    let mut world = World::new();
    let night = world.daily("Night", 0, 30, 120);
    world.remind(ReminderTarget::Block(night), Trigger::at(ReminderAnchor::BlockEnd));
    // 1 November 2026: 2:00 goes back to 1:00, so two real hours from 0:30 end at 1:30.
    let fired = world.fired(at(2026, 11, 1, 0, 0), at(2026, 11, 1, 12, 0));
    assert_eq!(fired.len(), 1);
    let start = date(2026, 11, 1).at(0, 30, 0, 0).to_zoned(zone()).unwrap().timestamp();
    assert_eq!(fired[0].duration_since(start), jiff::SignedDuration::from_hours(2));
    assert_eq!(fired[0].to_zoned(zone()).time(), time(1, 30, 0, 0));
}

#[test]
fn the_window_is_exclusive_at_its_start_so_back_to_back_checks_fire_each_once() {
    let mut world = World::new();
    let deep = world.daily("Deep work", 9, 0, 120);
    world.remind(ReminderTarget::Block(deep), Trigger::at(ReminderAnchor::BlockStart));
    let nine = at(2026, 5, 6, 9, 0);
    assert_eq!(world.fired(nine.checked_sub(1.minute()).unwrap(), nine).len(), 1);
    assert!(world.fired(nine, nine.checked_add(1.minute()).unwrap()).is_empty());
}

#[test]
fn checks_in_any_steps_fire_exactly_what_one_check_over_the_whole_time_does() {
    let mut world = World::new();
    world.snapshot.settings.default_block_reminders = vec![Trigger::before(ReminderAnchor::BlockStart, 5)];
    world.daily("Deep work", 9, 0, 120);
    let late = world.daily("Late", 23, 30, 60);
    world.remind(ReminderTarget::Block(late), Trigger::after(ReminderAnchor::BlockEnd, 15));
    let call = world.task("Call", Due::at(date(2026, 3, 9), time(10, 0, 0, 0)));
    world.remind(ReminderTarget::Task(call), Trigger::before(ReminderAnchor::Due, 90));
    // A week across the spring change, checked in steps of uneven length.
    let (start, end) = (at(2026, 3, 5, 0, 0), at(2026, 3, 12, 0, 0));
    let whole = world.fired(start, end);
    for step_mins in [1_i64, 7, 59, 60, 61, 333, 1440, 2000] {
        let mut stepped = Vec::new();
        let mut from = start;
        while from < end {
            let until = from.checked_add(jiff::SignedDuration::from_mins(step_mins)).unwrap().min(end);
            stepped.extend(world.fired(from, until));
            from = until;
        }
        assert_eq!(stepped, whole, "in steps of {step_mins} minutes");
    }
    assert!(whole.len() >= 15, "{}", whole.len());
}
