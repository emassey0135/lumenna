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
    // The Inbox is a record (§3.4), and it is the same record on every device: it comes
    // with the store, from a deterministic change, rather than being minted by a client.
    assert_eq!(snapshot.inbox(), Some(&lumenna_core::model::Project::inbox()));
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

#[test]
fn compacting_keeps_what_another_process_wrote_since_its_last_look() {
    // The compacting process has not refreshed, so its document lacks the other's task. A
    // compaction that saved only what it held, then deleted every change, would lose it.
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let mut first = Store::open(&path).unwrap();
    let mut second = Store::open(&path).unwrap();

    let mine = Task::new(lumenna_core::ProjectId::INBOX, "mine", OrderKey::middle());
    let theirs = Task::new(lumenna_core::ProjectId::INBOX, "theirs", OrderKey::middle());
    first.write(|docs| docs.put_task(&mine, None)).unwrap();
    second.write(|docs| docs.put_task(&theirs, None)).unwrap();

    first.compact_document(DocId::Core).unwrap();
    assert!(first.refresh().unwrap(), "what compaction took in is still reported");

    let reopened = Store::open(&path).unwrap();
    let tasks = reopened.snapshot().0.tasks;
    assert!(tasks.contains_key(&mine.id));
    assert!(tasks.contains_key(&theirs.id), "the other process's task was compacted away");
}

#[test]
fn a_running_process_keeps_seeing_writes_after_another_compacts() {
    // Compaction deletes the newest rows. If their rowids were handed out again, a process
    // whose cursor had already passed them would skip every write after.
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let mut writer = Store::open(&path).unwrap();
    let mut watcher = Store::open(&path).unwrap();

    for i in 0..5 {
        let task = Task::new(lumenna_core::ProjectId::INBOX, format!("t{i}"), OrderKey::middle());
        writer.write(|docs| docs.put_task(&task, None)).unwrap();
    }
    assert!(watcher.refresh().unwrap());

    writer.compact_document(DocId::Core).unwrap();
    let late = Task::new(lumenna_core::ProjectId::INBOX, "after compaction", OrderKey::middle());
    writer.write(|docs| docs.put_task(&late, None)).unwrap();

    assert!(watcher.refresh().unwrap());
    assert!(watcher.snapshot().0.tasks.contains_key(&late.id));
}

#[test]
fn a_process_that_missed_changes_now_compacted_still_receives_them() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let mut writer = Store::open(&path).unwrap();
    let mut watcher = Store::open(&path).unwrap();

    let task = Task::new(lumenna_core::ProjectId::INBOX, "written, then compacted", OrderKey::middle());
    writer.write(|docs| docs.put_task(&task, None)).unwrap();
    writer.compact_document(DocId::Core).unwrap();

    // Its rows are gone; the snapshot is the only place the change remains.
    assert!(watcher.refresh().unwrap());
    assert!(watcher.snapshot().0.tasks.contains_key(&task.id));
}

#[test]
fn a_changes_table_from_before_autoincrement_is_migrated_intact() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let task = Task::new(lumenna_core::ProjectId::INBOX, "kept", OrderKey::middle());
    {
        let mut store = Store::open(&path).unwrap();
        store.write(|docs| docs.put_task(&task, None)).unwrap();
    }
    {
        // Put the table back the way the first release made it.
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE old (rowid INTEGER PRIMARY KEY, doc_id TEXT NOT NULL,
                              hash BLOB NOT NULL, data BLOB NOT NULL, UNIQUE (doc_id, hash));
             INSERT INTO old SELECT rowid, doc_id, hash, data FROM changes;
             DROP TABLE changes;
             ALTER TABLE old RENAME TO changes;",
        )
        .unwrap();
    }

    let store = Store::open(&path).unwrap();
    assert!(store.snapshot().0.tasks.contains_key(&task.id));
    let conn = rusqlite::Connection::open(&path).unwrap();
    let sql: String = conn
        .query_row("SELECT sql FROM sqlite_master WHERE name = 'changes'", [], |r| r.get(0))
        .unwrap();
    assert!(sql.contains("AUTOINCREMENT"), "{sql}");
}

fn daily(title: &str, from: jiff::civil::Date) -> BlockSeries {
    let mut series =
        BlockSeries::one_off(title, BlockKind::Work, from, time(9, 0, 0, 0), 60).unwrap();
    series.rrule = Some("FREQ=DAILY".to_owned());
    series.end_date = None;
    series
}

#[test]
fn a_routine_begun_last_year_is_still_there_on_new_years_day() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let routine = daily("Morning pages", date(2026, 10, 3));
    let meeting =
        BlockSeries::one_off("Meeting", BlockKind::Event, date(2025, 3, 1), time(9, 0, 0, 0), 30)
            .unwrap();
    {
        let mut store = Store::open(&path).unwrap();
        store
            .write(|docs| {
                docs.put_series(&routine, None)?;
                docs.put_series(&meeting, None)
            })
            .unwrap();
    }

    let mut store = Store::open(&path).unwrap();
    store.load_year(2027).unwrap();
    let day = store.snapshot().0.day(date(2027, 1, 1)).unwrap();
    assert_eq!(day.len(), 1, "the routine disappeared at New Year");
    assert_eq!(day[0].series_id, routine.id);
    assert_eq!(
        store.documents().loaded_years(),
        vec![2026, 2027],
        "a year with only one-off blocks stays on disk"
    );
}

#[test]
fn a_store_from_before_the_index_has_its_routines_found() {
    // Write a blocks year directly, alongside a `core` that has only its genesis — what a
    // store looked like before the recurring-year index existed.
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let routine = daily("Walk", date(2026, 6, 1));
    {
        let mut db = Db::open(&path).unwrap();
        let mut docs = lumenna_store::Documents::new();
        docs.put_series(&routine, None).unwrap();
        let year = docs.blocks(2026).changes_since(&[]);
        db.append_changes(DocId::Blocks(2026), &year).unwrap();
        let core = lumenna_store::Doc::new(DocId::Core).changes_since(&[]);
        db.append_changes(DocId::Core, &core).unwrap();
    }

    let mut store = Store::open(&path).unwrap();
    assert_eq!(store.documents().recurring_years(), vec![2026]);
    store.load_year(2027).unwrap();
    assert_eq!(store.snapshot().0.day(date(2027, 2, 1)).unwrap().len(), 1);
}

#[test]
fn moving_a_block_into_another_year_moves_the_whole_record() {
    let dir = scratch();
    let path = dir.path().join("profile.sqlite");
    let before =
        BlockSeries::one_off("Retreat", BlockKind::Work, date(2026, 12, 30), time(9, 0, 0, 0), 60)
            .unwrap();
    let mut after = before.clone();
    after.start_date = date(2027, 1, 4);
    after.end_date = Some(date(2027, 1, 4));
    {
        let mut store = Store::open(&path).unwrap();
        store.write(|docs| docs.put_series(&before, None)).unwrap();
        store.write(|docs| docs.put_series(&after, Some(&before))).unwrap();
    }

    let mut store = Store::open(&path).unwrap();
    store.load_all_years().unwrap();
    let (snapshot, report) = store.snapshot();
    assert!(report.is_clean(), "a partial record was left behind: {report:?}");
    assert_eq!(snapshot.series[&after.id], after);
    assert_eq!(snapshot.series.len(), 1);
}
