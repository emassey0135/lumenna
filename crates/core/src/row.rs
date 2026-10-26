//! The row projection: list semantics computed once, in the core.
//!
//! Every list-shaped view on every platform is the same handful of facts — where you are in
//! a list, how deep, what state the item is in — and every platform's accessibility API
//! wants exactly those facts under slightly different names:
//!
//! | Platform | Position | Size | Depth |
//! | --- | --- | --- | --- |
//! | GTK4 | `posinset` | `setsize` | `level` |
//! | Win32 | `SysTreeView32` reports all three from the control's own structure | | |
//! | AppKit | `NSAccessibilityOutline` / `Row` / `Cell` | | |
//! | Compose | `collectionItemInfo` | `collectionInfo` | |
//! | Web | `aria-posinset` | `aria-setsize` | `aria-level` |
//!
//! Computing them once here prevents eleven subtly different announcements of the same list,
//! and makes the semantics snapshot-testable in Rust rather than only observable through a
//! screen reader on each platform.
//!
//! # Components, never a composed sentence
//!
//! A [`Row`] carries its parts separately because speech and braille compose them
//! differently, and pre-flattening into one label string forces one channel to accept the
//! other's conventions:
//!
//! - **Speech** spells roles and states out: *"Review PR, task, overdue, 1 of 12"*.
//! - **Braille** uses the short forms a reader already knows — [`State::abbreviation`] —
//!   but renders [`Row::title`] **verbatim**.
//!
//! **Titles are never abbreviated.** Braille users get everything speech users get, and
//! abbreviating content is information loss. Long labels cost nothing when scanning, because
//! panning to the next braille *window* and moving to the next *line* are separate
//! operations — you move between items without reading through the current one.
//!
//! The words themselves are the app's (`lumenna_app::speech`, `lines`), which every platform
//! reads: the core says what is true, never how a line is joined.

use crate::filter::{Context, Expr};
use crate::id::{LabelId, ProjectId, SeriesId, TaskId};
use crate::snapshot::Snapshot;
use crate::state::State;

/// What a row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RowId {
    /// A task.
    Task(TaskId),
    /// A project.
    Project(ProjectId),
    /// A label.
    Label(LabelId),
    /// One occurrence of a block series, on a date.
    Occurrence(SeriesId, jiff::civil::Date),
}

/// The kind of thing a row is, structured rather than a string.
///
/// A string here would be a label in disguise, and every platform would end up translating
/// it back into its own role vocabulary — badly, and eleven times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// A task, which can be checked off.
    Task,
    /// A project in the tree.
    Project,
    /// A label.
    Label,
    /// A block occupying part of a day.
    Block,
    /// A heading that groups the rows beneath it.
    Group,
}

impl Role {
    /// What a screen reader says after the title.
    #[must_use]
    pub const fn speech(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Project => "project",
            Self::Label => "label",
            Self::Block => "block",
            Self::Group => "group",
        }
    }

    /// The braille short form.
    ///
    /// Like [`State::abbreviation`], these want checking against the convention BTBraille's
    /// tree view already uses before they ship.
    #[must_use]
    pub const fn abbreviation(self) -> &'static str {
        match self {
            Self::Task => "tsk",
            Self::Project => "prj",
            Self::Label => "lbl",
            Self::Block => "blk",
            Self::Group => "grp",
        }
    }
}

/// One line of a list-shaped view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What it stands for.
    pub id: RowId,
    /// What kind of thing it is.
    pub role: Role,
    /// How deep in the tree, with the top level at zero.
    pub depth: u32,
    /// Position within its sibling set, counting from one — which is what every
    /// accessibility API expects, and off-by-one here is off-by-one in eleven places.
    pub index: u32,
    /// How many siblings it has, including itself.
    pub count: u32,
    /// Whether it can be expanded, and whether it currently is. `None` for a leaf.
    ///
    /// Expansion state is per-device local state — syncing it would mean the watch
    /// collapsing a project on the desktop — so a caller supplies it rather than the
    /// snapshot carrying it.
    pub expanded: Option<bool>,
    /// Whether it is checked off. `None` for something that cannot be.
    pub checked: Option<bool>,
    /// The content, verbatim. Never abbreviated, never truncated.
    pub title: String,
    /// What is true about it, in announcement order.
    pub state: Vec<State>,
    /// When a task is due, in words and without its time: *"due tomorrow"*.
    pub due: Option<String>,
    /// The time of day it is due, if it has one: its own part, so each app says it in the
    /// clock its person chose.
    pub due_time: Option<jiff::civil::Time>,
    /// A secondary value: a task's priority, a block's time, a project's task count.
    pub value: Option<String>,
    /// A hint about what can be done here, if the platform has somewhere to put one.
    pub hint: Option<String>,
    /// A task's priority, 1 to 3, when it has one above none: the same as its value says in
    /// words, for a client that marks it by a face, a colour or a voice.
    pub priority: Option<u8>,
}

impl Snapshot {
    /// Projects the tasks a filter selects into rows, as a flattened tree.
    ///
    /// A task whose parent is not in the result appears at the top level rather than being
    /// hidden — filtering for `overdue` should show you the overdue subtask, not nothing at
    /// all because its parent is not overdue. Depth still reflects the real tree for any
    /// ancestor that *is* present, so an outline stays coherent.
    #[must_use]
    pub fn task_rows(&self, expr: &Expr, cx: &Context<'_>) -> Vec<Row> {
        self.task_rows_sorted(expr, cx, self.settings.task_order)
    }

    /// The same rows, siblings in `sort`'s order. A subtask stays under its parent whatever
    /// the order: urgency ranks within each level, never across it.
    #[must_use]
    pub fn task_rows_sorted(&self, expr: &Expr, cx: &Context<'_>, sort: crate::filter::Sort) -> Vec<Row> {
        let mut selected = expr.select(cx);
        let mut included: std::collections::BTreeSet<TaskId> =
            selected.iter().map(|task| task.id).collect();

        // Each task's live subtasks: how many there are and how many are done, said on its
        // row; and, where the setting asks, the done ones a list left out shown under it.
        let mut subtasks: std::collections::BTreeMap<TaskId, Vec<&crate::model::Task>> = std::collections::BTreeMap::new();
        for task in self.tasks.values().filter(|t| !t.is_deleted()) {
            if let Some(parent) = task.parent_id {
                subtasks.entry(parent).or_default().push(task);
            }
        }
        if self.settings.show_completed_subtasks {
            let mut frontier: Vec<TaskId> = included.iter().copied().collect();
            while let Some(parent) = frontier.pop() {
                for child in subtasks.get(&parent).into_iter().flatten() {
                    if cx.facts().is_completed(child) && included.insert(child.id) {
                        selected.push(child);
                        frontier.push(child.id);
                    }
                }
            }
        }
        let done: std::collections::BTreeMap<TaskId, (usize, usize)> = subtasks
            .iter()
            .map(|(parent, children)| {
                (*parent, (children.iter().filter(|c| cx.facts().is_completed(c)).count(), children.len()))
            })
            .collect();

        // Group by the nearest ancestor that survived the filter. A task whose parent was
        // filtered out but whose grandparent was not still belongs under the grandparent:
        // dropping it to the top level would lose a relationship the list can show.
        let mut children: std::collections::BTreeMap<Option<TaskId>, Vec<&crate::model::Task>> =
            std::collections::BTreeMap::new();
        for task in &selected {
            children.entry(self.surviving_ancestor(task, &included)).or_default().push(task);
        }
        for group in children.values_mut() {
            group.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            if sort == crate::filter::Sort::Urgency {
                let score = |task: &crate::model::Task| {
                    crate::urgency::urgency(self, cx.facts(), task, cx.now, &self.settings.urgency).score
                };
                // Scored once each, not once per comparison: urgency is most of a sort's cost.
                // Stable, so the person's order breaks ties.
                let mut scored: Vec<(f32, &crate::model::Task)> = group.iter().map(|task| (score(task), *task)).collect();
                scored.sort_by(|a, b| b.0.total_cmp(&a.0));
                *group = scored.into_iter().map(|(_, task)| task).collect();
            }
        }

        let mut rows = Vec::with_capacity(selected.len());
        push_task_rows(cx, &children, &done, None, 0, &mut rows);
        rows
    }

    /// The closest ancestor of `task` that is in `included`, if any.
    ///
    /// Walks with a visited set, so an unrepaired parent cycle ends the walk rather than
    /// hanging it.
    fn surviving_ancestor(
        &self,
        task: &crate::model::Task,
        included: &std::collections::BTreeSet<TaskId>,
    ) -> Option<TaskId> {
        let mut seen = std::collections::BTreeSet::from([task.id]);
        let mut current = task.parent_id;
        while let Some(id) = current {
            if included.contains(&id) {
                return Some(id);
            }
            if !seen.insert(id) {
                return None;
            }
            current = self.tasks.get(&id).and_then(|t| t.parent_id);
        }
        None
    }
}

/// A task's priority as its row says it; P4 is none, so says nothing.
const fn priority_words(priority: crate::model::Priority) -> Option<&'static str> {
    match priority {
        crate::model::Priority::P1 => Some("priority 1"),
        crate::model::Priority::P2 => Some("priority 2"),
        crate::model::Priority::P3 => Some("priority 3"),
        crate::model::Priority::P4 => None,
    }
}

fn push_task_rows(
    cx: &Context<'_>,
    children: &std::collections::BTreeMap<Option<TaskId>, Vec<&crate::model::Task>>,
    done: &std::collections::BTreeMap<TaskId, (usize, usize)>,
    parent: Option<TaskId>,
    depth: u32,
    rows: &mut Vec<Row>,
) {
    let Some(group) = children.get(&parent) else {
        return;
    };
    let count = u32::try_from(group.len()).unwrap_or(u32::MAX);
    for (index, task) in group.iter().enumerate() {
        let has_children = children.contains_key(&Some(task.id));
        rows.push(Row {
            id: RowId::Task(task.id),
            role: Role::Task,
            depth,
            index: u32::try_from(index).unwrap_or(u32::MAX) + 1,
            count,
            expanded: has_children.then_some(true),
            checked: Some(cx.facts().is_completed(task)),
            title: task.title.clone(),
            state: cx.facts().notable_states_of(task, cx.now),
            due: task.due.as_ref().map(|due| crate::time::due_words(due.date, cx.now.date())),
            due_time: task.due.as_ref().and_then(|due| due.time),
            value: {
                // "2 of 5 subtasks done": what a parent's row holds, whether or not they show.
                // A due time held to one place's zone says so: its time is that zone's.
                let zone = task.due.as_ref().and_then(|due| due.timezone.as_ref()).map(|zone| format!("{zone} time"));
                let parts: Vec<String> = zone
                    .into_iter()
                    .chain(priority_words(task.priority).map(str::to_owned))
                    .chain(done.get(&task.id).map(|(finished, all)| {
                        format!("{finished} of {all} {} done", if *all == 1 { "subtask" } else { "subtasks" })
                    }))
                    .collect();
                (!parts.is_empty()).then(|| parts.join(", "))
            },
            hint: None,
            priority: (task.priority != crate::model::Priority::P4).then(|| task.priority.as_u8()),
        });
        push_task_rows(cx, children, done, Some(task.id), depth + 1, rows);
    }
}
