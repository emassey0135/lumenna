//! The row projection (§13): list semantics computed once, in the core.
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

use jiff::Zoned;

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
    /// Expansion state is per-device local state (§3.12) — syncing it would mean the watch
    /// collapsing a project on the desktop — so a caller supplies it rather than the
    /// snapshot carrying it.
    pub expanded: Option<bool>,
    /// Whether it is checked off. `None` for something that cannot be.
    pub checked: Option<bool>,
    /// The content, verbatim. Never abbreviated, never truncated.
    pub title: String,
    /// What is true about it, in announcement order.
    pub state: Vec<State>,
    /// A secondary value: a due date, a block's time, a project's task count.
    pub value: Option<String>,
    /// A hint about what can be done here, if the platform has somewhere to put one.
    pub hint: Option<String>,
}

impl Row {
    /// The row as speech would render it.
    ///
    /// Assembled from the components rather than stored, so braille can assemble them
    /// differently from the same row.
    #[must_use]
    pub fn speech(&self) -> String {
        let mut parts = vec![self.title.clone(), self.role.speech().to_owned()];
        parts.extend(self.state.iter().map(|s| s.speech().to_owned()));
        if let Some(value) = &self.value {
            parts.push(value.clone());
        }
        if self.count > 1 {
            parts.push(format!("{} of {}", self.index, self.count));
        }
        parts.join(", ")
    }

    /// The row as braille would render it: short forms for role and state, title verbatim.
    #[must_use]
    pub fn braille(&self) -> String {
        let mut parts = vec![self.role.abbreviation().to_owned(), self.title.clone()];
        parts.extend(self.state.iter().map(|s| s.abbreviation().to_owned()));
        if let Some(value) = &self.value {
            parts.push(value.clone());
        }
        let mut line = parts.join(" ");
        if self.count > 1 {
            line.push_str(&format!(" {}/{}", self.index, self.count));
        }
        line
    }
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
        let selected = expr.select(cx);
        let included: std::collections::BTreeSet<TaskId> =
            selected.iter().map(|task| task.id).collect();

        // Group by the nearest ancestor that survived the filter.
        let mut children: std::collections::BTreeMap<Option<TaskId>, Vec<&crate::model::Task>> =
            std::collections::BTreeMap::new();
        for task in &selected {
            let parent = task.parent_id.filter(|id| included.contains(id));
            children.entry(parent).or_default().push(task);
        }
        for group in children.values_mut() {
            group.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
        }

        let mut rows = Vec::with_capacity(selected.len());
        push_task_rows(self, cx, &children, None, 0, &mut rows);
        rows
    }

    /// Projects the block occurrences of one day into rows, in the order the day is lived.
    ///
    /// # Errors
    ///
    /// If a series carries a rule that cannot be expanded.
    pub fn day_rows(
        &self,
        date: jiff::civil::Date,
        now: &Zoned,
    ) -> Result<Vec<Row>, crate::recur::RecurError> {
        let occurrences = self.day(date)?;
        let count = u32::try_from(occurrences.len()).unwrap_or(u32::MAX);
        Ok(occurrences
            .into_iter()
            .enumerate()
            .map(|(index, occurrence)| {
                let assigned = self
                    .assignments
                    .values()
                    .filter(|a| {
                        a.block_ref.series_id() == occurrence.series_id
                            && a.block_ref.date().is_none_or(|d| d == occurrence.date)
                    })
                    .count();
                let running = date == now.date()
                    && occurrence.start_time <= now.time()
                    && now.time() < occurrence.end_time();
                Row {
                    id: RowId::Occurrence(occurrence.series_id, occurrence.date),
                    role: Role::Block,
                    depth: 0,
                    index: u32::try_from(index).unwrap_or(u32::MAX) + 1,
                    count,
                    expanded: (assigned > 0).then_some(false),
                    checked: None,
                    title: occurrence.title.clone(),
                    // A block is not a task, so it wears no task states. Whether it is
                    // happening *now* is derived from the clock and never stored (§3.6).
                    state: Vec::new(),
                    value: Some(describe_block(&occurrence, assigned, running)),
                    hint: None,
                }
            })
            .collect())
    }
}

/// A time of day without its seconds.
fn clock(time: jiff::civil::Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

fn describe_block(
    occurrence: &crate::recur::Occurrence,
    assigned: usize,
    running: bool,
) -> String {
    let mut parts = vec![
        format!("{} to {}", clock(occurrence.start_time), clock(occurrence.end_time())),
        format!("{} minutes", occurrence.duration_mins),
    ];
    if running {
        parts.push("now".to_owned());
    }
    match assigned {
        0 => {}
        1 => parts.push("1 task assigned".to_owned()),
        n => parts.push(format!("{n} tasks assigned")),
    }
    if occurrence.modified {
        parts.push("changed for this day".to_owned());
    }
    parts.join(", ")
}

fn push_task_rows(
    snapshot: &Snapshot,
    cx: &Context<'_>,
    children: &std::collections::BTreeMap<Option<TaskId>, Vec<&crate::model::Task>>,
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
            checked: Some(snapshot.is_completed(task)),
            title: task.title.clone(),
            state: snapshot.notable_states_of(task, cx.now),
            value: task.due.as_ref().map(|due| match due.time {
                Some(time) => {
                    format!("due {} at {:02}:{:02}", due.date, time.hour(), time.minute())
                }
                None => format!("due {}", due.date),
            }),
            hint: None,
        });
        push_task_rows(snapshot, cx, children, Some(task.id), depth + 1, rows);
    }
}
