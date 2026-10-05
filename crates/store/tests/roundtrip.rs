//! Every record survives the trip through an Automerge document unchanged.
//!
//! This is the cheapest test in the crate and the one that catches the most: a field added
//! to the model and forgotten in the mapping reads back as its default, and no type error
//! will ever tell you about it.

use std::collections::BTreeSet;

use jiff::civil::{Weekday, date, time};
use jiff::{Timestamp, Zoned};
use lumenna_core::id::{FilterId, LabelId, NodeId, ProjectId, TaskId};
use lumenna_core::model::{
    AssignmentStatus, BlockAssignment, BlockException, BlockKind, BlockRef, BlockSeries,
    Delivery, Device, Due, ExceptionAction, ExternalProvider, ExternalRef, Label, Priority,
    Project, Recurrence, Reminder, ReminderAck, ReminderAction, ReminderAnchor, ReminderTarget,
    SavedFilter, Settings, Task, TaskCompletion, Trigger, TzName, Verbosity,
};
use lumenna_core::order::OrderKey;
use lumenna_store::{Doc, DocId, Documents};

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

/// A task with every optional field populated, so nothing can be forgotten silently.
fn full_task(project: ProjectId) -> Task {
    let mut task = Task::new(project, "Write the chapter", OrderKey::middle());
    task.notes = "Some longer notes.\nWith a second line.".to_owned();
    task.parent_id = Some(TaskId::new());
    task.priority = Priority::P1;
    task.labels = [LabelId::new(), LabelId::new()].into_iter().collect();
    task.depends = [TaskId::new()].into_iter().collect();
    task.due = Some(Due {
        date: date(2026, 9, 1),
        time: Some(time(15, 30, 0, 0)),
        timezone: Some(TzName::new("America/New_York")),
        recurrence: Some(Recurrence {
            rrule: "FREQ=WEEKLY;BYDAY=MO".to_owned(),
            from_completion: true,
        }),
    });
    task.estimate_mins = Some(90);
    task.external = Some(ExternalRef {
        provider: ExternalProvider::CalDav,
        external_id: "abc-123".to_owned(),
        read_only: false,
        last_synced: now(),
    });
    task.created_at = now();
    task.deleted_at = Some(now());
    task
}

#[test]
fn a_fully_populated_task_round_trips() {
    let project = Project::inbox();
    let task = full_task(project.id);

    let mut docs = Documents::new();
    docs.put_project(&project, None).unwrap();
    docs.put_task(&task, None).unwrap();

    let (snapshot, report) = docs.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(snapshot.tasks[&task.id], task);
    assert_eq!(snapshot.projects[&project.id], project);
}

#[test]
fn a_minimal_task_round_trips() {
    let project = Project::inbox();
    let task = Task::new(project.id, "Just a title", OrderKey::middle());

    let mut docs = Documents::new();
    docs.put_task(&task, None).unwrap();

    let (snapshot, report) = docs.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(snapshot.tasks[&task.id], task);
}

#[test]
fn every_core_record_round_trips() {
    let project = Project::inbox();
    let task = Task::new(project.id, "t", OrderKey::middle());
    let completion = TaskCompletion::cascaded(task.id, Some(date(2026, 9, 1)), TaskId::new());
    let label =
        Label { color: Some("#abc".to_owned()), ..Label::new("laptop", OrderKey::middle()) };
    let filter = SavedFilter {
        id: FilterId::new(),
        name: "Overdue at the laptop".to_owned(),
        query: "overdue & @laptop".to_owned(),
        order: OrderKey::middle(),
        color: None,
        deleted_at: None,
    };
    let mut reminder =
        Reminder::new(ReminderTarget::Task(task.id), Trigger::before(ReminderAnchor::Due, 30))
            .unwrap();
    reminder.delivery = Delivery::OnlyDevices([NodeId::from_bytes([7; 32])].into_iter().collect());
    let ack = ReminderAck {
        reminder_id: reminder.id,
        occurrence_date: date(2026, 9, 1),
        action: ReminderAction::Snoozed {
            until: "2026-09-01T15:00:00-04:00[America/New_York]".parse::<Zoned>().unwrap(),
        },
        acked_at: now(),
        acked_by: NodeId::from_bytes([9; 32]),
    };
    let device = Device {
        node_id: NodeId::from_bytes([3; 32]),
        name: "Laptop".to_owned(),
        platform: "linux".to_owned(),
        paired_at: now(),
        last_seen: now(),
    };
    let settings = Settings {
        cascade_complete_subtasks: false,
        verbosity: Verbosity::Terse,
        default_task_reminders: vec![Trigger::before(ReminderAnchor::Due, 15)],
        default_block_reminders: vec![
            Trigger::before(ReminderAnchor::BlockStart, 5),
            Trigger::after(ReminderAnchor::BlockEnd, 5),
        ],
        all_day_reminder_hour: time(7, 30, 0, 0),
        day_window: (time(6, 0, 0, 0), time(23, 0, 0, 0)),
        week_start: Weekday::Sunday,
    };

    let mut docs = Documents::new();
    docs.put_project(&project, None).unwrap();
    docs.put_task(&task, None).unwrap();
    docs.put_completion(&completion, None).unwrap();
    docs.put_label(&label, None).unwrap();
    docs.put_filter(&filter, None).unwrap();
    docs.put_reminder(&reminder, None).unwrap();
    docs.put_ack(&ack, None).unwrap();
    docs.put_device(&device, None).unwrap();
    docs.put_settings(&settings, None).unwrap();

    let (s, report) = docs.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(s.projects[&project.id], project);
    assert_eq!(s.tasks[&task.id], task);
    assert_eq!(s.completions[&completion.id], completion);
    assert_eq!(s.labels[&label.id], label);
    assert_eq!(s.saved_filters[&filter.id], filter);
    assert_eq!(s.reminders[&reminder.id], reminder);
    assert_eq!(s.acks[&(reminder.id, date(2026, 9, 1))], ack);
    assert_eq!(s.devices[&device.node_id], device);
    assert_eq!(s.settings, settings);
}

#[test]
fn every_block_record_round_trips() {
    let series = BlockSeries {
        notes: "Focus".to_owned(),
        rrule: Some("FREQ=DAILY".to_owned()),
        task_filter: Some("#work".to_owned()),
        timezone: Some(TzName::new("Europe/London")),
        icon: Some("brain".to_owned()),
        ..BlockSeries::one_off(
            "Deep work",
            BlockKind::Work,
            date(2026, 3, 4),
            time(9, 0, 0, 0),
            90,
        )
        .unwrap()
        .with_min_duration(45)
        .unwrap()
    };
    let cancelled = BlockException {
        series_id: series.id,
        original_date: date(2026, 3, 5),
        action: ExceptionAction::Cancelled,
    };
    let modified = BlockException {
        series_id: series.id,
        original_date: date(2026, 3, 6),
        action: ExceptionAction::Modified {
            start_time: Some(time(10, 0, 0, 0)),
            duration_mins: Some(40),
            title: Some("Deep work (short)".to_owned()),
            kind: Some(BlockKind::Break),
            flags: Some(BlockKind::Break.default_flags()),
        },
    };
    let mut assignment = BlockAssignment::new(
        BlockRef::Occurrence(series.id, date(2026, 3, 4)),
        TaskId::new(),
        OrderKey::middle(),
    );
    assignment.planned_mins = Some(45);
    assignment.accumulated_mins = 20;
    assignment.running_since = Some(now());
    assignment.status = AssignmentStatus::InProgress;
    assignment.created_at = now();

    let mut docs = Documents::new();
    docs.put_series(&series, None).unwrap();
    docs.put_exception(&cancelled, None).unwrap();
    docs.put_exception(&modified, None).unwrap();
    docs.put_assignment(2026, &assignment, None).unwrap();

    let (s, report) = docs.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(s.series[&series.id], series);
    assert_eq!(s.exceptions[&(series.id, date(2026, 3, 5))], cancelled);
    assert_eq!(s.exceptions[&(series.id, date(2026, 3, 6))], modified);
    assert_eq!(s.assignments[&assignment.id], assignment);
    assert_eq!(docs.loaded_years(), vec![2026]);
}

#[test]
fn a_one_off_assignment_needs_its_year_stated() {
    // BlockRef::OneOff carries no date, so the store cannot shard it. Proving the
    // caller's year is honoured is the only guard against assignments landing in a document
    // the day view never opens.
    let series = BlockSeries::one_off(
        "Dentist",
        BlockKind::Event,
        date(2027, 1, 15),
        time(9, 0, 0, 0),
        60,
    )
    .unwrap();
    let assignment =
        BlockAssignment::new(BlockRef::OneOff(series.id), TaskId::new(), OrderKey::middle());

    let mut docs = Documents::new();
    docs.put_series(&series, None).unwrap();
    docs.put_assignment(2027, &assignment, None).unwrap();

    assert_eq!(docs.loaded_years(), vec![2027]);
    let (s, _) = docs.snapshot();
    assert_eq!(s.assignments[&assignment.id].block_ref, BlockRef::OneOff(series.id));
}

#[test]
fn a_device_holding_one_year_is_not_a_broken_store() {
    let this_year =
        BlockSeries::one_off("A", BlockKind::Work, date(2026, 6, 1), time(9, 0, 0, 0), 60).unwrap();
    let next_year =
        BlockSeries::one_off("B", BlockKind::Work, date(2027, 6, 1), time(9, 0, 0, 0), 60).unwrap();

    let mut docs = Documents::new();
    docs.put_series(&this_year, None).unwrap();
    docs.put_series(&next_year, None).unwrap();
    assert_eq!(docs.loaded_years(), vec![2026, 2027]);

    let only_2026 = docs.blocks(2026).save();
    let mut partial = Documents::new();
    partial.insert(Doc::load(DocId::Blocks(2026), &only_2026).unwrap());
    let (s, report) = partial.snapshot();
    assert!(report.is_clean());
    assert_eq!(s.series.len(), 1);
    assert!(s.series.contains_key(&this_year.id));
}

#[test]
fn an_update_writes_only_what_changed() {
    // The property everything else rests on: a field nobody touched produces no operation,
    // so a concurrent change to it from another device survives.
    let project = Project::inbox();
    let task = Task::new(project.id, "original", OrderKey::middle());

    let mut alice = Documents::new();
    alice.put_task(&task, None).unwrap();
    let mut bob = Documents::new();
    bob.core().load_incremental(&alice.core().save()).unwrap();

    let mut alice_edit = task.clone();
    alice_edit.title = "renamed by alice".to_owned();
    alice.put_task(&alice_edit, Some(&task)).unwrap();

    let mut bob_edit = task.clone();
    bob_edit.estimate_mins = Some(45);
    bob_edit.priority = Priority::P2;
    bob.put_task(&bob_edit, Some(&task)).unwrap();

    let bob_changes = bob.core().save();
    alice.core().load_incremental(&bob_changes).unwrap();

    let (s, _) = alice.snapshot();
    let merged = &s.tasks[&task.id];
    assert_eq!(merged.title, "renamed by alice");
    assert_eq!(merged.estimate_mins, Some(45), "bob's edit must survive alice's");
    assert_eq!(merged.priority, Priority::P2);
}

#[test]
fn concurrent_label_additions_both_survive() {
    // Sets are member-keyed maps precisely so this works; a wholesale rewrite would make it
    // last-write-wins and lose one of them.
    let project = Project::inbox();
    let task = Task::new(project.id, "t", OrderKey::middle());
    let (work, home) = (LabelId::new(), LabelId::new());

    let mut alice = Documents::new();
    alice.put_task(&task, None).unwrap();
    let mut bob = Documents::new();
    bob.core().load_incremental(&alice.core().save()).unwrap();

    let mut a = task.clone();
    a.labels.insert(work);
    alice.put_task(&a, Some(&task)).unwrap();

    let mut b = task.clone();
    b.labels.insert(home);
    bob.put_task(&b, Some(&task)).unwrap();

    alice.core().load_incremental(&bob.core().save()).unwrap();
    let (s, _) = alice.snapshot();
    assert_eq!(s.tasks[&task.id].labels, [work, home].into_iter().collect::<BTreeSet<_>>());
}

#[test]
fn notes_merge_by_character() {
    // The model hands over a whole String because core knows nothing about Automerge; the
    // store diffs it into a splice so character-level merge is not lost on the way.
    let project = Project::inbox();
    let mut task = Task::new(project.id, "t", OrderKey::middle());
    task.notes = "shared line\n".to_owned();

    let mut alice = Documents::new();
    alice.put_task(&task, None).unwrap();
    let mut bob = Documents::new();
    bob.core().load_incremental(&alice.core().save()).unwrap();

    let mut a = task.clone();
    a.notes = "shared line\nalice was here\n".to_owned();
    alice.put_task(&a, Some(&task)).unwrap();

    let mut b = task.clone();
    b.notes = "bob was here\nshared line\n".to_owned();
    bob.put_task(&b, Some(&task)).unwrap();

    alice.core().load_incremental(&bob.core().save()).unwrap();
    let (s, _) = alice.snapshot();
    let notes = &s.tasks[&task.id].notes;
    assert!(notes.contains("alice was here"), "{notes:?}");
    assert!(notes.contains("bob was here"), "{notes:?}");
    assert_eq!(notes.matches("shared line").count(), 1, "the common text must not double");
}

#[test]
fn purging_removes_the_record_entirely() {
    let project = Project::inbox();
    let task = Task::new(project.id, "t", OrderKey::middle());
    let mut docs = Documents::new();
    docs.put_task(&task, None).unwrap();
    assert_eq!(docs.snapshot().0.tasks.len(), 1);

    docs.purge_task(&task.id).unwrap();
    assert!(docs.snapshot().0.tasks.is_empty());
    // Purging something absent is not an error: emptying the trash twice is harmless.
    docs.purge_task(&task.id).unwrap();
}

#[test]
fn saving_and_loading_preserves_everything() {
    let project = Project::inbox();
    let task = full_task(project.id);
    let mut docs = Documents::new();
    docs.put_project(&project, None).unwrap();
    docs.put_task(&task, None).unwrap();

    let bytes = docs.core().save();
    let mut restored = Documents::new();
    restored.insert(Doc::load(DocId::Core, &bytes).unwrap());

    assert_eq!(restored.snapshot().0, docs.snapshot().0);
}

#[test]
fn an_emptied_setting_list_stays_empty() {
    // An absent key means the setting was never written and the default applies; a key
    // holding an empty map means the user emptied the list. Conflating them makes a
    // deliberately silenced default reminder come back on the next restart.
    let mut docs = Documents::new();
    assert_eq!(
        docs.snapshot().0.settings.default_block_reminders,
        Settings::default().default_block_reminders,
        "an untouched profile gets the defaults"
    );

    let silenced = Settings { default_block_reminders: Vec::new(), ..Settings::default() };
    docs.put_settings(&silenced, None).unwrap();
    assert!(docs.snapshot().0.settings.default_block_reminders.is_empty());
}

#[test]
fn a_neutral_weight_is_kept_apart_from_no_weight() {
    // `None` inherits; `Some(1.0)` opts back to neutral under a heavy parent. Storing them
    // the same way would make the second impossible to say.
    let mut docs = Documents::new();
    let inherits = Project::new("inherits", OrderKey::middle());
    let neutral = Project { weight: Some(1.0), ..Project::new("neutral", OrderKey::middle()) };
    let heavy = Project { weight: Some(2.0), ..Project::new("heavy", OrderKey::middle()) };
    for project in [&inherits, &neutral, &heavy] {
        docs.put_project(project, None).unwrap();
    }
    let mut cleared = heavy.clone();
    cleared.weight = None;
    docs.put_project(&cleared, Some(&heavy)).unwrap();

    let (s, report) = docs.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(s.projects[&inherits.id].weight, None);
    assert_eq!(s.projects[&neutral.id].weight, Some(1.0));
    assert_eq!(s.projects[&cleared.id].weight, None);
}
