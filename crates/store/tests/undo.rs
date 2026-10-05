//! Undo and redo: saved on the device, shared by its processes, never synced, and
//! always rebased against what the store holds now.

use jiff::Zoned;
use jiff::civil::date;
use lumenna_core::ProjectId;
use lumenna_core::edit::{complete_task, create_task, update_task};
use lumenna_core::model::{Priority, Task};
use lumenna_core::order::OrderKey;
use lumenna_store::Store;
use lumenna_store::undo::Step;

fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

fn profile() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    (dir, path)
}

fn add(store: &mut Store, title: &str) -> Task {
    let task = Task::new(ProjectId::INBOX, title, OrderKey::middle());
    store.apply_recorded(&create_task(task.clone())).unwrap();
    task
}

fn done(step: Step) -> lumenna_store::undo::Reverted {
    match step {
        Step::Done(reverted) => reverted,
        other => panic!("expected an undo, got {other:?}"),
    }
}

#[test]
fn undo_reverses_and_redo_reapplies() {
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    let task = add(&mut store, "Review PR");
    let edit = complete_task(&store.snapshot().0, task.id, &now()).unwrap();
    store.apply_recorded(&edit).unwrap();
    let completed = |s: &Store| s.snapshot().0.is_completed(&s.snapshot().0.tasks[&task.id]);
    assert!(completed(&store));

    let undone = done(store.undo().unwrap());
    assert_eq!(undone.description, "Completed Review PR");
    assert!(undone.kept.is_empty());
    assert!(!completed(&store));

    done(store.redo().unwrap());
    assert!(completed(&store));
    assert_eq!(store.undo_depths().unwrap(), (2, 0));
}

#[test]
fn the_history_outlives_the_process_that_wrote_it() {
    // A one-shot `lum` is one process per command. `lum task done 1` then `lum undo` is two.
    let (_dir, path) = profile();
    let task = {
        let mut store = Store::open(&path).unwrap();
        add(&mut store, "Review PR")
    };
    let mut later = Store::open(&path).unwrap();
    done(later.undo().unwrap());
    assert!(!later.snapshot().0.tasks.contains_key(&task.id), "the add was undone");
}

#[test]
fn every_process_on_the_device_shares_one_history() {
    let (_dir, path) = profile();
    let mut app = Store::open(&path).unwrap();
    let mut terminal = Store::open(&path).unwrap();
    let task = add(&mut app, "added in the app");
    let undone = done(terminal.undo().unwrap());
    assert_eq!(undone.description, "Added added in the app");
    app.refresh().unwrap();
    assert!(!app.snapshot().0.tasks.contains_key(&task.id));
}

#[test]
fn a_field_changed_since_is_kept_and_said() {
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    let task = add(&mut store, "Review PR");

    // This device: priority and title.
    let before = store.snapshot().0.tasks[&task.id].clone();
    let mut edited = before.clone();
    edited.priority = Priority::P1;
    edited.title = "Review the PR".to_owned();
    store.apply_recorded(&update_task(before, edited.clone())).unwrap();

    // Since then, merged in from elsewhere without going through this history: a new title.
    let mut elsewhere = edited.clone();
    elsewhere.title = "Review PR 42".to_owned();
    store.write(|docs| docs.put_task(&elsewhere, Some(&edited))).unwrap();

    let undone = done(store.undo().unwrap());
    let now = &store.snapshot().0.tasks[&task.id];
    assert_eq!(now.priority, Priority::P4, "the priority went back");
    assert_eq!(now.title, "Review PR 42", "the newer title was not overwritten");
    assert_eq!(undone.kept, vec!["kept task Review PR 42's title, changed since".to_owned()]);
}

#[test]
fn doing_something_new_clears_what_could_have_been_redone() {
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    add(&mut store, "first");
    done(store.undo().unwrap());
    assert_eq!(store.undo_depths().unwrap(), (0, 1));
    add(&mut store, "second");
    assert_eq!(store.undo_depths().unwrap(), (1, 0));
    assert_eq!(store.redo().unwrap(), Step::Nothing);
}

#[test]
fn the_history_is_bounded() {
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    for i in 0..(lumenna_store::undo::DEPTH + 5) {
        add(&mut store, &format!("t{i}"));
    }
    assert_eq!(store.undo_depths().unwrap(), (lumenna_store::undo::DEPTH, 0));
}

#[test]
fn an_empty_history_says_so() {
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.undo().unwrap(), Step::Nothing);
    assert_eq!(store.redo().unwrap(), Step::Nothing);
}

#[test]
fn an_entry_this_version_cannot_read_is_dropped_not_guessed_at() {
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    add(&mut store, "older");
    {
        // An entry as some other version of the format might have written it.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO undo (format, recorded_at, edit) VALUES (99, 0, '{}')",
            [],
        )
        .unwrap();
    }
    assert_eq!(store.undo().unwrap(), Step::Unreadable);
    assert_eq!(done(store.undo().unwrap()).description, "Added older", "the one before is next");
}

#[test]
fn the_history_stays_on_the_device() {
    // A backup is Automerge documents and nothing else; restoring one elsewhere brings the
    // edits but none of this device's history of making them.
    let (_dir, path) = profile();
    let mut store = Store::open(&path).unwrap();
    add(&mut store, "Review PR");
    let backup = store.backup().unwrap();

    let mut elsewhere = Store::open_in_memory().unwrap();
    elsewhere.restore(&backup).unwrap();
    assert_eq!(elsewhere.undo_depths().unwrap(), (0, 0));
    assert_eq!(elsewhere.snapshot().0.tasks.len(), 1);
}

#[test]
fn every_kind_of_edit_undoes_to_exactly_where_it_was_and_redoes_to_where_it_went() {
    use lumenna_core::edit::{
        MoveTo, ProjectDeletion, assign_task, merge_labels, move_task, trash_project,
        update_settings,
    };
    use lumenna_core::model::{
        BlockKind, BlockRef, BlockSeries, Due, Label, Project, Recurrence,
    };
    use lumenna_core::snapshot::Snapshot;

    type Operation = Box<dyn Fn(&Snapshot) -> lumenna_core::edit::Edit>;

    let mut store = Store::open_in_memory().unwrap();
    let work = Project::new("Work", OrderKey::middle());
    let laptop = Label::new("laptop", OrderKey::middle());
    let lapto = Label::new("lapto", OrderKey::after(&laptop.order));
    let mut parent = Task::new(work.id, "Weekly review", OrderKey::middle());
    parent.due = Some(Due {
        recurrence: Some(Recurrence { rrule: "FREQ=WEEKLY".to_owned(), from_completion: false }),
        ..Due::on(date(2026, 5, 6))
    });
    let mut child = Task::new(work.id, "Clear the inbox", OrderKey::middle());
    child.parent_id = Some(parent.id);
    child.labels.insert(lapto.id);
    let mut focus =
        BlockSeries::one_off("Focus", BlockKind::Work, date(2026, 5, 1), jiff::civil::time(9, 0, 0, 0), 90)
            .unwrap();
    focus.rrule = Some("FREQ=DAILY".to_owned());
    focus.end_date = None;
    store
        .write(|docs| {
            docs.put_project(&work, None)?;
            docs.put_label(&laptop, None)?;
            docs.put_label(&lapto, None)?;
            docs.put_task(&parent, None)?;
            docs.put_task(&child, None)?;
            docs.put_series(&focus, None)
        })
        .unwrap();

    let (p, c, w, f) = (parent.id, child.id, work.id, focus.id);
    let (from, into) = (lapto.id, laptop.id);
    let operations: Vec<Operation> = vec![
        Box::new(move |s| complete_task(s, p, &now()).unwrap()),
        Box::new(move |s| move_task(s, c, MoveTo::Project(ProjectId::INBOX)).unwrap()),
        Box::new(move |s| merge_labels(s, from, into).unwrap()),
        Box::new(move |s| trash_project(s, w, ProjectDeletion::TrashTasks).unwrap()),
        Box::new(move |s| {
            assign_task(s, c, BlockRef::Occurrence(f, date(2026, 5, 6)), 2026).unwrap()
        }),
        Box::new(move |s| {
            let mut after = s.settings.clone();
            after.cascade_complete_subtasks = false;
            update_settings(s.settings.clone(), after)
        }),
    ];

    for operation in operations {
        store.load_all_years().unwrap();
        let before = store.snapshot().0;
        let edit = operation(&before);
        let description = edit.description.clone();
        store.apply_recorded(&edit).unwrap();
        let after = store.snapshot().0;

        let undone = done(store.undo().unwrap());
        assert!(undone.kept.is_empty(), "{description}: {:?}", undone.kept);
        assert_eq!(store.snapshot().0, before, "undoing: {description}");

        done(store.redo().unwrap());
        assert_eq!(store.snapshot().0, after, "redoing: {description}");
    }
}
