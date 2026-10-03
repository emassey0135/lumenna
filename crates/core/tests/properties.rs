//! Property tests for the two pieces of §3.13 that merge can break.
//!
//! §14 calls for property-based convergence tests over simulated replicas; those need a
//! store to apply operations to and belong with it. What can be proven here, without any
//! CRDT in the room, is that the pure machinery those tests will lean on holds up under
//! inputs nobody thought to write down: that ordering stays strict however a list is built,
//! and that repair terminates, converges, and only cuts what it has to.

use std::collections::{BTreeMap, BTreeSet};

use lumenna_core::order::OrderKey;
use lumenna_core::repair::{break_dependency_cycles, break_tree_cycles};
use proptest::prelude::*;

/// Rebuilds a list by inserting at a given position, the way a UI drag or a `lum move` does.
fn insert_at(list: &mut Vec<OrderKey>, index: usize) {
    let index = index.min(list.len());
    let before = index.checked_sub(1).and_then(|i| list.get(i));
    let after = list.get(index);
    let key = OrderKey::between(before, after).expect("neighbours of a sorted list ascend");
    list.insert(index, key);
}

fn is_strictly_sorted(list: &[OrderKey]) -> bool {
    list.windows(2).all(|w| w[0] < w[1])
}

/// Whether a parent map contains a cycle, walked with a visited set so it cannot hang on
/// the very input it is testing for.
fn tree_has_cycle(parents: &BTreeMap<u8, Option<u8>>) -> bool {
    parents.keys().any(|&start| {
        let mut seen = BTreeSet::new();
        let mut cur = Some(start);
        while let Some(node) = cur {
            if !seen.insert(node) {
                return true;
            }
            cur = parents.get(&node).copied().flatten();
        }
        false
    })
}

fn graph_has_cycle(depends: &BTreeMap<u8, BTreeSet<u8>>) -> bool {
    fn visit(
        node: u8,
        depends: &BTreeMap<u8, BTreeSet<u8>>,
        on_path: &mut BTreeSet<u8>,
        done: &mut BTreeSet<u8>,
    ) -> bool {
        if done.contains(&node) {
            return false;
        }
        if !on_path.insert(node) {
            return true;
        }
        let found = depends
            .get(&node)
            .is_some_and(|deps| deps.iter().any(|&d| visit(d, depends, on_path, done)));
        on_path.remove(&node);
        done.insert(node);
        found
    }

    let mut done = BTreeSet::new();
    depends.keys().any(|&n| visit(n, depends, &mut BTreeSet::new(), &mut done))
}

/// Small node counts on purpose: cycles are what is being tested, and they are far more
/// likely to appear in a graph of eight nodes than of eight hundred.
fn any_tree() -> impl Strategy<Value = BTreeMap<u8, Option<u8>>> {
    prop::collection::vec(prop::option::of(0u8..8), 1..8).prop_map(|parents| {
        parents.into_iter().enumerate().map(|(i, p)| (u8::try_from(i).unwrap(), p)).collect()
    })
}

fn any_graph() -> impl Strategy<Value = BTreeMap<u8, BTreeSet<u8>>> {
    prop::collection::vec(prop::collection::btree_set(0u8..8, 0..4), 1..8).prop_map(|deps| {
        deps.into_iter().enumerate().map(|(i, d)| (u8::try_from(i).unwrap(), d)).collect()
    })
}

fn apply_tree(
    parents: &BTreeMap<u8, Option<u8>>,
    breaks: &BTreeSet<u8>,
) -> BTreeMap<u8, Option<u8>> {
    parents
        .iter()
        .map(|(&n, &p)| (n, if breaks.contains(&n) { None } else { p }))
        .collect()
}

fn apply_graph(
    depends: &BTreeMap<u8, BTreeSet<u8>>,
    dropped: &BTreeSet<(u8, u8)>,
) -> BTreeMap<u8, BTreeSet<u8>> {
    depends
        .iter()
        .map(|(&n, deps)| {
            (n, deps.iter().copied().filter(|&d| !dropped.contains(&(n, d))).collect())
        })
        .collect()
}

proptest! {
    /// However a list is assembled, its keys stay in strict order.
    ///
    /// Repeated insertion at the same index is the pathological case, and it is also the
    /// common one: "add to the top of the list" every morning.
    #[test]
    fn ordering_survives_arbitrary_insertion(indices in prop::collection::vec(0usize..40, 1..80)) {
        let mut list = Vec::new();
        for index in indices {
            insert_at(&mut list, index);
            prop_assert!(is_strictly_sorted(&list));
        }
        let distinct: BTreeSet<_> = list.iter().collect();
        prop_assert_eq!(distinct.len(), list.len(), "keys must not collide");
    }

    /// Every key is round-trippable through its stored form.
    #[test]
    fn order_keys_parse_back(indices in prop::collection::vec(0usize..20, 1..40)) {
        let mut list = Vec::new();
        for index in indices {
            insert_at(&mut list, index);
        }
        for key in &list {
            prop_assert_eq!(&key.as_str().parse::<OrderKey>().unwrap(), key);
        }
    }

    /// Repair leaves a tree, is idempotent, and touches nothing that was already fine.
    #[test]
    fn tree_repair_is_sound(parents in any_tree()) {
        let breaks = break_tree_cycles(&parents);
        let repaired = apply_tree(&parents, &breaks);

        prop_assert!(!tree_has_cycle(&repaired));
        prop_assert!(break_tree_cycles(&repaired).is_empty(), "repair must be idempotent");
        if !tree_has_cycle(&parents) {
            prop_assert!(breaks.is_empty(), "an acyclic tree must be left alone");
        }
    }

    /// Repair cuts exactly one node per cycle, never a bystander: restoring any single cut
    /// brings a cycle back.
    #[test]
    fn tree_repair_cuts_no_spare_nodes(parents in any_tree()) {
        let breaks = break_tree_cycles(&parents);
        let repaired = apply_tree(&parents, &breaks);
        for node in &breaks {
            let mut restored = repaired.clone();
            restored.insert(*node, parents[node]);
            prop_assert!(tree_has_cycle(&restored), "{node} was cut loose for nothing");
        }
    }

    /// Repair leaves an acyclic graph, is idempotent, and drops nothing it did not have to.
    ///
    /// The last clause is the one that matters to a user: a dependency that vanishes without
    /// a cycle to justify it is data loss, however tidy the resulting graph looks.
    #[test]
    fn dependency_repair_is_sound(depends in any_graph()) {
        let dropped = break_dependency_cycles(&depends);
        let repaired = apply_graph(&depends, &dropped);

        prop_assert!(!graph_has_cycle(&repaired));
        prop_assert!(break_dependency_cycles(&repaired).is_empty(), "must be idempotent");
        if !graph_has_cycle(&depends) {
            prop_assert!(dropped.is_empty(), "an acyclic graph must be left alone");
        }
        for (from, to) in &dropped {
            prop_assert!(depends[from].contains(to), "only real edges may be dropped");
        }
    }

    /// Repair is minimal enough to be honest: restoring any single dropped edge brings the
    /// cycle back. It never drops one more than it needed to.
    #[test]
    fn dependency_repair_drops_no_spare_edges(depends in any_graph()) {
        let dropped = break_dependency_cycles(&depends);
        let repaired = apply_graph(&depends, &dropped);
        for &(from, to) in &dropped {
            let mut restored = repaired.clone();
            restored.entry(from).or_default().insert(to);
            prop_assert!(graph_has_cycle(&restored), "({from}, {to}) was dropped for nothing");
        }
    }
}
