//! Backup, export and import (§9): *import of our own output must be tested, not assumed.*
//!
//! A recovery path never exercised does not work. These are also the tests that prove the
//! export format is complete: a populated store goes out and comes back, and any field the
//! format forgot reads back as its default and fails the comparison.

use jiff::civil::{Weekday, date, time};
use jiff::{Timestamp, Zoned};
use lumenna_core::id::{NodeId, ProjectId, TaskId};
use lumenna_core::model::{
    AssignmentStatus, BlockAssignment, BlockException, BlockKind, BlockRef, BlockSeries,
    Delivery, Due, ExceptionAction, ExternalProvider, ExternalRef, Label, Priority, Project,
    Recurrence, Reminder, ReminderAnchor, ReminderTarget, SavedFilter, Settings, Task,
    TaskCompletion, Trigger, TzName, Verbosity,
};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_store::export::{export_json, ics, markdown, org, parse_json};
use lumenna_store::{Store, backup};
use proptest::prelude::*;

fn now() -> Timestamp {
    Timestamp::from_second(1_800_000_000).unwrap()
}

/// Everything the model has, with every optional field set somewhere — and one of each kind
/// of record in the trash, which an export must leave out and a backup must keep.
struct Populated {
    store: Store,
    trashed_task: TaskId,
}

fn populated() -> Populated {
    let mut store = Store::open_in_memory().unwrap();
    let mut work = Project::new("Work", OrderKey::middle());
    work.color = Some("#336699".to_owned());
    work.weight = Some(1.5);
    let mut backend = Project::new("Backend", OrderKey::after(&work.order));
    backend.parent_id = Some(work.id);
    backend.archived = true;
    backend.weight = Some(1.0);

    let mut laptop = Label::new("laptop", OrderKey::middle());
    laptop.color = Some("#abcdef".to_owned());
    let errand = Label::new("errand", OrderKey::after(&laptop.order));

    let filter = SavedFilter {
        id: lumenna_core::id::FilterId::new(),
        name: "Overdue at the laptop".to_owned(),
        query: "overdue & @laptop".to_owned(),
        order: OrderKey::middle(),
        color: Some("#ff0000".to_owned()),
        deleted_at: None,
    };

    let mut parent = Task::new(work.id, "Ship the release", OrderKey::middle());
    parent.notes = "Line one.\nLine two, with a comma; and a semicolon.".to_owned();
    parent.priority = Priority::P1;
    parent.labels = [laptop.id, errand.id].into_iter().collect();
    parent.due = Some(Due {
        date: date(2026, 9, 7),
        time: Some(time(15, 30, 0, 0)),
        timezone: Some(TzName::new("America/New_York")),
        recurrence: Some(Recurrence {
            rrule: "FREQ=WEEKLY;BYDAY=MO".to_owned(),
            from_completion: true,
        }),
    });
    parent.estimate_mins = Some(90);
    parent.external = Some(ExternalRef {
        provider: ExternalProvider::CalDav,
        external_id: "abc-123".to_owned(),
        read_only: false,
        last_synced: now(),
    });
    parent.created_at = now();

    let mut child = Task::new(work.id, "Write the notes", OrderKey::middle());
    child.parent_id = Some(parent.id);
    child.depends = [parent.id].into_iter().collect();
    child.due = Some(Due::on(date(2026, 9, 1)));
    child.created_at = now();

    let mut trashed = Task::new(ProjectId::INBOX, "Something private", OrderKey::middle());
    trashed.deleted_at = Some(now());
    let trashed_task = trashed.id;

    let completion = TaskCompletion {
        completed_at: now(),
        ..TaskCompletion::cascaded(child.id, Some(date(2026, 9, 7)), parent.id)
    };
    let trashed_completion = TaskCompletion::new(trashed.id, None);

    let mut series = BlockSeries::one_off(
        "Deep work",
        BlockKind::Work,
        date(2026, 3, 4),
        time(9, 0, 0, 0),
        90,
    )
    .unwrap()
    .with_min_duration(45)
    .unwrap();
    series.notes = "Phone in the other room.".to_owned();
    series.rrule = Some("FREQ=DAILY".to_owned());
    series.end_date = Some(date(2027, 6, 30));
    series.timezone = Some(TzName::new("Europe/London"));
    series.color = Some("#00ff00".to_owned());
    series.icon = Some("brain".to_owned());
    series.task_filter = Some("#Work".to_owned());
    series.flags.anchored = true;

    let lunch = BlockSeries::one_off("Lunch", BlockKind::Break, date(2027, 1, 4), time(12, 0, 0, 0), 45)
        .unwrap();

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
            title: Some("Deep work, short".to_owned()),
            kind: Some(BlockKind::Break),
            flags: Some(BlockKind::Break.default_flags()),
        },
    };

    let mut sitting = BlockAssignment::new(
        BlockRef::Occurrence(series.id, date(2026, 3, 4)),
        parent.id,
        OrderKey::middle(),
    );
    sitting.planned_mins = Some(45);
    sitting.accumulated_mins = 20;
    sitting.running_since = Some(now());
    sitting.status = AssignmentStatus::InProgress;
    sitting.created_at = now();
    let one_off = BlockAssignment {
        created_at: now(),
        ..BlockAssignment::new(BlockRef::OneOff(lunch.id), child.id, OrderKey::middle())
    };

    let mut reminder =
        Reminder::new(ReminderTarget::Task(parent.id), Trigger::before(ReminderAnchor::Due, 30))
            .unwrap();
    reminder.delivery = Delivery::OnlyDevices([NodeId::from_bytes([7; 32])].into_iter().collect());
    let at: Zoned = "2026-09-01T15:00:00-04:00[America/New_York]".parse().unwrap();
    let absolute =
        Reminder::new(ReminderTarget::Block(series.id), Trigger::at(ReminderAnchor::Absolute(at)))
            .unwrap();

    let settings = Settings {
        cascade_complete_subtasks: false,
        verbosity: Verbosity::Terse,
        default_task_reminders: vec![Trigger::before(ReminderAnchor::Due, 10)],
        default_block_reminders: vec![Trigger::after(ReminderAnchor::BlockEnd, 5)],
        all_day_reminder_hour: time(8, 30, 0, 0),
        day_window: (time(7, 0, 0, 0), time(23, 0, 0, 0)),
        week_start: Weekday::Sunday,
    };

    let before = store.snapshot().0.settings;
    store
        .write(|docs| {
            docs.put_settings(&settings, Some(&before))?;
            for p in [&work, &backend] {
                docs.put_project(p, None)?;
            }
            docs.put_label(&laptop, None)?;
            docs.put_label(&errand, None)?;
            docs.put_filter(&filter, None)?;
            for t in [&parent, &child, &trashed] {
                docs.put_task(t, None)?;
            }
            docs.put_completion(&completion, None)?;
            docs.put_completion(&trashed_completion, None)?;
            docs.put_series(&series, None)?;
            docs.put_series(&lunch, None)?;
            docs.put_exception(&cancelled, None)?;
            docs.put_exception(&modified, None)?;
            docs.put_assignment(2026, &sitting, None)?;
            docs.put_assignment(2027, &one_off, None)?;
            docs.put_reminder(&reminder, None)?;
            docs.put_reminder(&absolute, None)
        })
        .unwrap();
    Populated { store, trashed_task }
}

fn everything(store: &mut Store) -> Snapshot {
    store.load_all_years().unwrap();
    let (snapshot, report) = store.snapshot();
    assert!(report.is_clean(), "{report:?}");
    snapshot
}

#[test]
fn a_backup_restores_everything_including_the_trash() {
    let Populated { mut store, trashed_task } = populated();
    let bytes = store.backup().unwrap();
    let original = everything(&mut store);

    let mut fresh = Store::open_in_memory().unwrap();
    let restored = fresh.restore(&bytes).unwrap();
    assert_eq!(restored.documents, 4, "core, devices, and two years");
    assert!(restored.unknown.is_empty());

    let rebuilt = everything(&mut fresh);
    assert_eq!(rebuilt, original);
    assert!(rebuilt.tasks.contains_key(&trashed_task), "a backup is full fidelity");
    assert_eq!(rebuilt.series.len(), 2);

    // Restoring the same backup again brings in nothing new.
    assert_eq!(fresh.restore(&bytes).unwrap().changed, 0);
}

#[test]
fn a_restored_routine_still_shows_in_later_years() {
    let Populated { mut store, .. } = populated();
    let bytes = store.backup().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("restored.sqlite");
    {
        let mut fresh = Store::open(&path).unwrap();
        fresh.restore(&bytes).unwrap();
    }
    let mut reopened = Store::open(&path).unwrap();
    reopened.load_year(2027).unwrap();
    let day = reopened.snapshot().0.day(date(2027, 1, 5)).unwrap();
    assert!(day.iter().any(|o| o.title == "Deep work"), "{day:?}");
}

#[test]
fn an_export_round_trips_everything_but_the_trash() {
    let Populated { mut store, trashed_task } = populated();
    let original = everything(&mut store);
    let json = export_json(&original, now());
    assert!(!json.contains("Something private"), "the trash never leaves in an export");

    let imported = parse_json(&json).unwrap();
    let mut fresh = Store::open_in_memory().unwrap();
    let report = fresh.import(&imported).unwrap();
    assert_eq!(report.skipped, 0);
    let rebuilt = everything(&mut fresh);

    let mut expected = original.clone();
    expected.tasks.remove(&trashed_task);
    expected.completions.retain(|_, c| c.task_id != trashed_task);
    // The Inbox comes with every store, so the fresh one's matches the original's.
    assert_eq!(rebuilt.tasks, expected.tasks);
    assert_eq!(rebuilt.completions, expected.completions);
    assert_eq!(rebuilt.projects, expected.projects);
    assert_eq!(rebuilt.labels, expected.labels);
    assert_eq!(rebuilt.saved_filters, expected.saved_filters);
    assert_eq!(rebuilt.series, expected.series);
    assert_eq!(rebuilt.exceptions, expected.exceptions);
    assert_eq!(rebuilt.assignments, expected.assignments);
    assert_eq!(rebuilt.reminders, expected.reminders);
    assert_eq!(rebuilt.settings, expected.settings);
}

#[test]
fn importing_the_same_export_twice_changes_nothing_the_second_time() {
    let Populated { mut store, .. } = populated();
    let json = export_json(&everything(&mut store), now());
    let imported = parse_json(&json).unwrap();

    let mut fresh = Store::open_in_memory().unwrap();
    let first = fresh.import(&imported).unwrap();
    assert!(first.created > 0);
    let second = fresh.import(&imported).unwrap();
    assert_eq!((second.created, second.updated), (0, 0));

    // Onto the store it came from, only what changed since goes back.
    let again = store.import(&imported).unwrap();
    assert_eq!((again.created, again.updated), (0, 0));
}

#[test]
fn an_export_from_the_future_or_from_elsewhere_is_refused() {
    let newer = r#"{"format": "lumenna-export", "version": 99}"#;
    assert!(parse_json(newer).unwrap_err().to_string().contains("newer version"));
    let other = r#"{"format": "todo.txt", "version": 1}"#;
    assert!(parse_json(other).is_err());
    let broken = r#"{"format": "lumenna-export", "version": 1, "tasks": [{"id": "nope",
        "title": "t", "project": "x", "priority": 4, "order": "V", "created_at": "x"}]}"#;
    let error = parse_json(broken).unwrap_err().to_string();
    assert!(error.contains("task 't'") && error.contains("nope"), "{error}");
}

#[test]
fn markdown_and_org_lay_out_projects_and_subtasks() {
    let Populated { mut store, .. } = populated();
    let snapshot = everything(&mut store);
    let local: Zoned = "2026-09-01T09:15:00-04:00[America/New_York]".parse().unwrap();

    let md = markdown(&snapshot, &local);
    assert!(md.contains("Exported Tuesday 1 September 2026 at 09:15."), "{md}");
    assert!(md.contains("## Inbox"), "{md}");
    assert!(md.contains("## Work"));
    assert!(md.contains("### Backend (archived)"));
    assert!(md.contains("- [ ] Ship the release — due 2026-09-07 at 15:30"), "{md}");
    assert!(md.contains("  - [x] Write the notes"), "the subtask nests, and is done: {md}");
    assert!(!md.contains("Something private"));

    let o = org(&snapshot, &local);
    assert!(o.contains("#+DATE: [2026-09-01 Tue 09:15]"), "{o}");
    assert!(o.contains("** TODO [#A] Ship the release :laptop:errand:"), "{o}");
    assert!(o.contains("*** DONE Write the notes"), "{o}");
    assert!(o.contains("DEADLINE: <2026-09-07 Mon 15:30>"), "{o}");
    assert!(o.contains(":EFFORT: 1:30"));
    assert!(!o.contains("Something private"));
}

#[test]
fn blocks_export_as_a_calendar_with_their_exceptions() {
    let Populated { mut store, .. } = populated();
    let calendar = ics(&everything(&mut store), now());
    let unfolded = calendar.replace("\r\n ", "");
    assert!(unfolded.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(unfolded.contains("DTSTART;TZID=Europe/London:20260304T090000"), "{unfolded}");
    assert!(unfolded.contains("RRULE:FREQ=DAILY;UNTIL=20270630T235959"));
    assert!(unfolded.contains("EXDATE;TZID=Europe/London:20260305T090000"));
    assert!(unfolded.contains("RECURRENCE-ID;TZID=Europe/London:20260306T090000"));
    assert!(unfolded.contains("SUMMARY:Deep work\\, short"), "escaped comma: {unfolded}");
    assert!(unfolded.contains("DTSTART:20270104T120000"), "a floating block floats");
    assert!(unfolded.contains("TRANSP:TRANSPARENT"), "a break is free time");
    assert_eq!(unfolded.matches("BEGIN:VEVENT").count(), 3);
    assert!(calendar.lines().all(|l| l.len() <= 76), "folded at 75 octets plus CR");
}

#[test]
fn backups_land_in_their_directory_and_are_taken_only_when_due() {
    let Populated { mut store, .. } = populated();
    let dir = tempfile::tempdir().unwrap();
    let policy = backup::Policy {
        directory: dir.path().join("backups"),
        keep: 2,
        every: Some(backup::Policy::DEFAULT_EVERY),
    };
    assert!(store.back_up_if_due(&policy, now()).unwrap().is_some());
    assert!(store.back_up_if_due(&policy, now()).unwrap().is_none(), "not due again yet");
    let later = now() + jiff::SignedDuration::from_hours(25);
    let path = store.back_up_if_due(&policy, later).unwrap().unwrap();

    let bytes = std::fs::read(&path).unwrap();
    assert!(backup::is_backup(&bytes));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "a backup holds the whole history, deleted tasks included");
    }
    assert_eq!(backup::list(&policy.directory).unwrap().len(), 2);
}

// ---------------------------------------------------------------------------------------
// Generated tasks
// ---------------------------------------------------------------------------------------

fn arbitrary_due() -> impl Strategy<Value = Option<Due>> {
    let rules = prop::sample::select(vec![
        "FREQ=DAILY",
        "FREQ=WEEKLY;BYDAY=MO,WE,FR",
        "FREQ=MONTHLY;BYMONTHDAY=-1",
        "FREQ=YEARLY;INTERVAL=2",
    ]);
    prop::option::of((
        (2020i16..2040, 1i8..=12, 1i8..=28),
        prop::option::of((0i8..24, 0i8..60)),
        prop::option::of(prop::sample::select(vec!["Europe/London", "Asia/Tokyo"])),
        prop::option::of((rules, any::<bool>())),
    ))
    .prop_map(|due| {
        due.map(|((y, m, d), clock, zone, rule)| Due {
            date: date(y, m, d),
            time: clock.map(|(h, min)| time(h, min, 0, 0)),
            timezone: zone.map(TzName::new),
            recurrence: rule.map(|(rrule, from_completion)| Recurrence {
                rrule: rrule.to_owned(),
                from_completion,
            }),
        })
    })
}

prop_compose! {
    fn arbitrary_task()(
        title in ".*",
        notes in ".*",
        priority in 1u8..=4,
        due in arbitrary_due(),
        estimate in prop::option::of(1u32..10_000),
        steps in 0usize..20,
        millis in 0i64..4_000_000_000_000,
        labels in 0usize..3,
    ) -> Task {
        let mut order = OrderKey::middle();
        for _ in 0..steps {
            order = OrderKey::after(&order);
        }
        let mut task = Task::new(ProjectId::INBOX, title, order);
        task.notes = notes;
        task.priority = Priority::from_u8(priority);
        task.due = due;
        task.estimate_mins = estimate;
        task.created_at = Timestamp::from_millisecond(millis).unwrap();
        task.labels = (0..labels).map(|_| lumenna_core::id::LabelId::new()).collect();
        task
    }
}

proptest! {
    #[test]
    fn any_task_survives_an_export_unchanged(tasks in prop::collection::vec(arbitrary_task(), 1..8)) {
        let mut snapshot = Snapshot::default();
        for (i, mut task) in tasks.into_iter().enumerate() {
            // Chain some of them, so parents and dependencies are exercised too.
            if i % 3 == 2 {
                let previous = *snapshot.tasks.keys().last().unwrap();
                task.parent_id = Some(previous);
                task.depends.insert(previous);
            }
            snapshot.tasks.insert(task.id, task);
        }
        let imported = parse_json(&export_json(&snapshot, now())).unwrap();
        let back: std::collections::BTreeMap<TaskId, Task> =
            imported.tasks.into_iter().map(|t| (t.id, t)).collect();
        prop_assert_eq!(back, snapshot.tasks);
    }
}
