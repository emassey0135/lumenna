//! The typed command surface (§12).
//!
//! Every command returns a [`Response`] rather than printing one. `--json` serialises it;
//! text mode renders it as prose in [`render`](crate::render); `lum rpc` will serialise the
//! same structures over JSON-RPC, and the daemon the same again over its socket — §8's *one
//! protocol, two transports*.
//!
//! Building the surface once is what §12 means by *"this is not new work"*. The CLI, the RPC
//! server, `lum mcp` and the Windows PowerShell module are adapters over these types, not
//! four reimplementations of the same operations. A command that can only be reached by
//! reading prose off stdout is not on the surface.
//!
//! **Rows carry components, never a sentence** (§13). A client assembles speech and braille
//! itself from `role`, `state`, `title` and `value`, because the two compose differently —
//! braille abbreviates the role and renders the title verbatim, speech does neither. What is
//! composed *here* is only [`Response::announcement`], and only because core composed it
//! first: the quick-add readback and an edit's description are core's sentences, not the
//! CLI's.
//!
//! `--json` is a compatibility contract (§15). Reshaping anything in this file is a breaking
//! change, and [`VERSION`] says which shape a reader is looking at.

use lumenna_core::State;
use lumenna_core::edit::{Change as CoreChange, Edit};
use lumenna_core::filter::NameKind;
use lumenna_core::model::{Priority, Task};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;
use serde::Serialize;

/// The contract version, carried on every response. Bumped when a shape changes in a way a
/// reader could not survive.
pub const VERSION: u32 = 1;

/// What one command produced.
#[derive(Debug, Serialize)]
pub struct Response {
    /// The contract version.
    pub version: u32,
    /// One sentence saying what happened, for a client with nothing better to say. Composed
    /// in core — an [`Edit`]'s description, or a quick-add readback — never here.
    pub announcement: String,
    /// Things worth saying that are not the answer: a capped timer, a name the filter did
    /// not recognise. Text mode prints these to stderr, so a pipeline keeps its payload.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The payload.
    #[serde(flatten)]
    pub outcome: Outcome,
    /// Whether text mode should print nothing — `lum task add --quiet`. A structured reader
    /// gets the same response either way: suppressing output is a terminal convenience, not
    /// a smaller result.
    #[serde(skip)]
    pub silent: bool,
}

impl Response {
    /// A response with nothing to add beyond its outcome.
    pub fn new(announcement: impl Into<String>, outcome: Outcome) -> Self {
        Self {
            version: VERSION,
            announcement: announcement.into(),
            notices: Vec::new(),
            outcome,
            silent: false,
        }
    }

    /// Suppresses text output without changing the response.
    #[must_use]
    pub const fn quietly(mut self) -> Self {
        self.silent = true;
        self
    }

    /// Adds something worth saying that is not the answer.
    #[must_use]
    pub fn note(mut self, notice: impl Into<String>) -> Self {
        self.notices.push(notice.into());
        self
    }

    /// A mutation that changed something, announced as core described it.
    pub fn changed(edit: &Edit) -> Self {
        Self::changed_as(edit.description.clone(), edit)
    }

    /// A mutation announced in the CLI's own words, where core's description is not the
    /// whole story — a trashed task that says how to get it back, say.
    pub fn changed_as(announcement: impl Into<String>, edit: &Edit) -> Self {
        Self::new(
            announcement,
            Outcome::Change(Change {
                changed: !edit.is_empty(),
                affected: Affected::of(edit),
                task: None,
            }),
        )
    }

    /// Attaches the task a command created (§12's `add_task(text) -> Task`).
    #[must_use]
    pub fn with_task(mut self, task: TaskDetail) -> Self {
        if let Outcome::Change(change) = &mut self.outcome {
            change.task = Some(Box::new(task));
        }
        self
    }

    /// A mutation written straight to the store rather than through an [`Edit`] — block
    /// series, which `core::edit` does not cover yet.
    pub fn touched(announcement: impl Into<String>, affected: Affected) -> Self {
        Self::new(announcement, Outcome::Change(Change { changed: true, affected, task: None }))
    }

    /// A mutation that turned out to have nothing to do. Distinct from a failure: asking for
    /// a state something is already in is not an error, but a client that cannot tell the
    /// two apart will announce a change that did not happen.
    pub fn unchanged(announcement: impl Into<String>) -> Self {
        Self::new(
            announcement,
            Outcome::Change(Change {
                changed: false,
                affected: Affected::default(),
                task: None,
            }),
        )
    }
}

/// The payload of a [`Response`], tagged so a reader can dispatch on it.
#[derive(Debug, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Outcome {
    /// A mutation.
    Change(Change),
    /// A listing of rows, of one kind.
    Rows(Rows),
    /// Everything about one task.
    Task(Box<TaskDetail>),
    /// A day's blocks and what is assigned to them.
    Plan(Plan),
    /// Saved filters.
    Filters(Filters),
    /// Settings.
    Settings(SettingList),
    /// A timer that stopped.
    Timer(Timer),
    /// What could be typed next (§6.3). Reachable over `lum rpc` only: completion is a
    /// keystroke-rate question, and a process per keystroke is not an answer.
    Completions(Completions),
    /// What a quick-add line would produce, without producing it (§6.1).
    Preview(Preview),
    /// What this server is, for a client checking it can talk to it.
    Server(ServerInfo),
    /// A backup was written.
    Backup(BackupDone),
    /// A backup was merged in.
    Restore(RestoreDone),
    /// The current state was exported.
    Export(Exported),
    /// An export was read in.
    Import(ImportDone),
    /// A pairing finished (§7).
    Paired(PairedWith),
    /// A sync round with every paired device.
    Synced(SyncReport),
    /// How sync is going (§9).
    SyncStatus(SyncStatus),
    /// The paired devices.
    Devices(DeviceList),
}

// ---------------------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------------------

/// What a mutation did.
#[derive(Debug, Serialize)]
pub struct Change {
    /// Whether anything actually changed.
    pub changed: bool,
    /// The records it touched, so a client can act on what it just made without listing
    /// everything again to find it. Absent when nothing moved, so its presence means there
    /// is something to act on.
    #[serde(skip_serializing_if = "Affected::is_empty")]
    pub affected: Affected,
    /// The task itself, when the command created one. §12's surface is `add_task(text) ->
    /// Task`, and this is that return: a client that has just captured something needs it
    /// in hand, not an identifier to look up. Commands that merely *change* a task leave
    /// this empty — [`affected`](Self::affected) names it, and `show` fetches it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Box<TaskDetail>>,
}

/// Identifiers a change touched, by kind.
#[derive(Debug, Default, Serialize)]
pub struct Affected {
    /// Tasks.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<String>,
    /// Projects.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<String>,
    /// Labels.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
    /// Saved filters.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
    /// Block series.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<String>,
    /// Assignments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<String>,
    /// Devices paired, renamed or unpaired.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
    /// Whether the settings singleton moved.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub settings: bool,
}

impl Affected {
    /// Reads the identifiers straight off the edit, so every operation reports what it
    /// touched without each one remembering to.
    ///
    /// A completion and an exception are not addressable records, but they *name* one — so
    /// they report the task or series they are about, and a completed task shows up here
    /// even though the edit only wrote a completion. Reminders and their acknowledgements
    /// are absent entirely: nothing addresses them yet.
    fn of(edit: &Edit) -> Self {
        let mut affected = Self::default();
        for change in &edit.changes {
            match change {
                CoreChange::Task(transition) => {
                    push(&mut affected.tasks, transition.before.as_ref().map(|t| t.id), transition.after.as_ref().map(|t| t.id));
                }
                CoreChange::Project(transition) => {
                    push(&mut affected.projects, transition.before.as_ref().map(|p| p.id), transition.after.as_ref().map(|p| p.id));
                }
                CoreChange::Label(transition) => {
                    push(&mut affected.labels, transition.before.as_ref().map(|l| l.id), transition.after.as_ref().map(|l| l.id));
                }
                CoreChange::Filter(transition) => {
                    push(&mut affected.filters, transition.before.as_ref().map(|f| f.id), transition.after.as_ref().map(|f| f.id));
                }
                CoreChange::Series(transition) => {
                    push(&mut affected.blocks, transition.before.as_ref().map(|s| s.id), transition.after.as_ref().map(|s| s.id));
                }
                CoreChange::Assignment { transition, .. } => {
                    push(&mut affected.assignments, transition.before.as_ref().map(|a| a.id), transition.after.as_ref().map(|a| a.id));
                }
                CoreChange::Completion(transition) => {
                    let named = transition.after.as_ref().or(transition.before.as_ref());
                    push(&mut affected.tasks, None, named.map(|c| c.task_id));
                }
                CoreChange::Exception(transition) => {
                    let named = transition.after.as_ref().or(transition.before.as_ref());
                    push(&mut affected.blocks, None, named.map(|e| e.series_id));
                }
                CoreChange::Settings { .. } => affected.settings = true,
                CoreChange::Device(transition) => {
                    let named = transition.after.as_ref().or(transition.before.as_ref());
                    push(&mut affected.devices, None, named.map(|d| d.node_id));
                }
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

/// Records the identifier once, whichever side of the transition carries it.
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
#[derive(Debug, Serialize)]
pub struct Rows {
    /// The singular noun for the count line — *"17 tasks"*.
    pub noun: String,
    /// How many rows there are, so a reader knows before the rows arrive.
    pub count: usize,
    /// How the query was understood, when there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<Query>,
    /// The rows.
    pub rows: Vec<RowView>,
}

/// A filter query, read back (§6.2).
#[derive(Debug, Serialize)]
pub struct Query {
    /// The query as it was written.
    pub text: String,
    /// The query as it was understood. A mis-parsed filter shows wrong results silently, and
    /// wrong results are invisible — so every client should have this to hand.
    pub description: String,
    /// Names in the query that match nothing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unresolved: Vec<Unresolved>,
}

/// A name in a query that matches nothing.
#[derive(Debug, Serialize)]
pub struct Unresolved {
    /// `project` or `label`.
    pub kind: &'static str,
    /// The name as written.
    pub name: String,
    /// The nearest real name, if one is near enough to suggest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// One row, as its components (§13).
#[derive(Debug, Serialize)]
pub struct RowView {
    /// The row number in this listing, which is what `lum task done 3` takes.
    pub row: usize,
    /// The full identifier.
    pub id: String,
    /// What kind of thing it is.
    pub role: &'static str,
    /// Depth in the tree, zero at the top.
    pub depth: u32,
    /// Position within its sibling set, counting from one.
    pub index: u32,
    /// How many siblings, including itself.
    pub count: u32,
    /// Whether it is checked off, where that means anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    /// Whether it can be expanded, and whether the projection left it expanded. Absent for
    /// a leaf. A client tracks its own collapse state; this only says there is something
    /// under it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// The content, verbatim. Never abbreviated, in any medium.
    pub title: String,
    /// Computed states, as their filter keywords — so a client can feed one straight back
    /// into `lum task list`.
    pub state: Vec<&'static str>,
    /// A secondary value, such as a due date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// What can be done here, for a client with somewhere to put a hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl RowView {
    /// The states worth putting on a printed line. `ready` is true of almost every task, so
    /// printing it everywhere buries the states that mean something. A client that wants the
    /// full set has [`state`](Self::state).
    pub fn trailing_states(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.state.iter().copied().filter(|keyword| *keyword != State::Ready.keyword())
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

/// The kind name a row is remembered and addressed under.
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

impl Rows {
    /// Projects core's rows onto the wire, numbering them from one.
    pub fn new(rows: &[Row], noun: &str) -> Self {
        Self {
            noun: noun.to_owned(),
            count: rows.len(),
            query: None,
            rows: rows
                .iter()
                .enumerate()
                .map(|(position, row)| RowView {
                    row: position + 1,
                    id: row_id(&row.id),
                    role: role_name(row.role),
                    depth: row.depth,
                    index: row.index,
                    count: row.count,
                    checked: row.checked,
                    expanded: row.expanded,
                    title: row.title.clone(),
                    state: row.state.iter().map(|state| state.keyword()).collect(),
                    value: row.value.clone(),
                    hint: row.hint.clone(),
                })
                .collect(),
        }
    }

    /// Attaches the readback for the query that produced these rows.
    #[must_use]
    pub fn with_query(mut self, query: Query) -> Self {
        self.query = Some(query);
        self
    }
}

/// The noun a name kind is called by, for [`Unresolved`].
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

/// Everything about one task.
#[derive(Debug, Serialize)]
pub struct TaskDetail {
    /// The full identifier.
    pub id: String,
    /// The content, verbatim.
    pub title: String,
    /// Free text.
    pub notes: String,
    /// The project it sits in, by name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// The task it sits under, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// Label names, without the leading `@`.
    pub labels: Vec<String>,
    /// What it waits for.
    pub depends: Vec<Dependency>,
    /// The due date, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// The time of day it is due, if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    /// The repetition, as an RFC 5545 rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<String>,
    /// How long it should take.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_mins: Option<u32>,
    /// Computed states, as their filter keywords. The full set, not just the notable ones —
    /// a detail view is where the near-universal states are worth having.
    pub state: Vec<&'static str>,
    /// When it was created.
    pub created_at: String,
}

/// A task another task waits for.
#[derive(Debug, Serialize)]
pub struct Dependency {
    /// The full identifier.
    pub id: String,
    /// Its title, or a note that this device has not seen it yet (§3.1).
    pub title: String,
}

impl TaskDetail {
    /// Reads one task out of a snapshot, resolving the names it points at.
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
            due_time: task
                .due
                .as_ref()
                .and_then(|due| due.time)
                .map(crate::render::time_text),
            recurrence: task
                .due
                .as_ref()
                .and_then(|due| due.recurrence.as_ref())
                .map(|r| r.rrule.clone()),
            estimate_mins: task.estimate_mins,
            state: snapshot.states_of(task, now).iter().map(|s| s.keyword()).collect(),
            created_at: task.created_at.to_string(),
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
#[derive(Debug, Serialize)]
pub struct Plan {
    /// The day, as an ISO date.
    pub date: String,
    /// How many blocks it holds.
    pub count: usize,
    /// The blocks, in time order.
    pub blocks: Vec<PlanBlock>,
}

/// One block on a day.
#[derive(Debug, Serialize)]
pub struct PlanBlock {
    /// Its row number among blocks, counting from one.
    pub row: usize,
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
    /// What is assigned to it, in order.
    pub assignments: Vec<PlanAssignment>,
}

/// One task assigned to a block for one sitting (§3.7).
#[derive(Debug, Serialize)]
pub struct PlanAssignment {
    /// Its row number among assignments, counting from one across the whole day — so
    /// `lum start 1` works straight after `lum plan`.
    pub row: usize,
    /// The assignment's identifier.
    pub id: String,
    /// The task it puts in the block.
    pub task: String,
    /// The task's title, or a note that this device has not seen it yet (§3.1).
    pub title: String,
    /// `planned`, `in progress`, `done`, and so on.
    pub status: &'static str,
    /// How long this sitting is meant to take.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned_mins: Option<u32>,
    /// Minutes logged so far.
    pub minutes: u32,
    /// Whether the figure was capped at the block's length because a timer looks orphaned.
    /// Never presented as fact (§3.7).
    pub capped: bool,
}

// ---------------------------------------------------------------------------------------
// Filters and settings
// ---------------------------------------------------------------------------------------

/// The saved filters.
#[derive(Debug, Serialize)]
pub struct Filters {
    /// How many there are.
    pub count: usize,
    /// Them, in order.
    pub filters: Vec<FilterView>,
}

/// One saved filter. Stored as text, never as a resolved date range (§6.2).
#[derive(Debug, Serialize)]
pub struct FilterView {
    /// Its row number, counting from one.
    pub row: usize,
    /// The full identifier.
    pub id: String,
    /// Its name.
    pub name: String,
    /// The query, as written.
    pub query: String,
}

/// Settings, in a fixed order.
#[derive(Debug, Serialize)]
pub struct SettingList {
    /// The settings.
    pub settings: Vec<Setting>,
}

/// One setting.
#[derive(Debug, Serialize)]
pub struct Setting {
    /// Its name, as `lum config get` takes it.
    pub key: String,
    /// Its value, as text.
    pub value: String,
}

/// A timer that stopped.
#[derive(Debug, Serialize)]
pub struct Timer {
    /// The assignment it was running on.
    pub assignment: String,
    /// Minutes logged.
    pub minutes: u32,
    /// Whether the figure was capped at the block's length (§3.7).
    pub capped: bool,
}

// ---------------------------------------------------------------------------------------
// Reachable over RPC only
// ---------------------------------------------------------------------------------------

/// What could be inserted where the cursor is.
#[derive(Debug, Serialize)]
pub struct Completions {
    /// Byte offsets of the text a candidate replaces.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
    /// What could go there.
    pub candidates: Vec<Candidate>,
}

/// One thing that could be inserted.
#[derive(Debug, Serialize)]
pub struct Candidate {
    /// The text to put in, sigil included where one applies.
    pub text: String,
    /// What sort of thing it is.
    pub kind: &'static str,
    /// What to announce: *"project Work"*.
    pub label: String,
}

impl Completions {
    /// Projects the parser's completions onto the wire.
    ///
    /// The count is *not* carried: §6.3 wants it announced before the list, and `candidates`
    /// already has a length. Sending a sentence would be composing for a client whose output
    /// medium this does not know (§13).
    pub fn of(completions: &lumenna_parse::complete::Completions) -> Self {
        Self {
            start: completions.replace_span.0,
            end: completions.replace_span.1,
            candidates: completions
                .candidates
                .iter()
                .map(|candidate| Candidate {
                    text: candidate.text.clone(),
                    kind: candidate.kind.noun(),
                    label: candidate.label.clone(),
                })
                .collect(),
        }
    }
}

/// What a quick-add line would produce, without producing it.
#[derive(Debug, Serialize)]
pub struct Preview {
    /// The title, cut from the original input so spacing and punctuation survive.
    pub title: String,
    /// The resolved due date.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// The time of day it would be due.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_time: Option<String>,
    /// The date phrase as typed, for reading back beside the resolved value — §6.1 wants
    /// both, because "Friday" is the ambiguous part.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_phrase: Option<String>,
    /// The repetition in English.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repetition: Option<String>,
    /// The project it would go in, by name. Absent means the Inbox.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Labels that already exist.
    pub labels: Vec<String>,
    /// Labels that would be created on confirmation (§3.4).
    pub new_labels: Vec<String>,
    /// 1 to 4, where 1 is highest.
    pub priority: u8,
    /// How long it would take.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_mins: Option<u32>,
    /// Whether anything would be lost or mistaken by confirming.
    pub has_errors: bool,
    /// Everything worth saying before confirming.
    pub diagnostics: Vec<Diagnostic>,
}

/// Something worth saying about a span of the input.
#[derive(Debug, Serialize)]
pub struct Diagnostic {
    /// `error` or `notice`. An error stops the add; a notice does not.
    pub severity: &'static str,
    /// Byte offset of the first character it refers to.
    pub start: usize,
    /// One past the last.
    pub end: usize,
    /// The message, complete with position and token — there is no squiggle to point at and
    /// this text is the only channel (§6.3).
    pub message: String,
}

impl Preview {
    /// Projects a quick-add preview onto the wire, resolving the names it points at.
    pub fn of(preview: &lumenna_parse::quickadd::Preview, snapshot: &Snapshot) -> Self {
        Self {
            title: preview.title.clone(),
            due: preview.due.as_ref().map(|due| due.date.to_string()),
            due_time: preview
                .due
                .as_ref()
                .and_then(|due| due.time)
                .map(crate::render::time_text),
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
                    },
                    start: diagnostic.start,
                    end: diagnostic.end,
                    message: diagnostic.message.clone(),
                })
                .collect(),
        }
    }
}

/// What this server is.
#[derive(Debug, Serialize)]
pub struct ServerInfo {
    /// Always `lumenna`.
    pub name: &'static str,
    /// The binary's version.
    pub version: &'static str,
    /// The version of the shapes in this module — §15's compatibility contract. A client
    /// that does not know this number should refuse to guess.
    pub contract: u32,
    /// Every method this server answers, so a client can find out rather than assume.
    pub methods: Vec<&'static str>,
}

// ---------------------------------------------------------------------------------------
// Durability (§9)
// ---------------------------------------------------------------------------------------

/// Where a backup went.
#[derive(Debug, Serialize)]
pub struct BackupDone {
    /// The file written.
    pub path: String,
    /// How many backups that directory now keeps, this one included.
    pub kept: usize,
}

/// What a restore did.
#[derive(Debug, Serialize)]
pub struct RestoreDone {
    /// Documents the backup held that this version could read.
    pub documents: usize,
    /// How many of them brought in anything new.
    pub changed: usize,
    /// Documents of a kind this version does not know, left out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unknown: Vec<String>,
}

/// An export.
#[derive(Debug, Serialize)]
pub struct Exported {
    /// json, markdown, org or ics.
    pub format: &'static str,
    /// The file written, when one was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The export itself, when no file was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// What an import did, in records.
#[derive(Debug, Serialize)]
pub struct ImportDone {
    /// Records the store did not have.
    pub created: usize,
    /// Records it had, now matching the file.
    pub updated: usize,
    /// Records already as the file has them.
    pub unchanged: usize,
    /// Assignments left out because their block was nowhere to be found.
    pub skipped: usize,
}

// ---------------------------------------------------------------------------------------
// Sync (§7, §9)
// ---------------------------------------------------------------------------------------

/// The device a pairing joined.
#[derive(Debug, Serialize)]
pub struct PairedWith {
    /// What it is called.
    pub name: String,
    /// What it runs.
    pub platform: String,
    /// Its device key.
    pub node_id: String,
}

/// How one device went in a sync round.
#[derive(Debug, Default, Serialize, serde::Deserialize)]
pub struct PeerSync {
    /// What it is called.
    pub name: String,
    /// Its device key.
    pub node_id: String,
    /// Whether the sync completed.
    pub synced: bool,
    /// The documents that changed on this side.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    /// Why it did not complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A sync round.
#[derive(Debug, Serialize)]
pub struct SyncReport {
    /// Each paired device, and how it went.
    pub peers: Vec<PeerSync>,
}

/// One paired device, with how syncing with it last went.
#[derive(Debug, Serialize)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attempt: Option<String>,
    /// When that last worked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<String>,
    /// What went wrong, if the last attempt failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// §9's `sync_status()`.
#[derive(Debug, Serialize)]
pub struct SyncStatus {
    /// Whether a process on this device is holding the endpoint — the daemon, usually.
    pub running: bool,
    /// This device's key.
    pub this_device: String,
    /// Every paired device, this one first.
    pub devices: Vec<DeviceView>,
}

/// The paired devices.
#[derive(Debug, Serialize)]
pub struct DeviceList {
    /// This one first, then by name.
    pub devices: Vec<DeviceView>,
}
