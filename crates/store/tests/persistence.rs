//! §8's file: that what was written comes back, that two processes sharing it see each
//! other, and that compaction changes the representation without discarding anything.

use jiff::civil::{date, time};
use lumenna_core::model::{BlockKind, BlockSeries, Priority, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_store::{Db, DocId, Store};

fn scratch() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn a_written_task_survives_reopening() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");

    let project = Project::inbox();
    let mut task = Task::new(project.id, "survive", OrderKey::middle());
    task.priority = Priority::P1;
    task.notes = "with notes".to_owned();

    {
        let mut store = Store::open(&path).unwrap();
        store
            .write(|docs| {
                docs.put_project(&project, None)?;
                docs.put_task(&task, None)
            })
            .unwrap();
    }

    let store = Store::open(&path).unwrap();
    let (snapshot, report) = store.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(snapshot.tasks[&task.id], task);
    assert_eq!(snapshot.projects[&project.id], project);
}

#[test]
fn years_load_only_when_asked_for() {
    // What keeps the watch viable (§8): a device showing today holds one year, not five.
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let series =
        BlockSeries::one_off("Focus", BlockKind::Work, date(2026, 5, 4), time(9, 0, 0, 0), 60)
            .unwrap();

    {
        let mut store = Store::open(&path).unwrap();
        store.write(|docs| docs.put_series(&series, None)).unwrap();
    }

    let mut store = Store::open(&path).unwrap();
    assert!(store.snapshot().0.series.is_empty(), "the year must not load on startup");
    assert!(store.documents().loaded_years().is_empty());

    store.load_year(2026).unwrap();
    assert_eq!(store.snapshot().0.series[&series.id], series);

    // Asking twice is not a reload.
    store.load_year(2026).unwrap();
    assert_eq!(store.documents().loaded_years(), vec![2026]);
}

#[test]
fn two_processes_sharing_a_file_see_each_others_writes() {
    // The tray app and a CLI invocation, or the daemon and Emacs. No process is privileged
    // and neither is told anything; the second one notices and catches up.
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");

    let mut tray = Store::open(&path).unwrap();
    let mut cli = Store::open(&path).unwrap();

    let project = Project::inbox();
    let task = Task::new(project.id, "added by the CLI", OrderKey::middle());
    cli.write(|docs| {
        docs.put_project(&project, None)?;
        docs.put_task(&task, None)
    })
    .unwrap();

    assert!(tray.snapshot().0.tasks.is_empty(), "not until it looks");
    assert!(tray.refresh().unwrap(), "data_version moved, so there is something to read");
    assert_eq!(tray.snapshot().0.tasks[&task.id], task);

    // Nothing new: refreshing again is a single pragma read and reports no change.
    assert!(!tray.refresh().unwrap());
}

#[test]
fn concurrent_edits_from_two_processes_both_survive() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");

    let project = Project::inbox();
    let task = Task::new(project.id, "shared", OrderKey::middle());
    let mut first = Store::open(&path).unwrap();
    first
        .write(|docs| {
            docs.put_project(&project, None)?;
            docs.put_task(&task, None)
        })
        .unwrap();

    let mut second = Store::open(&path).unwrap();

    let mut renamed = task.clone();
    renamed.title = "renamed".to_owned();
    first.write(|docs| docs.put_task(&renamed, Some(&task))).unwrap();

    let mut estimated = task.clone();
    estimated.estimate_mins = Some(30);
    second.write(|docs| docs.put_task(&estimated, Some(&task))).unwrap();

    first.refresh().unwrap();
    second.refresh().unwrap();

    for store in [&first, &second] {
        let merged = &store.snapshot().0.tasks[&task.id];
        assert_eq!(merged.title, "renamed");
        assert_eq!(merged.estimate_mins, Some(30));
    }
}

#[test]
fn a_change_written_twice_is_stored_once() {
    // Changes are immutable and named by their hash, which is what makes replaying a sync
    // exchange or a crash mid-write harmless.
    let mut db = Db::open_in_memory().unwrap();
    let hash = automerge::ChangeHash([7; 32]);
    let changes = vec![(hash, vec![1, 2, 3])];

    db.append_changes(DocId::Core, &changes).unwrap();
    db.append_changes(DocId::Core, &changes).unwrap();
    assert_eq!(db.change_count(DocId::Core).unwrap(), 1);
}

#[test]
fn compaction_replaces_the_representation_without_losing_anything() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");

    let project = Project::inbox();
    let tasks: Vec<Task> = (0..8)
        .map(|i| Task::new(project.id, format!("t{i}"), OrderKey::middle()))
        .collect();

    let mut store = Store::open(&path).unwrap();
    store.write(|docs| docs.put_project(&project, None)).unwrap();
    for task in &tasks {
        store.write(|docs| docs.put_task(task, None)).unwrap();
    }
    let before = store.snapshot().0;
    assert!(store.db().change_count(DocId::Core).unwrap() > 1);

    // Compact on demand rather than by writing two thousand changes to trip the threshold.
    store.compact_document(DocId::Core).unwrap();
    assert_eq!(
        store.db().change_count(DocId::Core).unwrap(),
        0,
        "the changes it subsumes are gone"
    );

    let reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.snapshot().0, before, "and nothing else changed");
}

#[test]
fn an_empty_profile_is_usable_immediately() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let store = Store::open(&path).unwrap();
    let (snapshot, report) = store.snapshot();
    assert!(report.is_clean());
    assert!(snapshot.tasks.is_empty());
    assert!(snapshot.inbox().is_none(), "Inbox is a record, not an assumption (§3.4)");
    assert_eq!(snapshot.settings, lumenna_core::model::Settings::default());
}

#[test]
fn a_fresh_profile_records_its_genesis() {
    // Otherwise the first real write would carry it, and two devices that both started
    // fresh would each have an unrecorded genesis to reconcile.
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    {
        let _ = Store::open(&path).unwrap();
    }
    let db = Db::open(&path).unwrap();
    let stored = db.stored_documents().unwrap();
    assert!(stored.contains(&"core".to_owned()), "{stored:?}");
    assert!(stored.contains(&"devices".to_owned()), "{stored:?}");
}
