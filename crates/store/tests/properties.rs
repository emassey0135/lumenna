//! Properties of what leaves the store, over histories nobody writes by hand.
//!
//! A backup is the whole history and must come back whole: restoring one into an empty store
//! gives the same tasks, and restoring it again changes nothing. An export is the present
//! only, and is how someone sends a list to another person: a task in the trash must never be
//! in one, in any format. That is a privacy guarantee, not formatting, so it is checked as
//! one: every task's title carries its own identifier, and no export may hold a trashed one's.

use jiff::{Timestamp, Zoned};
use lumenna_core::id::TaskId;
use lumenna_core::model::{Priority, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::time;
use lumenna_store::Store;
use lumenna_store::export::{export_json, ics, markdown, org};
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
}

fn op() -> impl Strategy<Value = Op> {
    let i = 0..TASKS;
    prop_oneof![
        (i.clone(), any::<u8>()).prop_map(|(a, n)| Op::Rename(a, n)),
        (i.clone(), prop::option::of(0..TASKS)).prop_map(|(a, b)| Op::Reparent(a, b)),
        i.clone().prop_map(Op::Note),
        (i.clone(), 1u8..5).prop_map(|(a, p)| Op::Priority(a, p)),
        i.clone().prop_map(Op::ToggleTrash),
        i.prop_map(Op::Complete),
    ]
}

/// A store with six tasks, each titled with its own identifier so an export cannot hide one.
fn store() -> (Store, Vec<TaskId>) {
    let mut store = Store::open_in_memory().unwrap();
    let mut ids = Vec::new();
    let mut order = OrderKey::middle();
    store
        .write(|docs| {
            for i in 0..TASKS {
                let mut task = Task::new(Project::inbox().id, String::new(), order.clone());
                task.title = format!("<{}> t{i}", task.id);
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
fn apply(store: &mut Store, ids: &[TaskId], op: Op, step: usize) {
    let (snapshot, _) = store.snapshot();
    let index = match op {
        Op::Rename(i, _) | Op::Reparent(i, _) | Op::Note(i) | Op::Priority(i, _) | Op::ToggleTrash(i) | Op::Complete(i) => i,
    };
    let Some(before) = snapshot.tasks.get(&ids[index]).cloned() else { return };
    if let Op::Complete(_) = op {
        let completion = lumenna_core::model::TaskCompletion::new(before.id, None);
        store.write(|docs| docs.put_completion(&completion, None)).unwrap();
        return;
    }
    let mut after = before.clone();
    match op {
        Op::Rename(_, n) => after.title = format!("<{}> renamed {n} at {step}", after.id),
        Op::Reparent(_, parent) => after.parent_id = parent.map(|p| ids[p]).filter(|p| *p != after.id),
        Op::Note(_) => after.notes.push_str(&format!("[{step}]")),
        Op::Priority(_, p) => after.priority = Priority::from_u8(p),
        Op::ToggleTrash(_) => after.deleted_at = if after.deleted_at.is_some() { None } else { Some(time::now()) },
        Op::Complete(_) => unreachable!(),
    }
    store.write(|docs| docs.put_task(&after, Some(&before))).unwrap();
}

proptest! {
    // A store per case is slow, so few by default; PROPTEST_CASES asks for more.
    #![proptest_config(ProptestConfig {
        cases: std::env::var("PROPTEST_CASES").ok().and_then(|n| n.parse().ok()).unwrap_or(64),
        ..ProptestConfig::default()
    })]

    #[test]
    fn a_backup_restored_into_an_empty_store_is_the_same_store(ops in prop::collection::vec(op(), 0..40)) {
        let (mut store, ids) = store();
        for (step, op) in ops.into_iter().enumerate() {
            apply(&mut store, &ids, op, step);
        }
        let bytes = store.backup().unwrap();

        let mut restored = Store::open_in_memory().unwrap();
        restored.restore(&bytes).unwrap();
        let (original, _) = store.snapshot();
        let (back, _) = restored.snapshot();
        prop_assert_eq!(&back.tasks, &original.tasks);
        prop_assert_eq!(&back.completions, &original.completions);
        prop_assert_eq!(&back.projects, &original.projects);

        // A backup restored again, into the store it came from, changes nothing: restoring
        // merges, and can never take back a later change.
        restored.restore(&bytes).unwrap();
        prop_assert_eq!(&restored.snapshot().0.tasks, &original.tasks);
        store.restore(&bytes).unwrap();
        prop_assert_eq!(&store.snapshot().0.tasks, &original.tasks);
    }

    #[test]
    fn no_export_ever_holds_a_task_in_the_trash(ops in prop::collection::vec(op(), 0..40)) {
        let (mut store, ids) = store();
        for (step, op) in ops.into_iter().enumerate() {
            apply(&mut store, &ids, op, step);
        }
        let (snapshot, _) = store.snapshot();
        let now = Zoned::now();
        let exports = [
            ("JSON", export_json(&snapshot, Timestamp::now())),
            ("Markdown", markdown(&snapshot, &now)),
            ("org", org(&snapshot, &now)),
            ("ics", ics(&snapshot, Timestamp::now())),
        ];
        for task in snapshot.tasks.values() {
            let id = task.id.to_string();
            for (format, text) in &exports {
                if task.is_deleted() {
                    prop_assert!(!text.contains(&id), "the {} export holds {}, which is in the trash", format, task.title);
                }
            }
            if !task.is_deleted() {
                // What is not in the trash is there, so the check above is not passing by
                // exporting nothing.
                prop_assert!(exports[0].1.contains(&id), "the JSON export lacks {}", task.title);
            }
        }
    }
}
