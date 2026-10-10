//! What operations return, shaped for clients rather than derived from the model.
//!
//! **Every result carries its own `announcement`** — one sentence saying what happened, for a
//! client with nothing better to say — and its `notices`, things worth saying that are not the
//! answer. The sentence is composed here only because core composed it first: an edit's
//! description, a quick-add readback. Everything else stays in components.
//!
//! **Rows carry components, never a sentence**. Speech spells roles and states out;
//! braille abbreviates the role and renders the title verbatim; a client assembles its own
//! line from `role`, `state`, `title` and `value`.
//!
//! These are a compatibility contract twice over: the JSON `lum --json` and `lum rpc` write,
//! and the Swift and Kotlin types UniFFI generates. Renaming a field breaks both.

use lumenna_core::State;
use lumenna_core::edit::{Change as CoreChange, Edit};
use lumenna_core::filter::NameKind;
use lumenna_core::model::{Priority, Task};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::words::{count_line, time_text};

/// What every result carries: one sentence, and anything else worth saying.
pub trait Announced: Sized {
    /// What happened, in a sentence.
    fn announcement(&self) -> &str;

    /// Things worth saying that are not the answer.
    fn notices(&self) -> &[String];

    /// The same, to add to.
    fn notices_mut(&mut self) -> &mut Vec<String>;

    /// Adds something worth saying that is not the answer.
    #[must_use]
    fn note(mut self, notice: impl Into<String>) -> Self {
        self.notices_mut().push(notice.into());
        self
    }
}

/// Implements [`Announced`] for records with `announcement` and `notices` fields.
#[macro_export]
macro_rules! announced {
    ($($type:ty),* $(,)?) => {
        $(
            impl $crate::Announced for $type {
                fn announcement(&self) -> &str {
                    &self.announcement
                }
                fn notices(&self) -> &[String] {
                    &self.notices
                }
                fn notices_mut(&mut self) -> &mut Vec<String> {
                    &mut self.notices
                }
            }
        )*
    };
}

announced!(
    Change, Rows, TaskShown, Plan, Filters, SettingList, Timer, Completions, Preview,
    BackupDone, BackupFile, RestoreDone, Exported, ImportDone, PairedWith, SyncReport, SyncStatus,
    DeviceList, BlockShown, WorkBlocks, LinkSynced,
);

fn none<T>(list: &[T]) -> bool {
    list.is_empty()
}

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------------------

/// What a mutation did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Change {
    /// What happened, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// Whether anything actually changed. Asking for a state something is already in is not
    /// an error, but a client that cannot tell the two apart announces a change that did not
    /// happen.
    pub changed: bool,
    /// The records it touched, so a client can act on what it just made without listing
    /// everything again to find it.
    #[serde(default, skip_serializing_if = "Affected::is_empty")]
    pub affected: Affected,
    /// The task itself, when the operation created one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskDetail>,
}

impl Change {
    /// A mutation that changed something, announced as core described it.
    #[must_use]
    pub fn of(edit: &Edit) -> Self {
        Self::announced(edit.description.clone(), edit)
    }

    /// A mutation announced in other words, where core's description is not the whole story.
    #[must_use]
    pub fn announced(announcement: impl Into<String>, edit: &Edit) -> Self {
        Self {
            announcement: announcement.into(),
            notices: Vec::new(),
            changed: !edit.is_empty(),
            affected: Affected::of(edit),
            task: None,
        }
    }

    /// A mutation that turned out to have nothing to do.
    #[must_use]
    pub fn unchanged(announcement: impl Into<String>) -> Self {
        Self {
            announcement: announcement.into(),
            notices: Vec::new(),
            changed: false,
            affected: Affected::default(),
            task: None,
        }
    }

    /// A change made outside the document — a device setting, which never syncs.
    #[must_use]
    pub fn local(announcement: impl Into<String>) -> Self {
        Self { changed: true, ..Self::unchanged(announcement) }
    }
}

/// Identifiers a change touched, by kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Affected {
    /// Tasks.
    #[serde(default, skip_serializing_if = "none")]
    pub tasks: Vec<String>,
    /// Projects.
    #[serde(default, skip_serializing_if = "none")]
    pub projects: Vec<String>,
    /// Labels.
    #[serde(default, skip_serializing_if = "none")]
    pub labels: Vec<String>,
    /// Saved filters.
    #[serde(default, skip_serializing_if = "none")]
    pub filters: Vec<String>,
    /// Block series.
    #[serde(default, skip_serializing_if = "none")]
    pub blocks: Vec<String>,
    /// Assignments.
    #[serde(default, skip_serializing_if = "none")]
    pub assignments: Vec<String>,
    /// Devices paired, renamed or unpaired.
    #[serde(default, skip_serializing_if = "none")]
    pub devices: Vec<String>,
    /// Whether the settings singleton moved.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub settings: bool,
}

impl Affected {
    /// Reads the identifiers straight off the edit, so every operation reports what it
    /// touched without each one remembering to.
    ///
    /// A completion and an exception are not addressable records, but they *name* one — so
    /// they report the task or series they are about. Reminders and their acknowledgements
    /// are absent entirely: nothing addresses them yet.
    #[must_use]
    pub fn of(edit: &Edit) -> Self {
        let mut affected = Self::default();
        for change in &edit.changes {
            match change {
                CoreChange::Task(t) => {
                    push(&mut affected.tasks, t.before.as_ref().map(|x| x.id), t.after.as_ref().map(|x| x.id));
                }
                CoreChange::Project(t) => {
                    push(&mut affected.projects, t.before.as_ref().map(|x| x.id), t.after.as_ref().map(|x| x.id));
                }
                CoreChange::Label(t) => {
                    push(&mut affected.labels, t.before.as_ref().map(|x| x.id), t.after.as_ref().map(|x| x.id));
                }
                CoreChange::Filter(t) => {
                    push(&mut affected.filters, t.before.as_ref().map(|x| x.id), t.after.as_ref().map(|x| x.id));
                }
                CoreChange::Series(t) => {
                    push(&mut affected.blocks, t.before.as_ref().map(|x| x.id), t.after.as_ref().map(|x| x.id));
                }
                CoreChange::Assignment { transition: t, .. } => {
                    push(&mut affected.assignments, t.before.as_ref().map(|x| x.id), t.after.as_ref().map(|x| x.id));
                }
                CoreChange::Completion(t) => {
                    let named = t.after.as_ref().or(t.before.as_ref());
                    push(&mut affected.tasks, None, named.map(|c| c.task_id));
                }
                CoreChange::Exception(t) => {
                    let named = t.after.as_ref().or(t.before.as_ref());
                    push(&mut affected.blocks, None, named.map(|e| e.series_id));
                }
                CoreChange::Device(t) => {
                    let named = t.after.as_ref().or(t.before.as_ref());
                    push(&mut affected.devices, None, named.map(|d| d.node_id));
                }
                CoreChange::Settings { .. } => affected.settings = true,
                CoreChange::Reminder(_) | CoreChange::Ack(_) => {}
            }
        }
        affected
    }

    /// Whether it names nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
            && self.projects.is_empty()
            && self.labels.is_empty()
            && self.filters.is_empty()
            && self.blocks.is_empty()
            && self.assignments.is_empty()
            && self.devices.is_empty()
            && !self.settings
    }
}

fn push<T: ToString>(into: &mut Vec<String>, before: Option<T>, after: Option<T>) {
    if let Some(id) = after.or(before) {
        let text = id.to_string();
        if !into.contains(&text) {
            into.push(text);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Listings
// ---------------------------------------------------------------------------------------

/// A listing of rows, all of one kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Rows {
    /// The count line — *"17 tasks"*.
    pub announcement: String,
    /// Anything else worth saying, such as a name the query did not recognise.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The singular noun for the count.
    pub noun: String,
    /// How many rows there are, so a reader knows before the rows arrive.
    pub count: u32,
    /// How the query was understood, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<Query>,
    /// The rows.
    pub rows: Vec<RowView>,
    /// What a client says in place of the rows when there are none: "The trash is empty."
    #[serde(default)]
    pub empty: String,
}

impl Rows {
    /// Projects core's rows, numbering them from one and announcing the count.
    #[must_use]
    pub fn new(rows: &[Row], noun: &str) -> Self {
        Self {
            announcement: count_line(rows.len(), noun),
            notices: Vec::new(),
            noun: noun.to_owned(),
            count: to_u32(rows.len()),
            query: None,
            empty: format!("No {noun}s."),
            rows: rows
                .iter()
                .enumerate()
                .map(|(position, row)| RowView {
                    row: to_u32(position + 1),
                    id: row_id(&row.id),
                    role: role_name(row.role).to_owned(),
                    depth: row.depth,
                    index: row.index,
                    count: row.count,
                    checked: row.checked,
                    expanded: row.expanded,
                    title: row.title.clone(),
                    state: row.state.iter().map(|state| state.keyword().to_owned()).collect(),
                    due: row.due.clone(),
                    due_time: row.due_time.map(time_text),
                    value: row.value.clone(),
                    hint: row.hint.clone(),
                    actions: Vec::new(),
                })
                .collect(),
        }
    }
}

/// A filter query, read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Query {
    /// The query as it was written.
    pub text: String,
    /// The query as it was understood. A mis-parsed filter shows wrong results silently, and
    /// wrong results are invisible — so every client should have this to hand.
    pub description: String,
    /// Names in the query that match nothing.
    #[serde(default, skip_serializing_if = "none")]
    pub unresolved: Vec<Unresolved>,
}

/// A name in a query that matches nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Unresolved {
    /// `project` or `label`.
    pub kind: String,
    /// The name as written.
    pub name: String,
    /// The nearest real name, if one is near enough to suggest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// One row, as its components.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct RowView {
    /// Its position in this listing, counting from one — what `lum task done 3` takes.
    pub row: u32,
    /// The full identifier.
    pub id: String,
    /// What kind of thing it is: `task`, `project`, `label`, `block`.
    pub role: String,
    /// Depth in the tree, zero at the top.
    pub depth: u32,
    /// Position within its sibling set, counting from one.
    pub index: u32,
    /// How many siblings, including itself.
    pub count: u32,
    /// Whether it is checked off, where that means anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// Whether there is something under it, and whether the projection left it expanded.
    /// Absent for a leaf; a client tracks its own collapse state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// The content, verbatim. Never abbreviated, in any medium.
    pub title: String,
    /// Computed states, as their filter keywords — a client can feed one straight back into
    /// a filter.
    pub state: Vec<String>,
    /// When a task is due, in words and without its time: *"due tomorrow"*.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// The time it is due, `HH:MM`, for the client to say in its own clock after `due`:
    /// *"due tomorrow at 3:00 PM"*.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    /// What else the row says, after when it is due: a task's priority, a project's count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// What can be done here, for a client with somewhere to put a hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

impl RowView {
    /// The states worth putting on a printed line. `ready` is true of almost every task, so
    /// printing it everywhere buries the states that mean something.
    pub fn trailing_states(&self) -> impl Iterator<Item = &str> + '_ {
        self.state.iter().map(String::as_str).filter(|k| *k != State::Ready.keyword())
    }
}

/// The identifier a row refers to, as text.
#[must_use]
pub fn row_id(id: &RowId) -> String {
    match id {
        RowId::Task(id) => id.to_string(),
        RowId::Project(id) => id.to_string(),
        RowId::Label(id) => id.to_string(),
        RowId::Occurrence(id, date) => format!("{id}@{date}"),
    }
}

/// The kind name a row is addressed under.
#[must_use]
pub const fn role_name(role: Role) -> &'static str {
    match role {
        Role::Task => "task",
        Role::Project => "project",
        Role::Label => "label",
        Role::Block => "block",
        Role::Group => "group",
    }
}

/// The noun a name kind is called by.
#[must_use]
pub const fn name_kind(kind: NameKind) -> &'static str {
    match kind {
        NameKind::Project => "project",
        NameKind::Label => "label",
    }
}

// ---------------------------------------------------------------------------------------
// One task
// ---------------------------------------------------------------------------------------

/// One task, as [`show_task`](crate::Lumenna::show_task) returns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct TaskShown {
    /// Its title, which is all there is to announce.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The task.
    #[serde(flatten)]
    pub task: TaskDetail,
}

/// Everything about one task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct TaskDetail {
    /// The full identifier.
    pub id: String,
    /// The content, verbatim.
    pub title: String,
    /// Free text.
    pub notes: String,
    /// The project it sits in, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The task it sits under, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// Label names, without the leading `@`.
    pub labels: Vec<String>,
    /// What it waits for.
    pub depends: Vec<Dependency>,
    /// The due date, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// The time of day it is due, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    /// The repetition, as an RFC 5545 rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// The repetition in the words quick add takes — `every monday`, `every! day` — which
    /// [`TaskEdit::repeat`] reads back unchanged. Absent when it repeats by a rule the
    /// grammar cannot say, which `recurrence` then holds alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition: Option<String>,
    /// How long it should take.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_mins: Option<u32>,
    /// Computed states, the full set — a detail view is where the near-universal ones are
    /// worth having.
    pub state: Vec<String>,
    /// When it was created.
    pub created_at: String,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

/// A task another task waits for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Dependency {
    /// The full identifier.
    pub id: String,
    /// Its title, or a note that this device has not seen it yet.
    pub title: String,
}

impl TaskDetail {
    /// Reads one task out of a snapshot, resolving the names it points at.
    #[must_use]
    pub fn of(task: &Task, snapshot: &Snapshot, now: &jiff::Zoned) -> Self {
        Self {
            id: task.id.to_string(),
            title: task.title.clone(),
            notes: task.notes.clone(),
            project: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
            parent: task.parent_id.map(|id| id.to_string()),
            priority: task.priority.as_u8(),
            labels: snapshot.labels_of(task).iter().map(|l| l.name.clone()).collect(),
            depends: task
                .depends
                .iter()
                .map(|id| Dependency {
                    id: id.to_string(),
                    title: snapshot
                        .tasks
                        .get(id)
                        .map_or_else(|| "(not loaded)".to_owned(), |t| t.title.clone()),
                })
                .collect(),
            due: task.due.as_ref().map(|due| due.date.to_string()),
            due_time: task.due.as_ref().and_then(|due| due.time).map(time_text),
            recurrence: task
                .due
                .as_ref()
                .and_then(|due| due.recurrence.as_ref())
                .map(|r| r.rrule.clone()),
            repetition: task
                .due
                .as_ref()
                .and_then(|due| due.recurrence.as_ref())
                .and_then(|r| repetition_phrase(&r.rrule, r.from_completion)),
            estimate_mins: task.estimate_mins,
            state: snapshot.states_of(task, now).iter().map(|s| s.keyword().to_owned()).collect(),
            created_at: task.created_at.to_string(),
            actions: crate::actions::task(snapshot, task, snapshot.facts().is_completed(task), true),
        }
    }

    /// Whether the priority is worth mentioning. P4 is the default and says nothing.
    #[must_use]
    pub fn has_priority(&self) -> bool {
        self.priority != Priority::P4.as_u8()
    }
}

// ---------------------------------------------------------------------------------------
// The day
// ---------------------------------------------------------------------------------------

/// A day's blocks and what is assigned to them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Plan {
    /// The day and how many blocks it holds.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The day, as an ISO date.
    pub date: String,
    /// How many blocks it holds.
    pub count: u32,
    /// The blocks, in time order.
    pub blocks: Vec<PlanBlock>,
    /// The gestalt a sighted user gets from a glance at the day, as one sentence:
    /// *"Six blocks, four hours of work, three tasks assigned, one overdue."*
    #[serde(default)]
    pub summary: String,
    /// The day as it is lived, in order: blocks, the free time between them, and where now
    /// falls: what a timeline shows by empty space, a list has to say as a row.
    #[serde(default)]
    pub timeline: Vec<PlanItem>,
    /// Repeating blocks cancelled for this day alone, so that the day can be put back
    /// without remembering what used to be there.
    #[serde(default, skip_serializing_if = "none")]
    pub cancelled: Vec<CancelledBlock>,
}

/// The work blocks a task could be put in, from a task itself rather than from a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct WorkBlocks {
    /// How many there are, and over how many days.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The first day, as an ISO date.
    pub from: String,
    /// How many days they come from.
    pub days: u32,
    /// The work blocks, by day and then by time.
    pub blocks: Vec<WorkBlock>,
}

/// One work block occurrence, as a chooser lists it. Components, not a line: how a day and a
/// time are said is the platform's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct WorkBlock {
    /// `<series>@<date>`, as [`assign`](crate::Lumenna::assign) takes it.
    pub id: String,
    /// Its day, as an ISO date.
    pub date: String,
    /// Its name.
    pub title: String,
    /// When it starts, `HH:MM`.
    pub start: String,
    /// When it ends, `HH:MM`.
    pub end: String,
    /// How long it runs.
    pub duration_mins: u32,
    /// `past`, `now` or `upcoming` on today; empty on any other day.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub when: String,
    /// How many tasks are in it already.
    pub assigned: u32,
}

/// A repeating block that does not happen on one day because that day was cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct CancelledBlock {
    /// The series' identifier, for [`Lumenna::restore_occurrence`](crate::Lumenna::restore_occurrence).
    pub series: String,
    /// Its name.
    pub title: String,
    /// When the series has it start, `HH:MM`.
    pub start: String,
    /// What its line says after its start and title: "cancelled for this day".
    #[serde(default)]
    pub details: Vec<String>,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

/// One row of a day's timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "item", rename_all = "snake_case")]
pub enum PlanItem {
    /// A block, by its position in [`Plan::blocks`] counting from one.
    Block {
        /// Its `row`.
        row: u32,
    },
    /// Time with nothing in it, long enough to be worth a row: *"Free, 45 minutes, 10:15
    /// to 11:00."*
    Free {
        /// When it starts, `HH:MM`.
        start: String,
        /// When it ends, `HH:MM`.
        end: String,
        /// How long it is.
        minutes: u32,
        /// What its line says first: "Free".
        #[serde(default)]
        title: String,
        /// What its line says after the title and before its span: "45 minutes".
        #[serde(default)]
        details: Vec<String>,
        /// What can be done with it.
        #[serde(default, skip_serializing_if = "none")]
        actions: Vec<crate::actions::Action>,
    },
    /// Where the present falls, between the rows either side of it. Only on today.
    Now {
        /// The time, `HH:MM`.
        time: String,
        /// What its line says before the time: "Now".
        #[serde(default)]
        title: String,
    },
}

/// One block on a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PlanBlock {
    /// Its position among blocks, counting from one.
    pub row: u32,
    /// `<series>@<date>`, which addresses this occurrence.
    pub id: String,
    /// The series it belongs to.
    pub series: String,
    /// Its name.
    pub title: String,
    /// When it starts, `HH:MM`.
    pub start: String,
    /// When it ends, `HH:MM`.
    pub end: String,
    /// How long it runs.
    pub duration_mins: u32,
    /// `work`, `break` or `event`. The word says which actions exist: only work
    /// blocks take tasks.
    #[serde(default)]
    pub kind: String,
    /// `past`, `now` or `upcoming` on today; empty on any other day.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub when: String,
    /// Whether it repeats — whether changing it asks "this one, or every one?"
    #[serde(default)]
    pub repeats: bool,
    /// Whether this occurrence was changed apart from its series.
    #[serde(default)]
    pub changed_for_this_day: bool,
    /// What is assigned to it, in order.
    pub assignments: Vec<PlanAssignment>,
    /// Whether tasks can be put in it — not its kind: a break may take tasks, and an
    /// anchored block may too.
    #[serde(default)]
    pub accepts_tasks: bool,
    /// Whether it counts toward the hours available for work.
    #[serde(default)]
    pub counts_capacity: bool,
    /// Whether it is fixed in time.
    #[serde(default)]
    pub anchored: bool,
    /// Its colour, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    /// Its notes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// What a line about it says after its time and title, as parts every app words alike:
    /// "1 hour 30 minutes", "work block", "now", "2 tasks assigned".
    #[serde(default)]
    pub details: Vec<String>,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

/// One block series, as [`show_block`](crate::Lumenna::show_block) returns it — what an
/// editor starts from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BlockShown {
    /// Its name.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The series' identifier.
    pub id: String,
    /// Its name.
    pub title: String,
    /// When it starts, `HH:MM`.
    pub start: String,
    /// How long it lasts.
    pub minutes: u32,
    /// `work`, `break` or `event`.
    pub kind: String,
    /// The day it starts, or its only day.
    pub start_date: String,
    /// Whether it repeats.
    pub repeats: bool,
    /// How it repeats, as an RFC 5545 rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rrule: Option<String>,
    /// How it repeats in the words the block editor takes — `every weekday` — which
    /// [`BlockEdit::repeat`] reads back unchanged. Absent for a rule the grammar cannot say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition: Option<String>,
    /// Its notes.
    #[serde(default)]
    pub notes: String,
    /// Whether tasks can be put in it.
    pub accepts_tasks: bool,
    /// Whether it counts toward the hours available for work.
    pub counts_capacity: bool,
    /// Whether it is fixed in time.
    pub anchored: bool,
    /// How short re-flow may make it, when set apart from the kind's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_minutes: Option<u32>,
    /// The filter scoping which tasks are offered for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_filter: Option<String>,
    /// The last day a repeating block happens, as an ISO date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    /// Its colour, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
}

/// A rule in the words the date grammar reads, if it can say it.
#[must_use]
pub fn repetition_phrase(rrule: &str, from_completion: bool) -> Option<String> {
    lumenna_core::time::RecurrenceSpec::from_rrule(rrule)?.phrase(from_completion)
}

/// One task assigned to a block for one sitting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PlanAssignment {
    /// Its position among assignments across the whole day, counting from one — so
    /// `lum start 1` works straight after `lum plan`.
    pub row: u32,
    /// The assignment's identifier.
    pub id: String,
    /// The task it puts in the block.
    pub task: String,
    /// The task's title, or a note that this device has not seen it yet.
    pub title: String,
    /// `planned`, `in progress`, `worked`, and so on.
    pub status: String,
    /// How long this sitting is meant to take.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_mins: Option<u32>,
    /// Minutes logged so far.
    pub minutes: u32,
    /// Whether the figure was capped at the block's length because a timer looks orphaned.
    /// Never presented as fact.
    pub capped: bool,
    /// Whether its timer is running now; `status` is "paused" when it has run and stopped
    /// without the sitting ending.
    #[serde(default)]
    pub running: bool,
    /// What a line about it says after its title, as parts every app words alike: "planned
    /// for 45 minutes", or "paused", "20 minutes logged".
    #[serde(default)]
    pub details: Vec<String>,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

/// A timer stopped, or minutes logged by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Timer {
    /// What happened, in a sentence.
    pub announcement: String,
    /// Anything else worth saying — above all that a figure was capped.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// Whether anything was written: stopping a timer that was not running writes nothing.
    pub changed: bool,
    /// The assignment.
    pub assignment: String,
    /// Minutes logged on the sitting.
    pub minutes: u32,
    /// Whether the figure was capped at the block's length.
    pub capped: bool,
}

// ---------------------------------------------------------------------------------------
// Filters and settings
// ---------------------------------------------------------------------------------------

/// The saved filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Filters {
    /// How many there are, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// How many there are.
    pub count: u32,
    /// Them, in order.
    pub filters: Vec<FilterView>,
    /// What a client says in place of them when there are none.
    #[serde(default)]
    pub empty: String,
}

/// One saved filter. Stored as text, never as a resolved date range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct FilterView {
    /// Its position, counting from one.
    pub row: u32,
    /// The full identifier.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The query, as written.
    pub query: String,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

/// Settings, in a fixed order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SettingList {
    /// How many, or the one value asked for.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The settings.
    pub settings: Vec<Setting>,
}

/// One setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Setting {
    /// Its name, as `lum config get` takes it.
    pub key: String,
    /// Its value, as text.
    pub value: String,
    /// What a settings screen calls it: "Day starts".
    #[serde(default)]
    pub title: String,
    /// How its value is shaped, so a client offers the control that fits.
    #[serde(default)]
    pub kind: SettingKind,
    /// The values to choose from, for a toggle or a choice: `id` is the value, `title` what
    /// it is called. A value set elsewhere that is not among them is still the value.
    #[serde(default, skip_serializing_if = "none")]
    pub options: Vec<crate::actions::Choice>,
    /// Whether it syncs to every device, or is this device's alone.
    #[serde(default)]
    pub syncs: bool,
    /// What it does, for under the control. Empty for nothing to say.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
}

/// How a setting's value is shaped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum SettingKind {
    /// On or off: `true` or `false`.
    Toggle,
    /// One of its `options`.
    #[default]
    Choice,
    /// A time of day, `HH:MM`.
    Time,
    /// A whole number.
    Number,
    /// A folder on this device.
    Folder,
}

// ---------------------------------------------------------------------------------------
// Typing: completion and preview
// ---------------------------------------------------------------------------------------

/// What could be inserted where the cursor is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Completions {
    /// How many candidates there are, said before the list.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// UTF-8 byte offset of the text a candidate replaces.
    pub start: u32,
    /// One past its last byte.
    pub end: u32,
    /// What could go there.
    pub candidates: Vec<Candidate>,
}

/// One thing that could be inserted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Candidate {
    /// The text to put in, sigil included where one applies.
    pub text: String,
    /// What sort of thing it is.
    pub kind: String,
    /// What to announce: *"project Work"*.
    pub label: String,
}

impl Completions {
    /// Projects the parser's completions.
    #[must_use]
    pub fn of(completions: &lumenna_parse::complete::Completions) -> Self {
        Self {
            announcement: completions.announcement.clone(),
            notices: Vec::new(),
            start: to_u32(completions.replace_span.0),
            end: to_u32(completions.replace_span.1),
            candidates: completions
                .candidates
                .iter()
                .map(|candidate| Candidate {
                    text: candidate.text.clone(),
                    kind: candidate.kind.noun().to_owned(),
                    label: candidate.label.clone(),
                })
                .collect(),
        }
    }
}

/// Which grammar is being typed, for completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "kebab-case")]
pub enum Syntax {
    /// A quick-add line.
    QuickAdd,
    /// A filter query.
    Filter,
}

/// What a quick-add line would produce, without producing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Preview {
    /// The readback: what would be saved, with the resolved date — the sentence that stands in
    /// for a sighted user's inline highlighting.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The title, cut from the original input so spacing and punctuation survive.
    pub title: String,
    /// The resolved due date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// The time of day it would be due.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    /// The date phrase as typed, to read back beside the resolved value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_phrase: Option<String>,
    /// The repetition in English.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repetition: Option<String>,
    /// The project it would go in, by name. Absent means the Inbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Labels that already exist.
    pub labels: Vec<String>,
    /// Labels that would be created on confirmation.
    pub new_labels: Vec<String>,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// How long it would take.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate_mins: Option<u32>,
    /// Whether confirming would fail.
    pub has_errors: bool,
    /// Everything worth saying before confirming.
    pub diagnostics: Vec<Diagnostic>,
}

/// Something worth saying about a span of the input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Diagnostic {
    /// `error` or `notice`. An error stops the add; a notice does not.
    pub severity: String,
    /// UTF-8 byte offset of the first character it refers to.
    pub start: u32,
    /// One past the last.
    pub end: u32,
    /// The message, complete with position and token — there is no squiggle to point at, and
    /// this text is the only channel.
    pub message: String,
}

impl Preview {
    /// Projects a quick-add preview, resolving the names it points at.
    #[must_use]
    pub fn of(preview: &lumenna_parse::quickadd::Preview, snapshot: &Snapshot) -> Self {
        Self {
            announcement: preview.announcement(),
            notices: Vec::new(),
            title: preview.title.clone(),
            due: preview.due.as_ref().map(|due| due.date.to_string()),
            due_time: preview.due.as_ref().and_then(|due| due.time).map(time_text),
            due_phrase: preview.due_phrase.clone(),
            repetition: preview.repetition.clone(),
            project: preview
                .project
                .and_then(|id| snapshot.projects.get(&id))
                .map(|project| project.name.clone()),
            labels: preview
                .labels
                .iter()
                .filter_map(|id| snapshot.labels.get(id))
                .map(|label| label.name.clone())
                .collect(),
            new_labels: preview.new_labels.clone(),
            priority: preview.priority.as_u8(),
            estimate_mins: preview.estimate_mins,
            has_errors: preview.has_errors(),
            diagnostics: preview
                .diagnostics
                .iter()
                .map(|diagnostic| Diagnostic {
                    severity: match diagnostic.severity {
                        lumenna_parse::quickadd::Severity::Error => "error",
                        lumenna_parse::quickadd::Severity::Notice => "notice",
                    }
                    .to_owned(),
                    start: to_u32(diagnostic.start),
                    end: to_u32(diagnostic.end),
                    message: diagnostic.message.clone(),
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------------------
// Durability
// ---------------------------------------------------------------------------------------

/// Where a backup went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BackupDone {
    /// Where it went, in a sentence.
    pub announcement: String,
    /// Anything else worth saying — that the directory looks cloud-synced, say.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The file written.
    pub path: String,
    /// How many backups that directory now keeps, this one included.
    pub kept: u32,
}

/// A backup as a file's contents, for a client that saves it itself — the browser, as a
/// download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BackupFile {
    /// What was backed up, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// The name a backup taken now has, `lumenna-<when>.lumbak`.
    pub name: String,
    /// The whole backup: every document's history, the trash included.
    pub bytes: Vec<u8>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct RestoreDone {
    /// What came in, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// Documents the backup held that this version could read.
    pub documents: u32,
    /// How many of them brought in anything new.
    pub changed: u32,
    /// Documents of a kind this version does not know, left out.
    #[serde(default, skip_serializing_if = "none")]
    pub unknown: Vec<String>,
}

/// The formats an export can take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// Everything current, structured — the one that imports back.
    #[default]
    Json,
    /// Tasks as a checklist, for reading.
    Markdown,
    /// Tasks as an org outline, for Emacs.
    Org,
    /// Blocks as an iCalendar file, for any calendar.
    Ics,
}

impl ExportFormat {
    /// Its name.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "markdown",
            Self::Org => "org",
            Self::Ics => "ics",
        }
    }

    /// A format by any name people use for it.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        match word.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "markdown" | "md" => Some(Self::Markdown),
            "org" => Some(Self::Org),
            "ics" | "ical" | "icalendar" => Some(Self::Ics),
            _ => None,
        }
    }
}

/// An export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Exported {
    /// What was exported, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// json, markdown, org or ics.
    pub format: String,
    /// The file written, when one was asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The export itself, when no file was asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// What an import of an export did, in records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ImportDone {
    /// What came in, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// Records the store did not have.
    pub created: u32,
    /// Records it had, now matching the file.
    pub updated: u32,
    /// Records already as the file has them.
    pub unchanged: u32,
    /// Assignments left out because their block was nowhere to be found.
    pub skipped: u32,
}

/// What reading a file in did: it was an export, or it was a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Imported {
    /// A JSON export, imported record by record.
    Export {
        /// What it did.
        done: ImportDone,
    },
    /// A backup, merged in.
    Backup {
        /// What it did.
        done: RestoreDone,
    },
}

// ---------------------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------------------

/// The fields of a task to change; `None` leaves one alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct TaskEdit {
    /// A new title.
    #[serde(default)]
    pub title: Option<String>,
    /// A date phrase, or `none` to clear it. A phrase that names no repetition keeps the
    /// one the task has: moving a weekly task to Thursday does not stop it repeating.
    #[serde(default)]
    pub due: Option<String>,
    /// A repetition — `every monday`, `every! 2 weeks` — or `none` to stop it repeating.
    /// A task with no due date becomes due on the first day the repetition lands on.
    #[serde(default)]
    pub repeat: Option<String>,
    /// 1 to 4, where 1 is highest.
    #[serde(default)]
    pub priority: Option<u8>,
    /// How long it should take — `45m`, `1h30m`, a bare number of minutes — or `none`.
    #[serde(default)]
    pub estimate: Option<String>,
    /// Replacement notes.
    #[serde(default)]
    pub notes: Option<String>,
    /// A project to move it to, by name. Subtasks follow.
    #[serde(default)]
    pub project: Option<String>,
    /// The labels it should wear, by name, replacing the ones it has. A name that is not a
    /// label yet becomes one, as in quick add.
    #[serde(default)]
    pub labels: Option<Vec<String>>,
}

/// Where to move a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum MoveTarget {
    /// Under another task, joining its project.
    Parent {
        /// The task to go under.
        id: String,
    },
    /// Into a project by name, subtasks and all.
    Project {
        /// The project.
        name: String,
    },
    /// Out from under its parent, to the top of its project.
    Top,
}

/// A project's weight: its own, or inherited.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Weight {
    /// Take the parent's again.
    Inherit,
    /// This multiplier, roughly 0.5 to 2.0.
    Value {
        /// The multiplier.
        value: f32,
    },
}

/// The fields of a block to change; `None` leaves one alone.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BlockEdit {
    /// What to call it.
    #[serde(default)]
    pub title: Option<String>,
    /// When it starts, such as `9am`.
    #[serde(default)]
    pub at: Option<String>,
    /// How many minutes it lasts.
    #[serde(default)]
    pub minutes: Option<u32>,
    /// `work`, `break` or `event`.
    #[serde(default)]
    pub kind: Option<String>,
    /// A repetition such as `every weekday`, or `none` to make it happen once. The whole
    /// series only: one occurrence cannot repeat differently.
    #[serde(default)]
    pub repeat: Option<String>,
    /// New notes; empty clears them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub notes: Option<String>,
    /// Whether tasks can be put in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub accepts_tasks: Option<bool>,
    /// Whether it counts toward the hours available for work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub counts_capacity: Option<bool>,
    /// Whether it is fixed in time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub anchored: Option<bool>,
    /// How short re-flow may make it, in minutes; `0` returns it to the kind's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub min_minutes: Option<u32>,
    /// A filter scoping which tasks are offered for it; empty clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub task_filter: Option<String>,
    /// The last day it happens, as a date phrase; `none` makes it repeat for good.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub until: Option<String>,
    /// A colour, by name; empty clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub colour: Option<String>,
}

/// Which occurrences a change to a block applies to — asked of a repeating block every
/// time, never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum BlockScope {
    /// Every occurrence: the series itself.
    Series,
    /// Only the occurrence on this day, a date phrase.
    Occurrence {
        /// The day.
        date: String,
    },
}

/// Which way to move something in its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Direction {
    /// One place earlier.
    Up,
    /// One place later.
    Down,
}

/// A new block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct NewBlock {
    /// What to call it.
    pub title: String,
    /// When it starts, such as `9am`.
    pub at: String,
    /// How many minutes it lasts.
    pub minutes: u32,
    /// Which day it starts, as a date phrase. Today if absent.
    #[serde(default)]
    pub date: Option<String>,
    /// `work`, `break` or `event`.
    pub kind: String,
    /// A repetition, such as `every weekday`.
    #[serde(default)]
    pub repeat: Option<String>,
    /// Notes about the block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub notes: Option<String>,
    /// Whether tasks can be put in it; the kind's default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub accepts_tasks: Option<bool>,
    /// Whether it counts toward the hours available for work; the kind's default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub counts_capacity: Option<bool>,
    /// Whether it is fixed in time, never moved when the day slips; the kind's default when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub anchored: Option<bool>,
    /// How short re-flow may make it, in minutes; the kind's default when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub min_minutes: Option<u32>,
    /// A filter scoping which tasks are offered for it, such as `#Work`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub task_filter: Option<String>,
    /// The last day a repeating block happens, as a date phrase; it repeats for good when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub until: Option<String>,
    /// A colour, by name. Presentation only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub colour: Option<String>,
}

// ---------------------------------------------------------------------------------------
// Devices and sync
// ---------------------------------------------------------------------------------------

/// The device a pairing joined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PairedWith {
    /// Who, and how much came across.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// What it is called.
    pub name: String,
    /// What it runs.
    pub platform: String,
    /// Its device key.
    pub node_id: String,
}

/// What syncing over a watch's own link to its phone did: pairing the two the first time,
/// then reconciling the documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct LinkSynced {
    /// Who, and whether this paired them.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// What the other device is called.
    pub name: String,
    /// Its device key.
    pub node_id: String,
    /// Whether this was the first time, which paired the two and took the watch into every
    /// device's list: worth saying. A sync over the link otherwise is not.
    pub paired: bool,
    /// Whether anything came across, so the app redraws.
    pub changed: bool,
}

/// How one device went in a sync round.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct PeerSync {
    /// What it is called.
    pub name: String,
    /// Its device key.
    pub node_id: String,
    /// Whether the sync completed.
    pub synced: bool,
    /// The documents that changed on this side.
    #[serde(default, skip_serializing_if = "none")]
    pub changed: Vec<String>,
    /// Why it did not complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A sync round with every paired device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SyncReport {
    /// How many were reached.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// Each paired device, and how it went.
    pub peers: Vec<PeerSync>,
}

/// One paired device, with how syncing with it last went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DeviceView {
    /// What it is called.
    pub name: String,
    /// What it runs.
    pub platform: String,
    /// Its device key.
    pub node_id: String,
    /// Whether it is the device answering.
    pub this_device: bool,
    /// When it was paired.
    pub paired_at: String,
    /// When this device last tried to sync with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_attempt: Option<String>,
    /// When that last worked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success: Option<String>,
    /// What went wrong, if the last attempt failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    /// The stored-format version it runs; zero if it has not said, as a build from before
    /// versions never does.
    #[serde(default)]
    pub schema_version: u32,
    /// How syncing with it is going, as parts every app words alike: "this device", or
    /// "last synced 5 minutes ago".
    #[serde(default)]
    pub status: Vec<String>,
    /// What can be done to it, in the order offered ([`crate::actions`]).
    #[serde(default, skip_serializing_if = "none")]
    pub actions: Vec<crate::actions::Action>,
}

/// How syncing is going, as words rather than an icon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SyncStatus {
    /// Whether it is running, and with how many devices.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// Whether something on this device holds the sync endpoint — the daemon, or the app.
    pub running: bool,
    /// This device's key.
    pub this_device: String,
    /// Every paired device, this one first.
    pub devices: Vec<DeviceView>,
}

/// The paired devices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct DeviceList {
    /// How many.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "none")]
    pub notices: Vec<String>,
    /// This one first, then by name.
    pub devices: Vec<DeviceView>,
    /// What a client says in place of them when there are none.
    #[serde(default)]
    pub empty: String,
}

/// Which networks syncing may use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum Reach {
    /// Anywhere: a relay and the address lookup service, so devices on different networks
    /// find each other.
    #[default]
    Internet,
    /// The local network only: no relay, no lookup service.
    LocalOnly,
}
