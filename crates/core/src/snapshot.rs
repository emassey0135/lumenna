//! The materialized view every query runs over.
//!
//! §3.1 shards the data across several Automerge documents — `core`, one `blocks-<year>`
//! per calendar year, and `devices` — because Automerge loads and syncs whole documents and
//! a watch should not have to hold five years of block history to show today. A
//! [`Snapshot`] is the other side of that: whichever documents are currently loaded,
//! flattened into one thing to query. It is assembled by `store`, and nothing here knows
//! whether a given record arrived from an Automerge document or from the SQLite read model
//! that §8 leaves optional — which is exactly what lets that read model be added later
//! without changing an interface.
//!
//! Because the shards are independent, a snapshot is routinely **incomplete**: an assignment
//! in `blocks-2026` may name a task the local `core` document has not merged yet. That is
//! not corruption and must not be treated as such.

use std::collections::{BTreeMap, BTreeSet};

use jiff::civil;

use crate::id::{
    AssignmentId, CompletionId, FilterId, LabelId, NodeId, ProjectId, ReminderId, SeriesId,
    TaskId,
};
use crate::model::{
    BlockAssignment, BlockException, BlockSeries, Device, Label, Project, Reminder,
    ReminderAck, SavedFilter, Settings, Task, TaskCompletion,
};
use crate::repair::{break_dependency_cycles, break_tree_cycles};

/// Everything currently loaded, ready to query.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// From `core`.
    pub tasks: BTreeMap<TaskId, Task>,
    /// From `core`.
    pub completions: BTreeMap<CompletionId, TaskCompletion>,
    /// From `core`. Exactly one is the Inbox.
    pub projects: BTreeMap<ProjectId, Project>,
    /// From `core`.
    pub labels: BTreeMap<LabelId, Label>,
    /// From `core`.
    pub saved_filters: BTreeMap<FilterId, SavedFilter>,
    /// From the loaded `blocks-<year>` documents.
    pub series: BTreeMap<SeriesId, BlockSeries>,
    /// From the loaded `blocks-<year>` documents, keyed by the occurrence they modify.
    pub exceptions: BTreeMap<(SeriesId, civil::Date), BlockException>,
    /// From the loaded `blocks-<year>` documents.
    pub assignments: BTreeMap<AssignmentId, BlockAssignment>,
    /// From `core`.
    pub reminders: BTreeMap<ReminderId, Reminder>,
    /// From `core`, keyed by the firing they acknowledge.
    pub acks: BTreeMap<(ReminderId, civil::Date), ReminderAck>,
    /// From `devices`.
    pub devices: BTreeMap<NodeId, Device>,
    /// From `core`.
    pub settings: Settings,
}

impl Snapshot {
    /// Repairs both graphs in place, returning what had to be changed.
    ///
    /// Call this when materializing, before anything walks the tree or evaluates
    /// `ready`/`blocked` — an unrepaired parent cycle hangs a naive walk (§3.13).
    ///
    /// The result is not diagnostic noise to be discarded. A task that silently loses a
    /// dependency, or a project that silently jumps to the root, is worse than one the user
    /// was told about, so surface anything [`Repairs::is_clean`] denies.
    pub fn repair(&mut self) -> Repairs {
        let task_parents: BTreeMap<TaskId, Option<TaskId>> =
            self.tasks.iter().map(|(id, t)| (*id, t.parent_id)).collect();
        let reparented_tasks = break_tree_cycles(&task_parents);
        for id in &reparented_tasks {
            if let Some(task) = self.tasks.get_mut(id) {
                task.parent_id = None;
            }
        }

        let project_parents: BTreeMap<ProjectId, Option<ProjectId>> =
            self.projects.iter().map(|(id, p)| (*id, p.parent_id)).collect();
        let reparented_projects = break_tree_cycles(&project_parents);
        for id in &reparented_projects {
            if let Some(project) = self.projects.get_mut(id) {
                project.parent_id = None;
            }
        }

        let depends: BTreeMap<TaskId, BTreeSet<TaskId>> =
            self.tasks.iter().map(|(id, t)| (*id, t.depends.clone())).collect();
        let dropped_dependencies = break_dependency_cycles(&depends);
        for (from, to) in &dropped_dependencies {
            if let Some(task) = self.tasks.get_mut(from) {
                task.depends.remove(to);
            }
        }

        Repairs { reparented_tasks, reparented_projects, dropped_dependencies }
    }

    /// The Inbox, if the `core` document is loaded.
    #[must_use]
    pub fn inbox(&self) -> Option<&Project> {
        self.projects.values().find(|p| p.is_inbox && p.deleted_at.is_none())
    }

    /// The urgency multiplier in force for a project: its own weight, or the nearest
    /// ancestor's (§3.4).
    ///
    /// Making `#thesis` heavy should not require touching each of its chapters, so weight
    /// **inherits down the tree** and a sub-project overrides rather than compounds.
    /// Compounding would let a three-deep hierarchy at 2.0 reach a multiplier of eight,
    /// which is precisely the runaway the narrow UI range exists to prevent.
    ///
    /// Safe on an unrepaired snapshot: a parent cycle stops the walk rather than hanging it.
    #[must_use]
    pub fn effective_weight(&self, project_id: ProjectId) -> f32 {
        let mut seen = BTreeSet::new();
        let mut cur = Some(project_id);
        while let Some(id) = cur {
            if !seen.insert(id) {
                break;
            }
            let Some(project) = self.projects.get(&id) else {
                break; // Dangling parent (§3.1).
            };
            if let Some(weight) = project.declared_weight() {
                return weight;
            }
            cur = project.parent_id;
        }
        Project::NEUTRAL_WEIGHT
    }

    /// Every block occurrence in a date range, in the order the day is lived.
    ///
    /// Expanding on read is what §3.6's sparse exceptions buy: a daily routine is one
    /// series record and a handful of edits, not a row per day forever.
    ///
    /// # Errors
    ///
    /// If any loaded series carries a rule that cannot be expanded.
    pub fn occurrences(
        &self,
        range: &std::ops::RangeInclusive<civil::Date>,
    ) -> Result<Vec<crate::recur::Occurrence>, crate::recur::RecurError> {
        crate::recur::expand_all(&self.series, &self.exceptions, range)
    }

    /// Every block occurrence on one day.
    ///
    /// # Errors
    ///
    /// If any loaded series carries a rule that cannot be expanded.
    pub fn day(
        &self,
        date: civil::Date,
    ) -> Result<Vec<crate::recur::Occurrence>, crate::recur::RecurError> {
        self.occurrences(&(date..=date))
    }

    /// How many times a task has been completed.
    ///
    /// What [`advance`](crate::recur::advance) needs to interpret a rule's `COUNT`, since
    /// the model keeps only the current due date and the rule is re-anchored on each
    /// advance.
    #[must_use]
    pub fn completion_count(&self, task: TaskId) -> u32 {
        u32::try_from(self.completions.values().filter(|c| c.task_id == task).count())
            .unwrap_or(u32::MAX)
    }

    /// The label, if it still exists.
    ///
    /// Deleting a label touches no tasks, so identifiers left on tasks routinely point at
    /// nothing. §3.4 says those project as absent, and this is where that happens.
    #[must_use]
    pub fn label(&self, id: LabelId) -> Option<&Label> {
        self.labels.get(&id).filter(|l| l.deleted_at.is_none())
    }

    /// A task's labels, in list order, skipping deleted ones.
    #[must_use]
    pub fn labels_of(&self, task: &Task) -> Vec<&Label> {
        let mut labels: Vec<&Label> =
            task.labels.iter().filter_map(|id| self.label(*id)).collect();
        labels.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
        labels
    }
}

/// What [`Snapshot::repair`] had to change.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Repairs {
    /// Tasks cut loose from a parent cycle.
    pub reparented_tasks: BTreeSet<TaskId>,
    /// Projects cut loose from a parent cycle.
    pub reparented_projects: BTreeSet<ProjectId>,
    /// Dependency edges dropped, as `(task that depended, task it depended on)`.
    pub dropped_dependencies: BTreeSet<(TaskId, TaskId)>,
}

impl Repairs {
    /// Whether the snapshot was already well-formed.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.reparented_tasks.is_empty()
            && self.reparented_projects.is_empty()
            && self.dropped_dependencies.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Priority;
    use crate::order::OrderKey;

    fn snapshot_with_projects(n: usize) -> (Snapshot, Vec<ProjectId>) {
        let mut snap = Snapshot::default();
        let mut ids = Vec::new();
        for i in 0..n {
            let p = Project::new(format!("p{i}"), OrderKey::middle());
            ids.push(p.id);
            snap.projects.insert(p.id, p);
        }
        (snap, ids)
    }

    #[test]
    fn weight_inherits_from_the_nearest_ancestor() {
        let (mut snap, ids) = snapshot_with_projects(3);
        let (thesis, chapter, section) = (ids[0], ids[1], ids[2]);
        snap.projects.get_mut(&thesis).unwrap().weight = 2.0;
        snap.projects.get_mut(&chapter).unwrap().parent_id = Some(thesis);
        snap.projects.get_mut(&section).unwrap().parent_id = Some(chapter);

        assert!((snap.effective_weight(section) - 2.0).abs() < f32::EPSILON);

        // A sub-project overrides rather than compounding.
        snap.projects.get_mut(&chapter).unwrap().weight = 0.5;
        assert!((snap.effective_weight(section) - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn a_nonsensical_weight_is_ignored_rather_than_applied() {
        // Only a bad import or a future version can produce these, and a zero multiplier
        // would silently erase the urgency of everything beneath it.
        let (mut snap, ids) = snapshot_with_projects(1);
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            snap.projects.get_mut(&ids[0]).unwrap().weight = bad;
            assert!((snap.effective_weight(ids[0]) - 1.0).abs() < f32::EPSILON, "{bad}");
        }
    }

    #[test]
    fn weight_lookup_terminates_on_an_unrepaired_cycle() {
        let (mut snap, ids) = snapshot_with_projects(2);
        snap.projects.get_mut(&ids[0]).unwrap().parent_id = Some(ids[1]);
        snap.projects.get_mut(&ids[1]).unwrap().parent_id = Some(ids[0]);
        assert!((snap.effective_weight(ids[0]) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn repair_fixes_both_graphs_and_reports_what_it_did() {
        let (mut snap, projects) = snapshot_with_projects(2);
        snap.projects.get_mut(&projects[0]).unwrap().parent_id = Some(projects[1]);
        snap.projects.get_mut(&projects[1]).unwrap().parent_id = Some(projects[0]);

        let a = Task::new(projects[0], "a", OrderKey::middle());
        let b = Task::new(projects[0], "b", OrderKey::middle());
        let (a_id, b_id) = (a.id, b.id);
        snap.tasks.insert(a_id, a);
        snap.tasks.insert(b_id, b);
        snap.tasks.get_mut(&a_id).unwrap().parent_id = Some(b_id);
        snap.tasks.get_mut(&b_id).unwrap().parent_id = Some(a_id);
        snap.tasks.get_mut(&a_id).unwrap().depends.insert(b_id);
        snap.tasks.get_mut(&b_id).unwrap().depends.insert(a_id);

        let repairs = snap.repair();
        assert!(!repairs.is_clean());
        assert_eq!(repairs.reparented_tasks, [b_id].into_iter().collect());
        assert_eq!(repairs.reparented_projects, [projects[1]].into_iter().collect());
        assert_eq!(repairs.dropped_dependencies, [(b_id, a_id)].into_iter().collect());

        assert_eq!(snap.tasks[&b_id].parent_id, None);
        assert_eq!(snap.tasks[&a_id].parent_id, Some(b_id));
        assert!(snap.tasks[&b_id].depends.is_empty());
        assert!(snap.repair().is_clean(), "repair must be idempotent");
    }

    #[test]
    fn deleted_labels_project_as_absent() {
        let mut snap = Snapshot::default();
        let inbox = Project::inbox();
        let project_id = inbox.id;
        snap.projects.insert(inbox.id, inbox);

        let laptop = Label::new("laptop", OrderKey::middle());
        let gone = Label::new("gone", OrderKey::after(&laptop.order));
        let (laptop_id, gone_id) = (laptop.id, gone.id);
        snap.labels.insert(laptop_id, laptop);
        snap.labels.insert(gone_id, gone);
        snap.labels.get_mut(&gone_id).unwrap().deleted_at = Some(jiff::Timestamp::now());

        let mut task = Task::new(project_id, "write", OrderKey::middle());
        task.priority = Priority::P1;
        task.labels.insert(laptop_id);
        task.labels.insert(gone_id);
        // A label that was never in this document at all: also absent, not an error (§3.1).
        task.labels.insert(LabelId::new());

        let names: Vec<&str> = snap.labels_of(&task).iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, vec!["laptop"]);
    }
}
