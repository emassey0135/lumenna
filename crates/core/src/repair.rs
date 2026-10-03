//! Repairing the two graphs that CRDT merge can corrupt (§3.13).
//!
//! Both hazards have the same shape and neither is exotic. A parent pointer is the right
//! representation for a tree, but nothing stops device A moving X under Y while device B
//! moves Y under X; after merge you have an orphaned cycle, and a naive tree walk hangs.
//! `depends` is a second graph over the same nodes with the same hazard, and there
//! `ready`/`blocked` evaluation never terminates. Automerge provides no move primitive —
//! Kleppmann's *"A highly-available move operation for replicated trees"* is the principled
//! treatment — so the repair happens at projection time instead.
//!
//! **The repairs are different, because reparenting to root is meaningless for a dependency
//! edge.** A tree cycle is fixed by cutting one node loose; a dependency cycle is fixed by
//! dropping the edge that closes it.
//!
//! # Determinism without coordination
//!
//! Every replica must reach the same answer, or the repair itself becomes a source of
//! divergence. Both functions here pick **the most recently created node in the cycle** and
//! cut its outgoing edge. Identifiers are UUIDv7 ([`crate::id`]), so the greatest identifier
//! is the newest node, which makes "undo the most recent change" computable from the data
//! alone — no timestamps to compare, no votes to take, and the same result on a watch as on
//! a desktop.
//!
//! # Severity differs
//!
//! An unrepaired tree cycle **hangs**. An unrepaired dependency cycle merely reads as
//! blocked forever, provided evaluation carries a visited set — so
//! [`break_dependency_cycles`] is a correctness-of-meaning fix rather than a liveness one.
//! Both are worth surfacing to the user either way: a task silently losing a dependency is
//! worse than being told it lost one.

use std::collections::{BTreeMap, BTreeSet};

/// Finds the nodes that must be reparented to the root to make a parent-pointer graph a
/// tree.
///
/// `parents` maps every node to its parent. A parent that is not itself a key is treated as
/// the root: §3.1 forbids referential integrity across documents, so a task whose parent
/// lives in a document this device has not merged yet is expected, not corrupt.
///
/// The same function serves tasks and projects, which have the same hazard for the same
/// reason.
#[must_use]
pub fn break_tree_cycles<I: Copy + Ord>(parents: &BTreeMap<I, Option<I>>) -> BTreeSet<I> {
    let mut breaks = BTreeSet::new();
    // Nodes already known to reach the root. Without this the walk is quadratic on a deep
    // tree, which on a project hierarchy is fine and on a task tree is not.
    let mut settled: BTreeSet<I> = BTreeSet::new();

    for &start in parents.keys() {
        if settled.contains(&start) {
            continue;
        }
        let mut path: Vec<I> = Vec::new();
        let mut on_path: BTreeSet<I> = BTreeSet::new();
        let mut cur = start;

        loop {
            if settled.contains(&cur) {
                break;
            }
            if on_path.contains(&cur) {
                // The cycle is the tail of the path from where it closes.
                let idx = path.iter().position(|n| *n == cur).unwrap_or(0);
                let victim = *path[idx..].iter().max().expect("a cycle has a member");
                breaks.insert(victim);
                break;
            }
            let Some(parent) = parents.get(&cur) else {
                break; // Dangling parent: root, as far as this device can tell.
            };
            on_path.insert(cur);
            path.push(cur);
            if breaks.contains(&cur) {
                break; // This edge was already cut by an earlier walk.
            }
            match parent {
                None => break,
                Some(p) => cur = *p,
            }
        }
        settled.extend(path);
        settled.insert(start);
    }
    breaks
}

/// Finds the dependency edges that must be dropped to make the graph acyclic.
///
/// `depends` maps a node to the nodes it depends on, so an entry `(a, b)` in the result
/// means *`a` no longer depends on `b`*. Edges pointing at nodes that are not keys are left
/// alone: a dependency on a task from an unmerged document is a dangling reference (§3.1),
/// not a cycle.
#[must_use]
pub fn break_dependency_cycles<I: Copy + Ord>(
    depends: &BTreeMap<I, BTreeSet<I>>,
) -> BTreeSet<(I, I)> {
    let mut dropped = BTreeSet::new();
    // Each pass drops exactly one edge, so the edge count bounds the loop. A malformed
    // graph must not be able to spin here.
    let edge_count: usize = depends.values().map(BTreeSet::len).sum();

    for _ in 0..=edge_count {
        let Some(cycle) = find_cycle(depends, &dropped) else {
            return minimize(depends, dropped);
        };
        // Drop the outgoing edge of the newest node on the cycle: the most recently created
        // task loses its dependency, which is the change least likely to be load-bearing.
        let victim_at = cycle
            .iter()
            .enumerate()
            .max_by_key(|(_, node)| **node)
            .map_or(0, |(i, _)| i);
        let victim = cycle[victim_at];
        let target = cycle[(victim_at + 1) % cycle.len()];
        dropped.insert((victim, target));
    }
    minimize(depends, dropped)
}

/// Puts back any edge the greedy pass did not actually need.
///
/// Cycles overlap. Breaking a long one and then a short one can leave the first cut
/// redundant, because the second already severed the only path that closed it — and an
/// edge dropped for nothing is data loss the user cannot see and did not cause.
///
/// Edges are offered restoration oldest-first, so where a choice remains the drops settle on
/// the more recently created dependency, which is the same preference the greedy pass uses.
fn minimize<I: Copy + Ord>(
    depends: &BTreeMap<I, BTreeSet<I>>,
    dropped: BTreeSet<(I, I)>,
) -> BTreeSet<(I, I)> {
    let mut kept = dropped.clone();
    for edge in &dropped {
        kept.remove(edge);
        if find_cycle(depends, &kept).is_some() {
            kept.insert(*edge);
        }
    }
    kept
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    White,
    Gray,
    Black,
}

/// One cycle, listed so that each node depends on the next and the last depends on the
/// first. `None` if the graph is acyclic once `dropped` is honoured.
///
/// Iterative rather than recursive: a pathological chain of subtasks should degrade to a
/// slow projection, not a blown stack on a watch.
fn find_cycle<I: Copy + Ord>(
    depends: &BTreeMap<I, BTreeSet<I>>,
    dropped: &BTreeSet<(I, I)>,
) -> Option<Vec<I>> {
    let mut color: BTreeMap<I, Color> = depends.keys().map(|&k| (k, Color::White)).collect();

    for &root in depends.keys() {
        if color[&root] != Color::White {
            continue;
        }
        let mut path: Vec<I> = vec![root];
        let mut frames: Vec<(I, std::vec::IntoIter<I>)> = vec![(root, neighbours(depends, root))];
        color.insert(root, Color::Gray);

        while let Some((node, iter)) = frames.last_mut() {
            let node = *node;
            if let Some(next) = iter.next() {
                if dropped.contains(&(node, next)) {
                    continue;
                }
                match color.get(&next) {
                    None | Some(Color::Black) => {}
                    Some(Color::Gray) => {
                        let idx = path.iter().position(|n| *n == next).unwrap_or(0);
                        return Some(path[idx..].to_vec());
                    }
                    Some(Color::White) => {
                        color.insert(next, Color::Gray);
                        path.push(next);
                        frames.push((next, neighbours(depends, next)));
                    }
                }
            } else {
                color.insert(node, Color::Black);
                path.pop();
                frames.pop();
            }
        }
    }
    None
}

fn neighbours<I: Copy + Ord>(
    depends: &BTreeMap<I, BTreeSet<I>>,
    node: I,
) -> std::vec::IntoIter<I> {
    depends
        .get(&node)
        .map(|set| set.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(pairs: &[(u32, Option<u32>)]) -> BTreeMap<u32, Option<u32>> {
        pairs.iter().copied().collect()
    }

    fn graph(pairs: &[(u32, &[u32])]) -> BTreeMap<u32, BTreeSet<u32>> {
        pairs.iter().map(|(n, deps)| (*n, deps.iter().copied().collect())).collect()
    }

    #[test]
    fn a_tree_needs_no_repair() {
        let t = tree(&[(1, None), (2, Some(1)), (3, Some(1)), (4, Some(2))]);
        assert!(break_tree_cycles(&t).is_empty());
    }

    #[test]
    fn a_dangling_parent_is_a_root_not_a_cycle() {
        // §3.1: an assignment may reference a task the local document has not seen yet.
        let t = tree(&[(2, Some(99))]);
        assert!(break_tree_cycles(&t).is_empty());
    }

    #[test]
    fn the_newest_node_in_a_cycle_is_cut_loose() {
        // A moved X under Y; B moved Y under X.
        let t = tree(&[(1, Some(2)), (2, Some(1))]);
        assert_eq!(break_tree_cycles(&t), [2].into_iter().collect());
    }

    #[test]
    fn a_node_parented_to_itself_is_cut_loose() {
        let t = tree(&[(7, Some(7))]);
        assert_eq!(break_tree_cycles(&t), [7].into_iter().collect());
    }

    #[test]
    fn a_subtree_hanging_off_a_cycle_survives_it() {
        let t = tree(&[(1, Some(3)), (2, Some(1)), (3, Some(2)), (4, Some(1))]);
        assert_eq!(break_tree_cycles(&t), [3].into_iter().collect());
        // 4 keeps its parent; only the cycle is touched.
    }

    #[test]
    fn independent_cycles_are_each_repaired() {
        let t = tree(&[(1, Some(2)), (2, Some(1)), (10, Some(11)), (11, Some(10))]);
        assert_eq!(break_tree_cycles(&t), [2, 11].into_iter().collect());
    }

    #[test]
    fn tree_repair_does_not_depend_on_iteration_order() {
        // Every replica must reach the same answer without coordinating.
        let edges = [(1, Some(2)), (2, Some(3)), (3, Some(1)), (4, Some(3))];
        let forward = break_tree_cycles(&tree(&edges));
        let mut reversed = edges;
        reversed.reverse();
        assert_eq!(forward, break_tree_cycles(&tree(&reversed)));
        assert_eq!(forward, [3].into_iter().collect());
    }

    #[test]
    fn an_acyclic_dependency_graph_needs_no_repair() {
        let g = graph(&[(1, &[]), (2, &[1]), (3, &[1, 2])]);
        assert!(break_dependency_cycles(&g).is_empty());
    }

    #[test]
    fn the_newest_task_loses_its_dependency() {
        // A made X depend on Y; B made Y depend on X.
        let g = graph(&[(1, &[2]), (2, &[1])]);
        assert_eq!(break_dependency_cycles(&g), [(2, 1)].into_iter().collect());
    }

    #[test]
    fn a_self_dependency_is_dropped() {
        let g = graph(&[(5, &[5])]);
        assert_eq!(break_dependency_cycles(&g), [(5, 5)].into_iter().collect());
    }

    #[test]
    fn only_the_closing_edge_is_dropped() {
        let g = graph(&[(1, &[2]), (2, &[3]), (3, &[1]), (4, &[1, 2, 3])]);
        let dropped = break_dependency_cycles(&g);
        assert_eq!(dropped, [(3, 1)].into_iter().collect());
        // Everything 4 depends on survives; it was never part of the cycle.
    }

    #[test]
    fn a_dependency_on_an_unseen_task_is_not_a_cycle() {
        let g = graph(&[(1, &[42])]);
        assert!(break_dependency_cycles(&g).is_empty());
    }

    #[test]
    fn overlapping_cycles_all_get_broken() {
        let g = graph(&[(1, &[2]), (2, &[1, 3]), (3, &[2])]);
        let dropped = break_dependency_cycles(&g);
        assert!(!dropped.is_empty());
        let repaired: BTreeMap<u32, BTreeSet<u32>> = g
            .iter()
            .map(|(n, deps)| {
                (*n, deps.iter().copied().filter(|d| !dropped.contains(&(*n, *d))).collect())
            })
            .collect();
        assert!(find_cycle(&repaired, &BTreeSet::new()).is_none());
    }

    #[test]
    fn a_redundant_cut_is_put_back() {
        // Two overlapping cycles: 0 -> 4 -> 0, and 0 -> 3 -> 5 -> 4 -> 0. Dropping (4, 0)
        // breaks both, so nothing else may be lost along the way.
        let g = graph(&[(0, &[3, 4]), (3, &[5]), (4, &[0]), (5, &[4])]);
        assert_eq!(break_dependency_cycles(&g), [(4, 0)].into_iter().collect());
    }

    #[test]
    fn dependency_repair_does_not_depend_on_iteration_order() {
        let a = graph(&[(1, &[2]), (2, &[3]), (3, &[1])]);
        let b = graph(&[(3, &[1]), (2, &[3]), (1, &[2])]);
        assert_eq!(break_dependency_cycles(&a), break_dependency_cycles(&b));
    }

    #[test]
    fn a_long_chain_does_not_blow_the_stack() {
        // Iterative traversal is not an aesthetic choice; watch targets have small stacks.
        let mut g: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for i in 0..100_000u32 {
            g.insert(i, [i + 1].into_iter().collect());
        }
        g.insert(100_000, BTreeSet::new());
        assert!(break_dependency_cycles(&g).is_empty());

        let mut t: BTreeMap<u32, Option<u32>> = BTreeMap::new();
        for i in 0..100_000u32 {
            t.insert(i, Some(i + 1));
        }
        t.insert(100_000, None);
        assert!(break_tree_cycles(&t).is_empty());
    }
}
