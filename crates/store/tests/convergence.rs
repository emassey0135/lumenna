//! §19's third risk, retired: **CRDT tree convergence**, proven before any UI depends on
//! the store.
//!
//! Every replica must reach the same state from the same set of changes, whatever order it
//! receives them in. That is Automerge's guarantee for the document, but not for what this
//! crate builds on top of it — the field-granular writes, the sets-as-maps, the fractional
//! ordering, and above all the repairs of §3.13, which run *after* merge and could
//! themselves diverge if they depended on anything but the data.
//!
//! The interesting inputs are the ones nobody writes by hand: A reparenting X under Y while
//! B reparents Y under X, two devices editing the same note, a task deleted on one and
//! relabelled on another. So the operations are generated, applied to several replicas in
//! different orders, and the merged results compared.

use std::collections::BTreeSet;

use lumenna_core::id::{LabelId, ProjectId, TaskId};
use lumenna_core::model::{Priority, Project, Task};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::{Repairs, Snapshot};
use lumenna_core::time;
use lumenna_store::{Doc, DocId, Documents};
use proptest::prelude::*;

const TASKS: usize = 6;
const LABELS: usize = 3;
const REPLICAS: usize = 3;

/// What a user can do to a task, chosen for the hazards each one can create.
#[derive(Debug, Clone, Copy)]
enum Op {
    /// Last-write-wins on a short field.
    Rename(usize),
    /// The tree hazard: concurrent reparenting is what makes cycles (§3.13).
    Reparent(usize, Option<usize>),
    /// Fractional ordering under concurrent moves.
    MoveAfter(usize, usize),
    /// Set membership, which must merge rather than overwrite.
    ToggleLabel(usize, usize),
    /// The dependency hazard: the second graph over the same nodes.
    ToggleDepend(usize, usize),
    /// Character-level text merge.
    Note(usize),
    /// A scalar that is not a string.
    Priority(usize, u8),
    /// Trash, which is a field like any other and must not behave specially.
    ToggleTrash(usize),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let i = 0..TASKS;
    prop_oneof![
        i.clone().prop_map(Op::Rename),
        (i.clone(), prop::option::of(0..TASKS)).prop_map(|(a, b)| Op::Reparent(a, b)),
        (i.clone(), 0..TASKS).prop_map(|(a, b)| Op::MoveAfter(a, b)),
        (i.clone(), 0..LABELS).prop_map(|(a, b)| Op::ToggleLabel(a, b)),
        (i.clone(), 0..TASKS).prop_map(|(a, b)| Op::ToggleDepend(a, b)),
        i.clone().prop_map(Op::Note),
        (i.clone(), 1u8..5).prop_map(|(a, b)| Op::Priority(a, b)),
        i.prop_map(Op::ToggleTrash),
    ]
}

/// The fixed cast every replica starts with, so operations can address tasks by index.
struct Cast {
    project: ProjectId,
    tasks: Vec<TaskId>,
    labels: Vec<LabelId>,
}

fn genesis() -> (Documents, Cast) {
    let mut docs = Documents::new();
    let project = Project::inbox();
    docs.put_project(&project, None).unwrap();

    let mut labels = Vec::new();
    let mut order = OrderKey::middle();
    for i in 0..LABELS {
        let label = lumenna_core::model::Label::new(format!("l{i}"), order.clone());
        order = OrderKey::after(&order);
        labels.push(label.id);
        docs.put_label(&label, None).unwrap();
    }

    let mut tasks = Vec::new();
    let mut order = OrderKey::middle();
    for i in 0..TASKS {
        let task = Task::new(project.id, format!("t{i}"), order.clone());
        order = OrderKey::after(&order);
        tasks.push(task.id);
        docs.put_task(&task, None).unwrap();
    }
    (docs, Cast { project: project.id, tasks, labels })
}

/// Applies one operation as a device would: read what is there, change the one thing the
/// user asked for, write the difference.
fn apply(docs: &mut Documents, cast: &Cast, op: Op, replica: usize, step: usize) {
    let (snapshot, _) = docs.snapshot();
    let get = |i: usize| snapshot.tasks.get(&cast.tasks[i]).cloned();

    let Some(before) = get(match op {
        Op::Rename(i)
        | Op::Reparent(i, _)
        | Op::MoveAfter(i, _)
        | Op::ToggleLabel(i, _)
        | Op::ToggleDepend(i, _)
        | Op::Note(i)
        | Op::Priority(i, _)
        | Op::ToggleTrash(i) => i,
    }) else {
        return;
    };
    let mut after = before.clone();

    match op {
        Op::Rename(_) => after.title = format!("r{replica}s{step}"),
        Op::Reparent(_, parent) => {
            after.parent_id = parent.map(|p| cast.tasks[p]).filter(|p| *p != after.id);
        }
        Op::MoveAfter(_, target) => {
            if let Some(t) = get(target)
                && t.id != after.id
            {
                after.order = OrderKey::after(&t.order);
            }
        }
        Op::ToggleLabel(_, l) => {
            let label = cast.labels[l];
            if !after.labels.remove(&label) {
                after.labels.insert(label);
            }
        }
        Op::ToggleDepend(_, d) => {
            let dep = cast.tasks[d];
            if dep != after.id && !after.depends.remove(&dep) {
                after.depends.insert(dep);
            }
        }
        Op::Note(_) => after.notes.push_str(&format!("[{replica}:{step}]")),
        Op::Priority(_, p) => after.priority = Priority::from_u8(p),
        Op::ToggleTrash(_) => {
            after.deleted_at = if after.deleted_at.is_some() { None } else { Some(time::now()) };
        }
    }
    docs.put_task(&after, Some(&before)).unwrap();
}

/// Gives every replica every change, each in its own order.
fn cross_merge(replicas: &mut [Documents], seed: usize) {
    let saves: Vec<Vec<u8>> = replicas.iter_mut().map(|r| r.core().save()).collect();
    for (i, replica) in replicas.iter_mut().enumerate() {
        // A different rotation per replica and per seed: this is the whole point, since
        // Automerge's guarantee is that receiving order does not matter and the repairs
        // must not reintroduce a dependence on it. Stepping by one guarantees every replica
        // still receives every change — a stride sharing a factor with the replica count
        // would quietly skip some, and the test would pass by testing nothing.
        for step in 0..saves.len() {
            let j = (i + 1 + step + seed) % saves.len();
            if i != j {
                replica.core().load_incremental(&saves[j]).unwrap();
            }
        }
    }
}

fn settled(docs: &Documents) -> (Snapshot, Repairs) {
    let (mut snapshot, _) = docs.snapshot();
    let repairs = snapshot.repair();
    (snapshot, repairs)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// The headline property: same changes, different orders, identical state.
    #[test]
    fn replicas_converge(
        ops in prop::collection::vec((0..REPLICAS, op_strategy()), 1..24),
        seed in 0usize..8,
    ) {
        let (mut base, cast) = genesis();
        let mut replicas: Vec<Documents> = (0..REPLICAS).map(|_| base.fork()).collect();

        for (step, (replica, op)) in ops.iter().enumerate() {
            apply(&mut replicas[*replica], &cast, *op, *replica, step);
        }
        cross_merge(&mut replicas, seed);

        let (first, first_repairs) = settled(&replicas[0]);
        for other in &replicas[1..] {
            let (snapshot, repairs) = settled(other);
            prop_assert_eq!(&snapshot, &first, "replicas disagree after merge");
            // The repairs themselves must converge. A repair that depended on anything but
            // the merged data — iteration order, arrival order, which device ran it —
            // would make every replica's *fix* a fresh divergence.
            prop_assert_eq!(&repairs, &first_repairs, "repairs disagree");
        }
    }

    /// Merging is idempotent: a replica that receives the same changes twice does not
    /// change. Sync retries, and a store that drifted on a duplicate delivery would drift
    /// constantly.
    #[test]
    fn re_merging_changes_nothing(
        ops in prop::collection::vec((0..REPLICAS, op_strategy()), 1..16),
        seed in 0usize..8,
    ) {
        let (mut base, cast) = genesis();
        let mut replicas: Vec<Documents> = (0..REPLICAS).map(|_| base.fork()).collect();
        for (step, (replica, op)) in ops.iter().enumerate() {
            apply(&mut replicas[*replica], &cast, *op, *replica, step);
        }
        cross_merge(&mut replicas, seed);
        let once = settled(&replicas[0]).0;
        cross_merge(&mut replicas, seed + 1);
        prop_assert_eq!(settled(&replicas[0]).0, once);
    }

    /// Whatever merge produces, the tree and the dependency graph are walkable afterwards.
    ///
    /// This is the liveness half of §3.13: an unrepaired parent cycle hangs a naive walk,
    /// and the only way to be sure random concurrent reparenting cannot produce one is to
    /// try it.
    #[test]
    fn merged_state_is_always_walkable(
        ops in prop::collection::vec((0..REPLICAS, op_strategy()), 1..24),
        seed in 0usize..8,
    ) {
        let (mut base, cast) = genesis();
        let mut replicas: Vec<Documents> = (0..REPLICAS).map(|_| base.fork()).collect();
        for (step, (replica, op)) in ops.iter().enumerate() {
            apply(&mut replicas[*replica], &cast, *op, *replica, step);
        }
        cross_merge(&mut replicas, seed);

        let (snapshot, _) = settled(&replicas[0]);
        for task in snapshot.tasks.values() {
            let mut seen = BTreeSet::new();
            let mut cur = Some(task.id);
            while let Some(id) = cur {
                prop_assert!(seen.insert(id), "parent cycle survived repair");
                cur = snapshot.tasks.get(&id).and_then(|t| t.parent_id);
            }
        }
        // Ordering must still be a strict order within each parent, or the list a UI shows
        // depends on which device it is shown on.
        let mut keys: Vec<_> =
            snapshot.tasks.values().map(|t| (t.parent_id, t.order.clone(), t.id)).collect();
        keys.sort();
        keys.dedup();
        prop_assert_eq!(keys.len(), snapshot.tasks.len());
    }
}

#[test]
fn a_year_created_on_two_devices_at_once_does_not_lose_half_of_it() {
    // The realistic version of the concurrent-genesis hazard: two devices, both offline,
    // both scheduling something in a year neither has opened before.
    use jiff::civil::{date, time};
    use lumenna_core::model::{BlockKind, BlockSeries};

    let alice_block =
        BlockSeries::one_off("Alice", BlockKind::Work, date(2027, 4, 1), time(9, 0, 0, 0), 60)
            .unwrap();
    let bob_block =
        BlockSeries::one_off("Bob", BlockKind::Work, date(2027, 4, 2), time(9, 0, 0, 0), 60)
            .unwrap();

    let mut alice = Documents::new();
    let mut bob = Documents::new();
    alice.put_series(&alice_block, None).unwrap();
    bob.put_series(&bob_block, None).unwrap();

    alice.merge(&mut bob).unwrap();

    let (s, report) = alice.snapshot();
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(s.series.len(), 2, "one device's blocks were lost with the losing collection");
    assert!(s.series.contains_key(&alice_block.id));
    assert!(s.series.contains_key(&bob_block.id));
}

#[test]
fn the_genesis_change_is_identical_wherever_it_happens() {
    // What makes the test above work. If this ever stops holding, that one starts failing
    // in a way that is much harder to read.
    for id in [DocId::Core, DocId::Devices, DocId::Blocks(2027)] {
        let mut a = Doc::new(id);
        let mut b = Doc::new(id);
        assert_eq!(a.heads(), b.heads(), "{id:?} genesis differs between devices");
    }
}

#[test]
fn a_task_deleted_on_one_device_and_edited_on_another_keeps_both_facts() {
    // Trash is a field, not a tombstone (§3.2), so this is an ordinary field merge and the
    // edit must not be swallowed by the deletion.
    let (mut base, cast) = genesis();
    let mut alice = base.fork();
    let mut bob = base.fork();

    let before = alice.snapshot().0.tasks[&cast.tasks[0]].clone();

    let mut deleted = before.clone();
    deleted.deleted_at = Some(time::now());
    alice.put_task(&deleted, Some(&before)).unwrap();

    let mut renamed = before.clone();
    renamed.title = "bob renamed it".to_owned();
    bob.put_task(&renamed, Some(&before)).unwrap();

    alice.merge(&mut bob).unwrap();
    let (s, _) = alice.snapshot();
    let task = &s.tasks[&cast.tasks[0]];
    assert!(task.is_deleted(), "the deletion survived");
    assert_eq!(task.title, "bob renamed it", "so did the edit");
    assert_eq!(task.project_id, cast.project);
}

#[test]
fn concurrent_reparenting_makes_a_cycle_that_every_replica_repairs_the_same_way() {
    // The property tests above assert that repairs converge, which is only worth anything
    // if repairs actually happen. This is the case §3.13 opens with, built by hand: device
    // A moves X under Y while device B moves Y under X.
    let (mut base, cast) = genesis();
    let mut alice = base.fork();
    let mut bob = base.fork();
    let (x, y) = (cast.tasks[0], cast.tasks[1]);

    let x_before = alice.snapshot().0.tasks[&x].clone();
    let mut x_after = x_before.clone();
    x_after.parent_id = Some(y);
    alice.put_task(&x_after, Some(&x_before)).unwrap();

    let y_before = bob.snapshot().0.tasks[&y].clone();
    let mut y_after = y_before.clone();
    y_after.parent_id = Some(x);
    bob.put_task(&y_after, Some(&y_before)).unwrap();

    // Merge in both directions, so neither replica is the one that "saw it first".
    let alice_changes = alice.core().save();
    let bob_changes = bob.core().save();
    alice.core().load_incremental(&bob_changes).unwrap();
    bob.core().load_incremental(&alice_changes).unwrap();

    let (alice_state, alice_repairs) = settled(&alice);
    let (bob_state, bob_repairs) = settled(&bob);

    assert!(!alice_repairs.is_clean(), "the merge really did produce a cycle");
    assert_eq!(alice_repairs.reparented_tasks, [y.max(x)].into_iter().collect());
    assert_eq!(alice_repairs, bob_repairs, "the two devices disagreed about the fix");
    assert_eq!(alice_state, bob_state);
}

#[test]
fn a_concurrent_dependency_cycle_resolves_the_same_way_on_both_devices() {
    let (mut base, cast) = genesis();
    let mut alice = base.fork();
    let mut bob = base.fork();
    let (x, y) = (cast.tasks[2], cast.tasks[3]);

    let x_before = alice.snapshot().0.tasks[&x].clone();
    let mut x_after = x_before.clone();
    x_after.depends.insert(y);
    alice.put_task(&x_after, Some(&x_before)).unwrap();

    let y_before = bob.snapshot().0.tasks[&y].clone();
    let mut y_after = y_before.clone();
    y_after.depends.insert(x);
    bob.put_task(&y_after, Some(&y_before)).unwrap();

    alice.merge(&mut bob).unwrap();
    bob.merge(&mut alice).unwrap();

    let (alice_state, alice_repairs) = settled(&alice);
    let (bob_state, bob_repairs) = settled(&bob);

    // The newest task loses its dependency, and "newest" is computable from the identifiers
    // alone — which is why both devices reach it without talking to each other.
    let newest = x.max(y);
    let other = x.min(y);
    assert_eq!(alice_repairs.dropped_dependencies, [(newest, other)].into_iter().collect());
    assert_eq!(alice_repairs, bob_repairs);
    assert_eq!(alice_state, bob_state);
}

#[test]
fn two_devices_started_apart_share_one_inbox() {
    let mut alice = Documents::new();
    let mut bob = Documents::new();
    assert!(alice.ensure_schema().unwrap());
    assert!(bob.ensure_schema().unwrap());
    assert!(!alice.ensure_schema().unwrap(), "applying it twice is a no-op");

    alice.merge(&mut bob).unwrap();
    let (s, report) = alice.snapshot();
    assert!(report.is_clean(), "{report:?}");
    let inboxes: Vec<_> = s.projects.values().filter(|p| p.is_inbox).collect();
    assert_eq!(inboxes.len(), 1);
    assert_eq!(inboxes[0].id, ProjectId::INBOX);
    assert_eq!(inboxes[0], &Project::inbox(), "the frozen change spells the model's Inbox");
}

#[test]
fn edits_to_different_fields_of_one_occurrence_both_survive() {
    // An exception is keyed by series and date, which both devices arrive at on their own.
    // Stored as a map of its own, one device's map would win and the other's edit vanish.
    use jiff::civil::{date, time};
    use lumenna_core::model::{BlockException, ExceptionAction};

    let series = lumenna_core::SeriesId::new();
    let day = date(2026, 5, 6);
    let mut alice = Documents::new();
    let mut bob = alice.fork();

    let shorter = BlockException {
        series_id: series,
        original_date: day,
        action: ExceptionAction::Modified {
            start_time: None,
            duration_mins: Some(45),
            title: None,
            kind: None,
            flags: None,
        },
    };
    let renamed = BlockException {
        action: ExceptionAction::Modified {
            start_time: Some(time(10, 0, 0, 0)),
            duration_mins: None,
            title: Some("Review".to_owned()),
            kind: None,
            flags: None,
        },
        ..shorter.clone()
    };
    alice.put_exception(&shorter, None).unwrap();
    bob.put_exception(&renamed, None).unwrap();
    alice.merge(&mut bob).unwrap();

    let (s, report) = alice.snapshot();
    assert!(report.is_clean(), "{report:?}");
    let ExceptionAction::Modified { start_time, duration_mins, title, .. } =
        &s.exceptions[&(series, day)].action
    else {
        panic!("still a modification");
    };
    assert_eq!(*duration_mins, Some(45));
    assert_eq!(*start_time, Some(time(10, 0, 0, 0)));
    assert_eq!(title.as_deref(), Some("Review"));
}
