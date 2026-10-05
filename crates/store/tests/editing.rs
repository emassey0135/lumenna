//! Edits through the real store: applied as Automerge operations, undone the same way, and
//! merged between replicas.
//!
//! The in-memory tests in `core` prove the *rules*. These prove the rules survive the round
//! trip through a CRDT — which is a stronger claim, because an inverse has to reproduce
//! every field exactly for a document to come back to where it started.

use jiff::civil::date;
use jiff::Zoned;
use lumenna_core::edit::{
    MoveTo, ProjectDeletion, assign_task, complete_task, create_task, merge_labels, move_task,
    trash_project, trash_task, uncomplete_task, update_task,
};
use lumenna_core::model::{
    BlockKind, BlockRef, BlockSeries, Due, Label, Priority, Project, Recurrence, Task,
};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_store::{Documents, Store};

/// One mutation, deferred so the same list can be replayed against a changing snapshot.
type Operation = Box<dyn Fn(&Snapshot) -> lumenna_core::edit::Edit>;

fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

/// A store with an Inbox, a Work project, two labels, and a small tree of tasks.
fn seeded() -> (Documents, Project, Project, Label, Label) {
    let mut docs = Documents::new();
    let inbox = Project::inbox();
    let work = Project::new("Work", OrderKey::middle());
    let laptop = Label::new("laptop", OrderKey::middle());
    let lapto = Label::new("lapto", OrderKey::after(&laptop.order));

    docs.put_project(&inbox, None).unwrap();
    docs.put_project(&work, None).unwrap();
    docs.put_label(&laptop, None).unwrap();
    docs.put_label(&lapto, None).unwrap();
    (docs, inbox, work, laptop, lapto)
}

fn snapshot(docs: &Documents) -> Snapshot {
    let (snapshot, report) = docs.snapshot();
    assert!(report.is_clean(), "{report:?}");
    snapshot
}

#[test]
fn an_edit_applied_through_the_store_takes_effect() {
    let (mut docs, _, work, _, _) = seeded();
    let task = Task::new(work.id, "Review PR", OrderKey::middle());
    let id = task.id;

    docs.apply(&create_task(task)).unwrap();
    assert_eq!(snapshot(&docs).tasks[&id].title, "Review PR");

    let edit = complete_task(&snapshot(&docs), id, &now()).unwrap();
    docs.apply(&edit).unwrap();
    let after = snapshot(&docs);
    assert!(after.is_completed(&after.tasks[&id]));
}

#[test]
fn every_operation_undoes_cleanly_through_automerge() {
    // Undo applies an edit's inverse, which must reproduce every field exactly, or the
    // document does not come back to where it started.
    let (mut docs, inbox, work, laptop, lapto) = seeded();

    let mut parent = Task::new(work.id, "Ship release", OrderKey::middle());
    parent.due = Some(Due {
        recurrence: Some(Recurrence {
            rrule: "FREQ=WEEKLY".to_owned(),
            from_completion: false,
        }),
        ..Due::on(date(2026, 5, 6))
    });
    parent.notes = "Some notes that must survive a round trip.".to_owned();
    let mut child = Task::new(work.id, "Write notes", OrderKey::middle());
    child.parent_id = Some(parent.id);
    child.labels.insert(lapto.id);
    child.estimate_mins = Some(45);
    let (parent_id, child_id) = (parent.id, child.id);

    docs.apply(&create_task(parent)).unwrap();
    docs.apply(&create_task(child)).unwrap();

    let mut focus = BlockSeries::one_off(
        "Focus",
        BlockKind::Work,
        date(2026, 5, 1),
        jiff::civil::time(9, 0, 0, 0),
        90,
    )
    .unwrap();
    focus.rrule = Some("FREQ=DAILY".to_owned());
    focus.end_date = None;
    let focus_id = focus.id;
    docs.put_series(&focus, None).unwrap();

    let operations: Vec<Operation> = vec![
        Box::new(move |s| complete_task(s, parent_id, &now()).unwrap()),
        Box::new(move |s| trash_task(s, child_id).unwrap()),
        Box::new(move |s| move_task(s, child_id, MoveTo::Project(inbox.id)).unwrap()),
        Box::new(move |s| move_task(s, child_id, MoveTo::Parent(None)).unwrap()),
        Box::new(move |s| merge_labels(s, lapto.id, laptop.id).unwrap()),
        Box::new(move |s| trash_project(s, work.id, ProjectDeletion::TrashTasks).unwrap()),
        Box::new(move |s| {
            let mut edited = s.tasks[&child_id].clone();
            edited.priority = Priority::P1;
            edited.notes.push_str(" And a little more.");
            update_task(s.tasks[&child_id].clone(), edited)
        }),
        Box::new(move |s| {
            assign_task(
                s,
                child_id,
                BlockRef::Occurrence(focus_id, date(2026, 5, 6)),
                2026,
            )
            .unwrap()
        }),
    ];

    for operation in operations {
        let before = snapshot(&docs);
        let edit = operation(&before);
        let description = edit.description.clone();

        docs.apply(&edit).unwrap();
        docs.apply(&edit.inverse()).unwrap();

        assert_eq!(snapshot(&docs), before, "after undoing: {description}");
    }
}

#[test]
fn completing_a_recurring_task_survives_a_reload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");

    let work = Project::new("Work", OrderKey::middle());
    let mut task = Task::new(work.id, "Water plants", OrderKey::middle());
    task.due = Some(Due {
        recurrence: Some(Recurrence {
            rrule: "FREQ=DAILY;INTERVAL=3".to_owned(),
            from_completion: false,
        }),
        ..Due::on(date(2026, 5, 6))
    });
    let id = task.id;

    {
        let mut store = Store::open(&path).unwrap();
        store.apply(&lumenna_core::edit::create_project(work)).unwrap();
        store.apply(&create_task(task)).unwrap();
        let edit = complete_task(&store.snapshot().0, id, &now()).unwrap();
        store.apply(&edit).unwrap();
    }

    let store = Store::open(&path).unwrap();
    let (snapshot, report) = store.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(snapshot.tasks[&id].due.as_ref().unwrap().date, date(2026, 5, 9));
    assert_eq!(snapshot.completion_count(id), 1);
    assert!(!snapshot.is_completed(&snapshot.tasks[&id]));
}

#[test]
fn two_devices_completing_different_tasks_both_win() {
    let (mut alice, _, work, _, _) = seeded();
    let first = Task::new(work.id, "Review PR", OrderKey::middle());
    let second = Task::new(work.id, "Write notes", OrderKey::middle());
    let (first_id, second_id) = (first.id, second.id);
    alice.apply(&create_task(first)).unwrap();
    alice.apply(&create_task(second)).unwrap();

    let mut bob = alice.fork();

    alice.apply(&complete_task(&snapshot(&alice), first_id, &now()).unwrap()).unwrap();
    bob.apply(&complete_task(&snapshot(&bob), second_id, &now()).unwrap()).unwrap();

    alice.merge(&mut bob).unwrap();
    let merged = snapshot(&alice);
    assert!(merged.is_completed(&merged.tasks[&first_id]));
    assert!(merged.is_completed(&merged.tasks[&second_id]));
}

#[test]
fn an_undo_on_one_device_reaches_the_other() {
    // Undo history is local to the device, but the *change* it produces is an ordinary
    // edit and syncs like any other.
    let (mut alice, _, work, _, _) = seeded();
    let task = Task::new(work.id, "Review PR", OrderKey::middle());
    let id = task.id;
    alice.apply(&create_task(task)).unwrap();
    let mut bob = alice.fork();

    let edit = complete_task(&snapshot(&alice), id, &now()).unwrap();
    alice.apply(&edit).unwrap();
    bob.merge(&mut alice).unwrap();
    let seen = snapshot(&bob);
    assert!(seen.is_completed(&seen.tasks[&id]));

    alice.apply(&edit.inverse()).unwrap();
    bob.merge(&mut alice).unwrap();
    let seen = snapshot(&bob);
    assert!(!seen.is_completed(&seen.tasks[&id]));
}

#[test]
fn uncompleting_a_recurring_task_restores_the_date_through_the_store() {
    let (mut docs, _, work, _, _) = seeded();
    let mut task = Task::new(work.id, "Water plants", OrderKey::middle());
    task.due = Some(Due {
        recurrence: Some(Recurrence {
            rrule: "FREQ=DAILY;INTERVAL=3".to_owned(),
            from_completion: false,
        }),
        ..Due::on(date(2026, 5, 6))
    });
    let id = task.id;
    docs.apply(&create_task(task)).unwrap();

    docs.apply(&complete_task(&snapshot(&docs), id, &now()).unwrap()).unwrap();
    assert_eq!(snapshot(&docs).tasks[&id].due.as_ref().unwrap().date, date(2026, 5, 9));

    docs.apply(&uncomplete_task(&snapshot(&docs), id).unwrap()).unwrap();
    let after = snapshot(&docs);
    assert_eq!(after.tasks[&id].due.as_ref().unwrap().date, date(2026, 5, 6));
    assert_eq!(after.completion_count(id), 0);
}
