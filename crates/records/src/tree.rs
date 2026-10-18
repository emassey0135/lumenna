//! Projects in the order a tree of them is read.

use std::collections::{BTreeMap, BTreeSet};

use lumenna_core::id::ProjectId;
use lumenna_core::model::Project;
use lumenna_core::snapshot::Snapshot;

/// How deep a project sits, stopping at a cycle merge may have made.
/// Live projects with their depths, each followed by its own subprojects, siblings in their
/// order. A list sorted by order alone puts a subproject wherever its key falls, which in a
/// list that shows depth reads as a child of whatever is above it. A project whose parent is
/// gone is at the top.
pub fn in_tree_order(snapshot: &Snapshot) -> Vec<(&Project, u32)> {
    let live: Vec<&Project> = snapshot.projects.values().filter(|p| p.deleted_at.is_none()).collect();
    let ids: BTreeSet<ProjectId> = live.iter().map(|p| p.id).collect();
    let mut children: BTreeMap<Option<ProjectId>, Vec<&Project>> = BTreeMap::new();
    for project in &live {
        let parent = project.parent_id.filter(|id| ids.contains(id) && *id != project.id);
        children.entry(parent).or_default().push(project);
    }
    for siblings in children.values_mut() {
        siblings.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
    }
    let mut ordered = Vec::with_capacity(live.len());
    let mut seen = BTreeSet::new();
    let mut stack: Vec<(&Project, u32)> =
        children.get(&None).into_iter().flatten().rev().map(|p| (*p, 0)).collect();
    while let Some((project, depth)) = stack.pop() {
        if !seen.insert(project.id) {
            continue;
        }
        ordered.push((project, depth));
        if let Some(under) = children.get(&Some(project.id)) {
            stack.extend(under.iter().rev().map(|p| (*p, depth + 1)));
        }
    }
    // A cycle that repair has not broken yet is unreachable from the top; list it anyway.
    for project in live {
        if seen.insert(project.id) {
            ordered.push((project, 0));
        }
    }
    ordered
}

/// The projects a task's Project field offers, in tree order: every one not archived, `id` its
/// name as `task_edit` takes it.
#[must_use]
pub fn project_options(snapshot: &Snapshot) -> Vec<crate::Choice> {
    in_tree_order(snapshot)
        .into_iter()
        .filter(|(project, _)| !project.archived)
        .map(|(project, depth)| crate::Choice { id: project.name.clone(), title: project.name.clone(), depth, ..crate::Choice::default() })
        .collect()
}
