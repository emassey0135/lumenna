//! The snapshot a store keeps between changes (`Store::snapshot`, each document's part read
//! again only where Automerge's diff says it changed) must always be the snapshot read afresh
//! from the documents. A part that kept one stale field would be a wrong answer every app gave
//! until the record changed again, so it is checked after every step of histories nobody
//! writes by hand: edits, purges, blocks in two years, exceptions stored a key per field,
//! settings, changes merged in from another device, another process writing to the same file,
//! and the store opened again, starting from the parts an earlier process kept on disk.

use jiff::civil::{date, time};
use lumenna_core::id::TaskId;
use lumenna_core::model::{
    BlockAssignment, BlockException, BlockKind, BlockRef, BlockSeries, Due, ExceptionAction, Priority, Project, Task,
    TaskCompletion,
};
use lumenna_core::order::OrderKey;
use lumenna_core::time;
use std::path::Path;

use lumenna_store::Store;
use proptest::prelude::*;

const TASKS: usize = 6;

#[derive(Debug, Clone, Copy)]
enum Op {
    Rename(usize, u8),
    Reparent(usize, Option<usize>),
    Note(usize),
    Priority(usize, u8),
    ToggleTrash(usize),
    Complete(usize),
    Purge(usize),
    /// A one-off block in 2026 or 2027, on a day of May.
    Block(bool, i8),
    /// A task put in one of the blocks so far.
    Sitting(usize, usize),
    /// One of the blocks cancelled on a day: an exception, stored a key per field.
    Cancel(usize, i8),
    Setting(bool),
    /// Another device renames a task while this one does something else, and the two merge.
    Merged(usize, u8),
    LoadYears,
    /// Another process on this device renames a task in the same file.
    Elsewhere(usize, u8),
    /// The store closed and opened again: it starts from the parts kept on disk.
    Reopen,
    /// A task's due date set to a day of June, which may put it off: the postponements' tally.
    Due(usize, i8),
}

fn op() -> impl Strategy<Value = Op> {
    let i = 0..TASKS;
    prop_oneof![
        (i.clone(), any::<u8>()).prop_map(|(a, n)| Op::Rename(a, n)),
        (i.clone(), prop::option::of(0..TASKS)).prop_map(|(a, b)| Op::Reparent(a, b)),
        i.clone().prop_map(Op::Note),
        (i.clone(), 1u8..5).prop_map(|(a, p)| Op::Priority(a, p)),
        i.clone().prop_map(Op::ToggleTrash),
        i.clone().prop_map(Op::Complete),
        i.clone().prop_map(Op::Purge),
        (any::<bool>(), 1i8..29).prop_map(|(y, d)| Op::Block(y, d)),
        (0..4usize, i.clone()).prop_map(|(b, t)| Op::Sitting(b, t)),
        (0..4usize, 1i8..29).prop_map(|(b, d)| Op::Cancel(b, d)),
        any::<bool>().prop_map(Op::Setting),
        (i.clone(), any::<u8>()).prop_map(|(a, n)| Op::Merged(a, n)),
        Just(Op::LoadYears),
        (i, any::<u8>()).prop_map(|(a, n)| Op::Elsewhere(a, n)),
        Just(Op::Reopen),
        (0..TASKS, 1i8..29).prop_map(|(a, d)| Op::Due(a, d)),
    ]
}

fn store(path: &Path) -> (Store, Vec<TaskId>) {
    let mut store = Store::open(path).unwrap();
    let mut ids = Vec::new();
    let mut order = OrderKey::middle();
    store
        .write(|docs| {
            for i in 0..TASKS {
                let task = Task::new(Project::inbox().id, format!("t{i}"), order.clone());
                order = OrderKey::after(&order);
                ids.push(task.id);
                docs.put_task(&task, None)?;
            }
            Ok(())
        })
        .unwrap();
    (store, ids)
}

/// Applies `op` as a device would: the version read, the one thing changed, the difference written.
fn apply(store: &mut Store, path: &Path, ids: &[TaskId], blocks: &mut Vec<(i16, BlockSeries)>, op: Op, step: usize) {
    let snapshot = store.documents().snapshot().0;
    let task = |i: usize| snapshot.tasks.get(&ids[i]).cloned();
    match op {
        Op::Rename(i, _) | Op::Reparent(i, _) | Op::Note(i) | Op::Priority(i, _) | Op::ToggleTrash(i) | Op::Due(i, _) => {
            let Some(before) = task(i) else { return };
            let mut after = before.clone();
            match op {
                Op::Rename(_, n) => after.title = format!("renamed {n} at {step}"),
                Op::Reparent(_, parent) => after.parent_id = parent.map(|p| ids[p]).filter(|p| *p != after.id),
                Op::Note(_) => after.notes.push_str(&format!("[{step}]")),
                Op::Priority(_, p) => after.priority = Priority::from_u8(p),
                Op::Due(_, day) => {
                    after.due = Some(Due { date: date(2026, 6, day), time: None, timezone: None, recurrence: None });
                }
                _ => after.deleted_at = if after.deleted_at.is_some() { None } else { Some(time::now()) },
            }
            store.write(|docs| docs.put_task(&after, Some(&before))).unwrap();
        }
        Op::Complete(i) => {
            let Some(before) = task(i) else { return };
            let completion = TaskCompletion::new(before.id, None);
            store.write(|docs| docs.put_completion(&completion, None)).unwrap();
        }
        Op::Purge(i) => {
            if task(i).is_some() {
                store.write(|docs| docs.purge_task(&ids[i])).unwrap();
            }
        }
        Op::Block(later, day) => {
            let year = if later { 2027 } else { 2026 };
            let series = BlockSeries::one_off(format!("b{step}"), BlockKind::Work, date(year, 5, day), time(9, 0, 0, 0), 60)
                .unwrap();
            store.write(|docs| docs.put_series(&series, None)).unwrap();
            blocks.push((year, series));
        }
        Op::Sitting(b, t) => {
            let Some((year, series)) = blocks.get(b) else { return };
            let sitting = BlockAssignment::new(BlockRef::OneOff(series.id), ids[t], OrderKey::middle());
            store.write(|docs| docs.put_assignment(*year, &sitting, None)).unwrap();
        }
        Op::Cancel(b, day) => {
            let Some((year, series)) = blocks.get(b) else { return };
            let exception = BlockException {
                series_id: series.id,
                original_date: date(*year, 5, day),
                action: ExceptionAction::Cancelled,
            };
            store.write(|docs| docs.put_exception(&exception, None)).unwrap();
        }
        Op::Setting(cascade) => {
            let before = snapshot.settings.clone();
            let mut after = before.clone();
            after.cascade_complete_subtasks = cascade;
            store.write(|docs| docs.put_settings(&after, Some(&before))).unwrap();
        }
        Op::Merged(i, n) => {
            let mut other = Store::open_in_memory().unwrap();
            other.restore(&store.backup().unwrap()).unwrap();
            if let Some(before) = other.documents().snapshot().0.tasks.get(&ids[i]).cloned() {
                let mut after = before.clone();
                after.title = format!("elsewhere {n} at {step}");
                other.write(|docs| docs.put_task(&after, Some(&before))).unwrap();
            }
            store.restore(&other.backup().unwrap()).unwrap();
        }
        Op::LoadYears => store.load_all_years().unwrap(),
        Op::Elsewhere(i, n) => {
            let mut other = Store::open(path).unwrap();
            if let Some(before) = other.snapshot().0.tasks.get(&ids[i]).cloned() {
                let mut after = before.clone();
                after.title = format!("another process {n} at {step}");
                other.write(|docs| docs.put_task(&after, Some(&before))).unwrap();
            }
            drop(other);
            store.refresh().unwrap();
        }
        Op::Reopen => *store = Store::open(path).unwrap(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES").ok().and_then(|n| n.parse().ok()).unwrap_or(64),
        ..ProptestConfig::default()
    })]

    #[test]
    fn a_snapshot_kept_between_changes_is_always_the_one_read_afresh(ops in prop::collection::vec(op(), 1..40)) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.sqlite");
        let (mut store, ids) = store(&path);
        let mut blocks = Vec::new();
        // Read once first, so every step after follows a change from a part already kept.
        let _ = store.snapshot();
        for (step, op) in ops.into_iter().enumerate() {
            apply(&mut store, &path, &ids, &mut blocks, op, step);
            let (kept, report) = store.snapshot();
            let (mut fresh, fresh_report) = store.documents().snapshot();
            fresh.postponed.clone_from(&kept.postponed);
            prop_assert_eq!(&kept, &fresh, "after {:?}", op);
            let skipped = |mut s: Vec<lumenna_store::Skipped>| {
                s.sort_by(|a, b| (a.doc, a.collection, &a.key).cmp(&(b.doc, b.collection, &b.key)));
                s
            };
            prop_assert_eq!(skipped(report.skipped), skipped(fresh_report.skipped));
            if matches!(op, Op::Reopen) {
                // The tally read on from what was kept is the tally read from the start.
                prop_assert_eq!(&kept.postponed, &counted_afresh(&path), "postponements after reopening");
            }
        }
    }
}

/// The postponements a store at `path` counts from the start of its history: a copy of it with
/// nothing kept.
fn counted_afresh(path: &Path) -> std::collections::BTreeMap<TaskId, u32> {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("profile.sqlite");
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute("VACUUM INTO ?1", [copy.to_str().unwrap()]).unwrap();
    let db = rusqlite::Connection::open(&copy).unwrap();
    db.execute("DELETE FROM hydrated", []).unwrap();
    drop(db);
    Store::open(&copy).unwrap().snapshot().0.postponed
}

/// A store with tasks, closed once so its part of the snapshot is kept on disk, then that part
/// with its tasks taken out, kept as `build` at `heads` (else as it was kept), and the store
/// opened again: a part used where it should not be shows as no tasks.
fn reopened_over_a_planted_part(build: Option<&str>, heads: Option<Vec<u8>>) -> Store {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let (mut store, _) = store(&path);
    let _ = store.snapshot();
    drop(store);
    let db = rusqlite::Connection::open(&path).unwrap();
    let (kept_build, kept_heads, part): (String, Vec<u8>, Vec<u8>) = db
        .query_row("SELECT build, heads, part FROM hydrated WHERE doc_id = 'core'", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    let mut part: serde_json::Value = serde_json::from_slice(&part).unwrap();
    part["tasks"] = serde_json::json!([]);
    db.execute(
        "UPDATE hydrated SET build = ?1, heads = ?2, part = ?3 WHERE doc_id = 'core'",
        rusqlite::params![build.unwrap_or(&kept_build), heads.unwrap_or(kept_heads), serde_json::to_vec(&part).unwrap()],
    )
    .unwrap();
    drop(db);
    let reopened = Store::open(&path).unwrap();
    // The folder outlives the test's store, which is all that reads it.
    std::mem::forget(dir);
    reopened
}

#[test]
fn a_part_kept_by_this_build_at_the_documents_heads_is_what_a_reopened_store_starts_from() {
    let mut store = reopened_over_a_planted_part(None, None);
    assert!(store.snapshot().0.tasks.is_empty(), "the planted part, not a reading of the document");
}

#[test]
fn a_part_kept_by_another_build_is_not_used() {
    let mut store = reopened_over_a_planted_part(Some("another build"), None);
    assert_eq!(store.snapshot().0.tasks.len(), TASKS, "read from the document, not the planted part");
}

#[test]
fn a_part_kept_at_heads_the_document_lacks_is_not_used() {
    let mut store = reopened_over_a_planted_part(None, Some(vec![7; 32]));
    assert_eq!(store.snapshot().0.tasks.len(), TASKS, "read from the document, not the planted part");
}
