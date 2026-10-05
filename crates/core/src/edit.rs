//! Mutations: what a change *is*, computed here and applied by `store`.
//!
//! Nothing in this module writes anything. Each operation reads a [`Snapshot`] and returns
//! an [`Edit`] — a list of before-and-after record pairs — which `store` turns into Automerge
//! operations. That split exists for three reasons, in ascending order of how much they
//! matter:
//!
//! 1. Core keeps its promise of no I/O, so every rule below is testable against a plain
//!    struct with no CRDT and no database in the room.
//! 2. A caller can inspect or announce a change *before* committing it, which is what the
//!    readbacks and confirmation prompts need.
//! 3. **Undo falls out for free.** Automerge does not provide undo — it provides history,
//!    and rewinding would discard concurrent remote changes along with your own. Undo means computing and applying an *inverse*, and an edit that already
//!    carries both sides of every record is its own inverse when you swap them.
//!
//! # Why operations are not just field assignments
//!
//! Completing a task is the worked example. It writes a
//! [`TaskCompletion`](crate::model::TaskCompletion), cascades to subtasks if the setting
//! says so, and advances the due date if the task recurs — three records' worth of
//! consequence from one keystroke, with rules about each that no caller should have to
//! remember. Leaving that to eleven UI targets would leak business logic into every one of
//! them, and they would drift.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;

use crate::id::{AssignmentId, FilterId, LabelId, ProjectId, TaskId};
use crate::model::{
    BlockAssignment, BlockException, BlockRef, BlockSeries, Device, ExceptionAction, Label,
    Project, Reminder, ReminderAck, SavedFilter, Settings, Task, TaskCompletion,
};
use crate::order::OrderKey;
use crate::recur::{self, Advanced};
use crate::snapshot::Snapshot;
use crate::time;

/// A refusal to compute an edit.
///
/// Deliberately narrow. Almost everything that could be wrong about a record is something
/// CRDT merge can produce anyway, and refusing to load or refusing to edit would be
/// worse than tolerating it. What is here is the small set where proceeding would corrupt
/// something a repair could not sensibly fix.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The record is not in this snapshot. It may exist in a document that is not loaded.
    #[error("no such {kind} in the loaded documents")]
    NotFound {
        /// What was being looked for.
        kind: &'static str,
    },

    /// Reparenting would put something inside itself.
    ///
    /// Merge can still produce a cycle from two concurrent moves, and [`crate::repair`]
    /// exists for that. But a *local* move that closes a cycle is a mistake this device can
    /// see, and letting it through would mean the repair silently undoing what the user just
    /// asked for.
    #[error("that would put an item inside itself")]
    WouldCycle,

    /// The new dependency would close a loop: the task it names already waits, directly or
    /// through others, for the task being given it.
    ///
    /// Merge can still produce one, and [`crate::repair`] drops an edge when it does — but
    /// that edge is the newest task's, which need not be the one just added. Refusing here
    /// is what keeps the repair from quietly undoing some other dependency instead.
    #[error("that would make tasks wait for each other in a circle")]
    DependencyCycle,

    /// The task is already finished, so completing it again would record a second
    /// completion for one occurrence.
    #[error("that task is already complete")]
    AlreadyComplete,

    /// The task has never been completed, so there is nothing to reverse.
    #[error("that task is not complete")]
    NotComplete,

    /// The block does not occur on the day the assignment names — before its series
    /// starts, after it ends, on a day its rule skips, or on one that was cancelled.
    #[error("that block does not happen on {date}")]
    NoOccurrence {
        /// The day that was asked for.
        date: jiff::civil::Date,
    },

    /// A single occurrence was singled out of a block that happens only once. Its one
    /// occurrence is the block itself, which changes through the series.
    #[error("that block happens once, so change the block itself")]
    NotRepeating,

    /// The block is a break or an event, which take no tasks.
    #[error("that block does not take tasks")]
    RefusesTasks,

    /// The task is in the trash.
    #[error("that task is in the trash")]
    Trashed,

    /// A stored recurrence rule could not be used.
    #[error(transparent)]
    Recurrence(#[from] recur::RecurError),
}

/// One record's transition. `None` on either side means it did not exist.
///
/// Every variant carries both sides, which is what makes [`Edit::inverse`] a swap rather
/// than a second implementation of every operation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Change {
    /// A task.
    Task(Box<Transition<Task>>),
    /// A completion record.
    Completion(Box<Transition<TaskCompletion>>),
    /// A project.
    Project(Box<Transition<Project>>),
    /// A label.
    Label(Box<Transition<Label>>),
    /// A saved filter.
    Filter(Box<Transition<SavedFilter>>),
    /// A block series, in the year it starts.
    Series(Box<Transition<BlockSeries>>),
    /// An exception, keyed by the occurrence it modifies.
    Exception(Box<Transition<BlockException>>),
    /// An assignment. Carries its year, because
    /// [`BlockRef::OneOff`](crate::model::BlockRef::OneOff) does not name a date and the
    /// store cannot shard it without reading the series.
    Assignment {
        /// Which `blocks-<year>` document it belongs in.
        year: i16,
        /// The transition.
        transition: Box<Transition<BlockAssignment>>,
    },
    /// A reminder.
    Reminder(Box<Transition<Reminder>>),
    /// A reminder acknowledgement.
    Ack(Box<Transition<ReminderAck>>),
    /// A paired device.
    Device(Box<Transition<Device>>),
    /// The settings singleton, which always exists.
    Settings {
        /// What it was.
        before: Box<Settings>,
        /// What it becomes.
        after: Box<Settings>,
    },
}

/// A record before and after.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transition<T> {
    /// What was there, if anything.
    pub before: Option<T>,
    /// What is there afterwards. `None` removes the record outright — *purge*, not trash.
    pub after: Option<T>,
}

impl<T> Transition<T> {
    fn created(after: T) -> Self {
        Self { before: None, after: Some(after) }
    }

    fn updated(before: T, after: T) -> Self {
        Self { before: Some(before), after: Some(after) }
    }

    fn removed(before: T) -> Self {
        Self { before: Some(before), after: None }
    }

    fn flip(self) -> Self {
        Self { before: self.after, after: self.before }
    }
}

impl Change {
    /// The same record change, backwards.
    #[must_use]
    pub fn inverse(self) -> Self {
        match self {
            Self::Task(t) => Self::Task(Box::new(t.flip())),
            Self::Completion(t) => Self::Completion(Box::new(t.flip())),
            Self::Project(t) => Self::Project(Box::new(t.flip())),
            Self::Label(t) => Self::Label(Box::new(t.flip())),
            Self::Filter(t) => Self::Filter(Box::new(t.flip())),
            Self::Series(t) => Self::Series(Box::new(t.flip())),
            Self::Exception(t) => Self::Exception(Box::new(t.flip())),
            Self::Assignment { year, transition } => {
                Self::Assignment { year, transition: Box::new(transition.flip()) }
            }
            Self::Reminder(t) => Self::Reminder(Box::new(t.flip())),
            Self::Ack(t) => Self::Ack(Box::new(t.flip())),
            Self::Device(t) => Self::Device(Box::new(t.flip())),
            Self::Settings { before, after } => Self::Settings { before: after, after: before },
        }
    }
}

/// One user action, as a set of record changes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Edit {
    /// What the user did, phrased for announcement: *"Completed Review PR"*.
    ///
    /// Undo has to be announceable — *"Undid: completed Review PR"* — because
    /// without a visual channel a mis-keystroke can go unnoticed for minutes, by which point
    /// the context for recovering it is gone.
    pub description: String,
    /// The records that change, in the order they should be applied.
    pub changes: Vec<Change>,
}

impl Edit {
    /// An edit that does nothing.
    #[must_use]
    pub fn nothing() -> Self {
        Self { description: String::new(), changes: Vec::new() }
    }

    /// Whether anything would actually change.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// The edit that puts everything back.
    ///
    /// Changes are reversed in order as well as individually, so an edit that creates a
    /// record and then references it undoes in the order that keeps each step consistent.
    ///
    /// The description is **not** rephrased: it still names the action, and a caller says
    /// *"Undid: {description}"*. Rewriting it here would mean guessing at grammar the
    /// caller can produce correctly.
    #[must_use]
    pub fn inverse(self) -> Self {
        Self {
            description: self.description,
            changes: self.changes.into_iter().rev().map(Change::inverse).collect(),
        }
    }
}

/// Builds an edit, one record at a time.
struct Builder {
    description: String,
    changes: Vec<Change>,
}

impl Builder {
    fn new(description: impl Into<String>) -> Self {
        Self { description: description.into(), changes: Vec::new() }
    }

    fn task(&mut self, transition: Transition<Task>) -> &mut Self {
        self.changes.push(Change::Task(Box::new(transition)));
        self
    }

    fn completion(&mut self, transition: Transition<TaskCompletion>) -> &mut Self {
        self.changes.push(Change::Completion(Box::new(transition)));
        self
    }

    fn finish(&mut self) -> Edit {
        Edit { description: std::mem::take(&mut self.description), changes: std::mem::take(&mut self.changes) }
    }
}

// ---------------------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------------------

/// Adds a task.
#[must_use]
pub fn create_task(task: Task) -> Edit {
    let mut builder = Builder::new(format!("Added {}", task.title));
    builder.task(Transition::created(task));
    builder.finish()
}

/// Replaces a task with an edited copy.
///
/// `before` must be the version the user actually edited — the same rule `store` states for
/// writes, and for the same reason: only the fields that differ produce operations, so a
/// field someone else changed on another device survives.
#[must_use]
pub fn update_task(before: Task, after: Task) -> Edit {
    if before == after {
        return Edit::nothing();
    }
    let mut builder = Builder::new(format!("Edited {}", after.title));
    builder.task(Transition::updated(before, after));
    builder.finish()
}

/// Marks a task complete, cascading and advancing as the model requires.
///
/// Three things happen, and the order matters only in that they are one edit:
///
/// - A [`TaskCompletion`](crate::model::TaskCompletion) is recorded. For a recurring task it
///   names the occurrence, since a recurring task is one task whose date advances rather
///   than a generated series — and so does one for a subtask of a recurring task,
///   which recurs with it (see [`Snapshot::occurrence_of`]).
/// - **Subtasks cascade**, if [`Settings::cascade_complete_subtasks`] is on. Their
///   completions record what caused them, so uncompleting the parent later reverses only
///   these and not a subtask you independently finished last week.
/// - **A recurring task advances** rather than ending. If the rule has run out, it
///   simply stays complete.
///
/// # Errors
///
/// If the task is not loaded, is already complete, or carries a rule that cannot be expanded.
pub fn complete_task(snapshot: &Snapshot, id: TaskId, now: &Zoned) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let facts = snapshot.facts();
    if facts.is_completed(task) {
        return Err(EditError::AlreadyComplete);
    }
    let occurrence = snapshot.occurrence_of(task);

    // Every completion this edit writes shares one timestamp. That is what lets
    // `uncomplete_task` find the cascade that belongs to *this* completion of the parent and
    // leave the cascades of earlier occurrences alone.
    let stamp = time::now();
    let mut builder = Builder::new(format!("Completed {}", task.title));
    builder.completion(Transition::created(TaskCompletion {
        completed_at: stamp,
        ..TaskCompletion::new(id, occurrence)
    }));

    if snapshot.settings.cascade_complete_subtasks {
        for subtask in descendants(snapshot, id) {
            if facts.is_completed(subtask) || subtask.is_deleted() {
                continue;
            }
            // The completion names the subtask's *own* occurrence — its own due date if it
            // recurs, its recurring ancestor's otherwise — since that is what `is_completed`
            // checks it against. Under a recurring parent that is the occurrence just closed,
            // so the subtask reopens when the parent comes round again.
            //
            // Cascaded completions never *advance* a recurring subtask, though. The cascade
            // means "the parent is finished, so these are finished too" — moving one to next
            // week instead would resurrect the very thing that was just closed out.
            let occurrence = snapshot.occurrence_of(subtask);
            builder.completion(Transition::created(TaskCompletion {
                completed_at: stamp,
                ..TaskCompletion::cascaded(subtask.id, occurrence, id)
            }));
        }
    }

    if let Some(due) = &task.due
        && due.recurrence.is_some()
    {
        let completed = snapshot.completion_count(id).saturating_add(1);
        if let Advanced::Next(next) = recur::advance(due, now.date(), completed)? {
            let mut advanced = task.clone();
            advanced.due = Some(next);
            builder.task(Transition::updated(task.clone(), advanced));
        }
    }
    Ok(builder.finish())
}

/// How far apart a cascade and the completion that caused it may have been stamped.
///
/// Since `complete_task` stamps them identically they are normally equal. Completions written
/// before that took the clock once per record, so they can sit a millisecond or two apart;
/// a second covers that with room to spare, and two separate completions of one parent
/// inside a second of each other are not a real case.
const CASCADE_WINDOW_MS: i64 = 1_000;

/// Reverses the most recent completion of a task.
///
/// For a recurring task this also rolls the due date back to the occurrence that was
/// completed, which is recoverable exactly because the completion record names it.
///
/// Only completions **caused by the completion being reversed** go with it. A subtask you
/// finished independently last week must not be uncompleted because you changed your mind
/// about the parent — and neither must the cascades of a recurring parent's earlier
/// occurrences, which were separate completions.
///
/// A completion a cascade wrote can be reversed on its own, too. The subtask reads as
/// complete, so it has to be possible to say it is not.
///
/// # Errors
///
/// If the task is not loaded or has never been completed.
pub fn uncomplete_task(snapshot: &Snapshot, id: TaskId) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let latest = snapshot
        .completions
        .values()
        .filter(|c| c.task_id == id)
        .max_by_key(|c| (c.completed_at, c.id))
        .ok_or(EditError::NotComplete)?;

    let mut builder = Builder::new(format!("Uncompleted {}", task.title));
    builder.completion(Transition::removed(latest.clone()));

    if latest.cascaded_from.is_none() {
        let at = latest.completed_at.as_millisecond();
        for completion in snapshot.completions.values() {
            if completion.cascaded_from == Some(id)
                && (completion.completed_at.as_millisecond() - at).abs() <= CASCADE_WINDOW_MS
            {
                builder.completion(Transition::removed(completion.clone()));
            }
        }
    }

    // Only a task that recurs itself moves its date back. A subtask's completion can name
    // its recurring parent's occurrence, which says nothing about the subtask's own due date.
    if let Some(occurrence) = latest.occurrence_date
        && let Some(due) = &task.due
        && due.recurrence.is_some()
        && due.date != occurrence
    {
        let mut rolled = task.clone();
        if let Some(due) = rolled.due.as_mut() {
            due.date = occurrence;
        }
        builder.task(Transition::updated(task.clone(), rolled));
    }
    Ok(builder.finish())
}

/// Moves a task to the trash, along with everything under it.
///
/// Trash is `deleted_at` on the record, not removal: it syncs, it is undoable, and Automerge
/// needs no tombstone to converge. Subtasks follow, because a subtask left behind
/// when its parent is trashed is unreachable in every view that shows a tree.
///
/// # Errors
///
/// If the task is not loaded.
pub fn trash_task(snapshot: &Snapshot, id: TaskId) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let stamp = time::now();
    let mut builder = Builder::new(format!("Deleted {}", task.title));

    for target in std::iter::once(task).chain(descendants(snapshot, id)) {
        if target.is_deleted() {
            continue;
        }
        let mut trashed = target.clone();
        trashed.deleted_at = Some(stamp);
        builder.task(Transition::updated(target.clone(), trashed));
    }
    Ok(builder.finish())
}

/// Takes a task back out of the trash, along with everything under it.
///
/// # Errors
///
/// If the task is not loaded.
pub fn restore_task(snapshot: &Snapshot, id: TaskId) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let mut builder = Builder::new(format!("Restored {}", task.title));

    for target in std::iter::once(task).chain(descendants(snapshot, id)) {
        if !target.is_deleted() {
            continue;
        }
        let mut restored = target.clone();
        restored.deleted_at = None;
        builder.task(Transition::updated(target.clone(), restored));
    }
    Ok(builder.finish())
}

/// Removes a task from the current state, as emptying the trash does. Its content remains in
/// the document's history and in backups.
///
/// # Errors
///
/// If the task is not loaded.
pub fn purge_task(snapshot: &Snapshot, id: TaskId) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let mut builder = Builder::new(format!("Deleted {} from the trash", task.title));

    for target in std::iter::once(task).chain(descendants(snapshot, id)) {
        for completion in snapshot.completions.values() {
            if completion.task_id == target.id {
                builder.completion(Transition::removed(completion.clone()));
            }
        }
        builder.task(Transition::removed(target.clone()));
    }
    Ok(builder.finish())
}

/// Makes one task wait for another.
///
/// # Errors
///
/// If either task is not loaded, or the edge would close a cycle of any length.
pub fn add_dependency(snapshot: &Snapshot, id: TaskId, on: TaskId) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let other = snapshot.tasks.get(&on).ok_or(EditError::NotFound { kind: "task" })?;
    if id == on || snapshot.waits_for(on, id) {
        return Err(EditError::DependencyCycle);
    }
    let mut after = task.clone();
    after.depends.insert(on);
    if &after == task {
        return Ok(Edit::nothing());
    }
    let mut builder = Builder::new(format!("{} now waits for {}", task.title, other.title));
    builder.task(Transition::updated(task.clone(), after));
    Ok(builder.finish())
}

/// Stops one task waiting for another.
///
/// # Errors
///
/// If the task is not loaded. The other one need not be: a dependency on a task this device
/// has never seen must still be removable.
pub fn remove_dependency(snapshot: &Snapshot, id: TaskId, on: TaskId) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let mut after = task.clone();
    if !after.depends.remove(&on) {
        return Ok(Edit::nothing());
    }
    let other = snapshot.tasks.get(&on).map_or("a task", |t| t.title.as_str());
    let mut builder = Builder::new(format!("{} no longer waits for {other}", task.title));
    builder.task(Transition::updated(task.clone(), after));
    Ok(builder.finish())
}

/// Where a task is being moved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveTo {
    /// A different project. Subtasks follow, since a subtask in another project from its
    /// parent has no view that can show both.
    Project(ProjectId),
    /// Under a different parent, or to the top level.
    Parent(Option<TaskId>),
    /// A different position among its current siblings.
    Between {
        /// The task it should follow, if any.
        after: Option<TaskId>,
        /// The task it should precede, if any.
        before: Option<TaskId>,
    },
}

/// Moves a task.
///
/// # Errors
///
/// If the task is not loaded, or the move would put it inside itself.
pub fn move_task(snapshot: &Snapshot, id: TaskId, to: MoveTo) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let mut builder = Builder::new(format!("Moved {}", task.title));

    match to {
        MoveTo::Project(project) => {
            if task.project_id != project {
                // A subtask whose parent stays behind would sit under a task in another
                // project, which no view can show, so it leaves its parent and lands at the
                // top of the new project.
                let detach = task
                    .parent_id
                    .and_then(|parent| snapshot.tasks.get(&parent))
                    .is_some_and(|parent| parent.project_id != project);
                for target in std::iter::once(task).chain(descendants(snapshot, id)) {
                    let mut moved = target.clone();
                    moved.project_id = project;
                    if target.id == id && detach {
                        moved.parent_id = None;
                    }
                    builder.task(Transition::updated(target.clone(), moved));
                }
            }
        }
        MoveTo::Parent(parent) => {
            if let Some(parent) = parent
                && (parent == id || descendants(snapshot, id).iter().any(|t| t.id == parent))
            {
                return Err(EditError::WouldCycle);
            }
            // Joining a parent means joining its project, subtasks and all, for the same
            // reason as above.
            let project = match parent {
                Some(parent) => {
                    snapshot.tasks.get(&parent).ok_or(EditError::NotFound { kind: "task" })?.project_id
                }
                None => task.project_id,
            };
            if task.parent_id != parent || task.project_id != project {
                let mut moved = task.clone();
                moved.parent_id = parent;
                moved.project_id = project;
                builder.task(Transition::updated(task.clone(), moved));
                if task.project_id != project {
                    for target in descendants(snapshot, id) {
                        let mut moved = target.clone();
                        moved.project_id = project;
                        builder.task(Transition::updated(target.clone(), moved));
                    }
                }
            }
        }
        MoveTo::Between { after, before } => {
            let lower = after.and_then(|id| snapshot.tasks.get(&id)).map(|t| t.order.clone());
            let upper = before.and_then(|id| snapshot.tasks.get(&id)).map(|t| t.order.clone());
            if let Ok(order) = OrderKey::between(lower.as_ref(), upper.as_ref())
                && order != task.order
            {
                let mut moved = task.clone();
                moved.order = order;
                builder.task(Transition::updated(task.clone(), moved));
            }
        }
    }
    Ok(builder.finish())
}

/// Every task beneath `root`, at any depth.
///
/// Walks with a visited set, so an unrepaired parent cycle yields a finite list
/// rather than hanging the caller.
fn descendants(snapshot: &Snapshot, root: TaskId) -> Vec<&Task> {
    let mut children: BTreeMap<TaskId, Vec<&Task>> = BTreeMap::new();
    for task in snapshot.tasks.values() {
        if let Some(parent) = task.parent_id {
            children.entry(parent).or_default().push(task);
        }
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::from([root]);
    let mut queue = vec![root];
    while let Some(id) = queue.pop() {
        for child in children.get(&id).into_iter().flatten() {
            if seen.insert(child.id) {
                out.push(*child);
                queue.push(child.id);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Projects and labels
// ---------------------------------------------------------------------------------------

/// Adds a project.
#[must_use]
pub fn create_project(project: Project) -> Edit {
    let description = format!("Added project {}", project.name);
    Edit {
        description,
        changes: vec![Change::Project(Box::new(Transition::created(project)))],
    }
}

/// Replaces a project with an edited copy.
#[must_use]
pub fn update_project(before: Project, after: Project) -> Edit {
    if before == after {
        return Edit::nothing();
    }
    Edit {
        description: format!("Edited project {}", after.name),
        changes: vec![Change::Project(Box::new(Transition::updated(before, after)))],
    }
}

/// What happens to a project's tasks when it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDeletion {
    /// Trash them with it. What Todoist does, and what someone deleting a finished project
    /// usually means.
    TrashTasks,
    /// Keep them, moved to the Inbox. What someone reorganising usually means.
    ///
    /// Worth offering rather than picking: the two intentions are indistinguishable from the
    /// keystroke, and guessing wrong silently loses a project's worth of work.
    MoveTasksToInbox,
}

/// Moves a project to the trash, along with its sub-projects.
///
/// # Errors
///
/// If the project is not loaded, or [`ProjectDeletion::MoveTasksToInbox`] was asked for and
/// there is no Inbox in the loaded documents.
pub fn trash_project(
    snapshot: &Snapshot,
    id: ProjectId,
    tasks: ProjectDeletion,
) -> Result<Edit, EditError> {
    let project = snapshot.projects.get(&id).ok_or(EditError::NotFound { kind: "project" })?;
    let stamp = time::now();
    let mut builder = Builder::new(format!("Deleted project {}", project.name));

    let mut affected = BTreeSet::from([id]);
    for candidate in snapshot.projects.values() {
        if snapshot.is_within(candidate.id, id) {
            affected.insert(candidate.id);
        }
    }

    let inbox = match tasks {
        ProjectDeletion::MoveTasksToInbox => {
            Some(snapshot.inbox().ok_or(EditError::NotFound { kind: "inbox" })?.id)
        }
        ProjectDeletion::TrashTasks => None,
    };

    for task in snapshot.tasks.values() {
        if !affected.contains(&task.project_id) {
            continue;
        }
        let mut changed = task.clone();
        match inbox {
            Some(inbox) => changed.project_id = inbox,
            None if task.is_deleted() => continue,
            None => changed.deleted_at = Some(stamp),
        }
        builder.task(Transition::updated(task.clone(), changed));
    }

    for project_id in affected {
        if let Some(project) = snapshot.projects.get(&project_id)
            && project.deleted_at.is_none()
        {
            let mut trashed = project.clone();
            trashed.deleted_at = Some(stamp);
            builder.changes.push(Change::Project(Box::new(Transition::updated(
                project.clone(),
                trashed,
            ))));
        }
    }
    Ok(builder.finish())
}

/// Adds a label.
#[must_use]
pub fn create_label(label: Label) -> Edit {
    let description = format!("Added label {}", label.name);
    Edit { description, changes: vec![Change::Label(Box::new(Transition::created(label)))] }
}

/// Replaces a label with an edited copy.
///
/// Renaming updates **one record**, and every task wearing it follows. With plain strings
/// this would be a rewrite of every task carrying it, which in a CRDT is a large
/// multi-object change where a concurrent edit can leave the rename half-applied.
#[must_use]
pub fn update_label(before: Label, after: Label) -> Edit {
    if before == after {
        return Edit::nothing();
    }
    Edit {
        description: format!("Renamed label {} to {}", before.name, after.name),
        changes: vec![Change::Label(Box::new(Transition::updated(before, after)))],
    }
}

/// Saves a filter query under a name.
///
/// The query is stored as **text, never as a resolved date range**. A filter
/// containing `today` has to mean today at evaluation time; resolving it at save time
/// produces one that silently rots overnight.
#[must_use]
pub fn create_filter(filter: SavedFilter) -> Edit {
    let description = format!("Saved filter {}", filter.name);
    Edit { description, changes: vec![Change::Filter(Box::new(Transition::created(filter)))] }
}

/// Moves a saved filter to the trash.
///
/// # Errors
///
/// If the filter is not loaded.
pub fn trash_filter(snapshot: &Snapshot, id: FilterId) -> Result<Edit, EditError> {
    let filter = snapshot
        .saved_filters
        .get(&id)
        .ok_or(EditError::NotFound { kind: "filter" })?;
    let mut trashed = filter.clone();
    trashed.deleted_at = Some(time::now());
    Ok(Edit {
        description: format!("Deleted filter {}", filter.name),
        changes: vec![Change::Filter(Box::new(Transition::updated(filter.clone(), trashed)))],
    })
}

/// Moves a label to the trash.
///
/// **Touches no tasks.** Identifiers left pointing at it project as absent, which
/// tolerating dangling references already requires — so undo is free and a large
/// multi-task write is avoided entirely. The consequence is worth remembering: a later
/// label with the same *name* is a different record, and old tasks do not acquire it.
///
/// # Errors
///
/// If the label is not loaded.
pub fn trash_label(snapshot: &Snapshot, id: LabelId) -> Result<Edit, EditError> {
    let label = snapshot.labels.get(&id).ok_or(EditError::NotFound { kind: "label" })?;
    let mut trashed = label.clone();
    trashed.deleted_at = Some(time::now());
    Ok(Edit {
        description: format!("Deleted label {}", label.name),
        changes: vec![Change::Label(Box::new(Transition::updated(label.clone(), trashed)))],
    })
}

/// Folds one label into another.
///
/// A first-class operation precisely because implicit creation makes near-duplicates
/// inevitable: type `@lapto` once and you have one. Merging rewrites the affected tasks'
/// sets and soft-deletes the loser — cheap with records, and impossible with plain strings,
/// where the two tags were never distinguishable from intent in the first place.
///
/// # Errors
///
/// If either label is not loaded.
pub fn merge_labels(snapshot: &Snapshot, from: LabelId, into: LabelId) -> Result<Edit, EditError> {
    let loser = snapshot.labels.get(&from).ok_or(EditError::NotFound { kind: "label" })?;
    let winner = snapshot.labels.get(&into).ok_or(EditError::NotFound { kind: "label" })?;
    if from == into {
        return Ok(Edit::nothing());
    }

    let mut builder =
        Builder::new(format!("Merged label {} into {}", loser.name, winner.name));
    for task in snapshot.tasks.values() {
        if !task.labels.contains(&from) {
            continue;
        }
        let mut changed = task.clone();
        changed.labels.remove(&from);
        changed.labels.insert(into);
        builder.task(Transition::updated(task.clone(), changed));
    }

    let mut trashed = loser.clone();
    trashed.deleted_at = Some(time::now());
    builder
        .changes
        .push(Change::Label(Box::new(Transition::updated(loser.clone(), trashed))));
    Ok(builder.finish())
}

// ---------------------------------------------------------------------------------------
// Blocks, assignments and timers — the join between the two halves
// ---------------------------------------------------------------------------------------

/// Adds a block series, one-off or recurring.
#[must_use]
pub fn create_series(series: BlockSeries) -> Edit {
    Edit {
        description: format!("Added block {}", series.title),
        changes: vec![Change::Series(Box::new(Transition::created(series)))],
    }
}

/// Moves a block series to the trash.
///
/// Its assignments are left alone, as a label's tasks are: identifiers pointing at a trashed
/// series project as absent, and restoring it brings them back with it.
///
/// # Errors
///
/// If the series is not loaded.
pub fn trash_series(snapshot: &Snapshot, id: crate::id::SeriesId) -> Result<Edit, EditError> {
    let series = snapshot.series.get(&id).ok_or(EditError::NotFound { kind: "block" })?;
    if series.deleted_at.is_some() {
        return Ok(Edit::nothing());
    }
    let mut trashed = series.clone();
    trashed.deleted_at = Some(time::now());
    Ok(Edit {
        description: format!("Deleted block {}", series.title),
        changes: vec![Change::Series(Box::new(Transition::updated(series.clone(), trashed)))],
    })
}

/// Changes a block for every occurrence: the "whole series" answer when a repeating block is
/// changed. A one-off block has only the one occurrence, so this is how it changes at all.
///
/// Exceptions already written for single occurrences keep their overrides; only the fields
/// they leave alone follow the series.
#[must_use]
pub fn update_series(before: BlockSeries, after: BlockSeries) -> Edit {
    if before == after {
        return Edit::nothing();
    }
    Edit {
        description: format!("Changed block {}", after.title),
        changes: vec![Change::Series(Box::new(Transition::updated(before, after)))],
    }
}

/// Changes one occurrence of a repeating block — the "this one only" answer — by writing
/// a sparse exception, rather than touching the series.
///
/// # Errors
///
/// If the series is not loaded, is a one-off (whose only occurrence is the block itself,
/// changed by [`update_series`]), or does not occur on `date`.
pub fn except_occurrence(
    snapshot: &Snapshot,
    series_id: crate::id::SeriesId,
    date: jiff::civil::Date,
    action: ExceptionAction,
) -> Result<Edit, EditError> {
    let series = snapshot.series.get(&series_id).ok_or(EditError::NotFound { kind: "block" })?;
    if !series.is_recurring() {
        return Err(EditError::NotRepeating);
    }
    let before = snapshot.exceptions.get(&(series_id, date)).cloned();
    // An occurrence already changed or cancelled is still one the rule placed there; a date
    // the rule never reaches is not an occurrence at all.
    let placed =
        before.is_some() || snapshot.day(date)?.iter().any(|o| o.series_id == series_id);
    if !placed {
        return Err(EditError::NoOccurrence { date });
    }
    let after = BlockException { series_id, original_date: date, action };
    if before.as_ref() == Some(&after) {
        return Ok(Edit::nothing());
    }
    let description = match &after.action {
        ExceptionAction::Cancelled => format!("Cancelled {} on {date}", series.title),
        ExceptionAction::Modified { .. } => format!("Changed {} on {date} only", series.title),
    };
    let transition = match before {
        Some(before) => Transition::updated(before, after),
        None => Transition::created(after),
    };
    Ok(Edit { description, changes: vec![Change::Exception(Box::new(transition))] })
}

/// Puts one occurrence of a repeating block back as the series has it, removing its
/// exception.
///
/// # Errors
///
/// If the series is not loaded.
pub fn restore_occurrence(
    snapshot: &Snapshot,
    series_id: crate::id::SeriesId,
    date: jiff::civil::Date,
) -> Result<Edit, EditError> {
    let series = snapshot.series.get(&series_id).ok_or(EditError::NotFound { kind: "block" })?;
    let Some(before) = snapshot.exceptions.get(&(series_id, date)).cloned() else {
        return Ok(Edit::nothing());
    };
    Ok(Edit {
        description: format!("Put {} on {date} back as the series has it", series.title),
        changes: vec![Change::Exception(Box::new(Transition::removed(before)))],
    })
}

/// Changes a saved filter's name or query. The query is stored as text, never resolved, so a
/// filter saying `today` keeps meaning today.
#[must_use]
pub fn update_filter(before: SavedFilter, after: SavedFilter) -> Edit {
    if before == after {
        return Edit::nothing();
    }
    Edit {
        description: format!("Changed filter {}", after.name),
        changes: vec![Change::Filter(Box::new(Transition::updated(before, after)))],
    }
}

/// Places a task into a block.
///
/// `year` says which `blocks-<year>` document it belongs in. It cannot be derived for a
/// one-off block, whose reference names only the series so that moving the block carries its
/// assignments with it.
///
/// There is deliberately **no uniqueness check** on task and date. Planning three sittings
/// for a long essay up front is a first-class use case, not an accident to prevent.
///
/// The occurrence is checked, though: it has to exist and take tasks. A sitting planned into
/// a day the block skips, or into a break, is invisible in every view of that day and would
/// read as planned work that silently never happened.
///
/// # Errors
///
/// If the task or block is not loaded, the task is in the trash, the block does not occur on
/// that day, or it takes no tasks.
pub fn assign_task(
    snapshot: &Snapshot,
    task_id: TaskId,
    block: BlockRef,
    year: i16,
) -> Result<Edit, EditError> {
    let task = snapshot.tasks.get(&task_id).ok_or(EditError::NotFound { kind: "task" })?;
    if task.is_deleted() {
        return Err(EditError::Trashed);
    }
    let occurrence = occurrence_of(snapshot, block)?;
    if !occurrence.flags.accepts_tasks {
        return Err(EditError::RefusesTasks);
    }

    let last = snapshot
        .assignments
        .values()
        .filter(|a| a.block_ref == block)
        .map(|a| a.order.clone())
        .max();
    let order = last.map_or_else(OrderKey::middle, |last| OrderKey::after(&last));

    Ok(Edit {
        description: format!("Scheduled {}", task.title),
        changes: vec![Change::Assignment {
            year,
            transition: Box::new(Transition::created(BlockAssignment::new(
                block, task_id, order,
            ))),
        }],
    })
}

/// The occurrence a block reference names, with its exception applied.
///
/// # Errors
///
/// If the series is not loaded or is trashed, the reference is the wrong shape for the
/// series — a date on a one-off, or none on a repeating block — or the series does not
/// occur on that day.
pub fn occurrence_of(snapshot: &Snapshot, block: BlockRef) -> Result<recur::Occurrence, EditError> {
    let series = snapshot
        .series
        .get(&block.series_id())
        .filter(|s| s.deleted_at.is_none())
        .ok_or(EditError::NotFound { kind: "block" })?;
    let date = block.date().unwrap_or(series.start_date);
    recur::expand(
        series,
        |day| snapshot.exceptions.get(&(series.id, day)).map(|e| &e.action),
        &(date..=date),
    )?
    .into_iter()
    .find(|occurrence| occurrence.block_ref(series) == block)
    .ok_or(EditError::NoOccurrence { date })
}

/// Takes a task back out of a block.
///
/// # Errors
///
/// If the assignment is not loaded.
pub fn unassign(
    snapshot: &Snapshot,
    assignment_id: AssignmentId,
    year: i16,
) -> Result<Edit, EditError> {
    let assignment = snapshot
        .assignments
        .get(&assignment_id)
        .ok_or(EditError::NotFound { kind: "assignment" })?;
    let title = snapshot
        .tasks
        .get(&assignment.task_id)
        .map_or("task", |t| t.title.as_str())
        .to_owned();
    Ok(Edit {
        description: format!("Unscheduled {title}"),
        changes: vec![Change::Assignment {
            year,
            transition: Box::new(Transition::removed(assignment.clone())),
        }],
    })
}

/// Starts the timer on an assignment.
///
/// Writes a fact — when it started — rather than a counter, so nothing ticks in storage and
/// two devices starting the same timer is a harmless last-write-wins on one timestamp.
///
/// # Errors
///
/// If the assignment is not loaded.
pub fn start_timer(
    snapshot: &Snapshot,
    assignment_id: AssignmentId,
    year: i16,
    now: &Zoned,
) -> Result<Edit, EditError> {
    let assignment = snapshot
        .assignments
        .get(&assignment_id)
        .ok_or(EditError::NotFound { kind: "assignment" })?;
    if assignment.is_running() {
        return Ok(Edit::nothing());
    }
    let mut started = assignment.clone();
    started.start(time::truncate(now.timestamp()));
    Ok(Edit {
        description: "Started timer".to_owned(),
        changes: vec![Change::Assignment {
            year,
            transition: Box::new(Transition::updated(assignment.clone(), started)),
        }],
    })
}

/// Stops the timer, folding the running interval into the accumulated total.
///
/// `cap_mins` should be the containing occurrence's duration. A timer left running past the
/// end of its block — started on a phone that then died — would otherwise record the
/// wall-clock time since, so the running interval is capped and the caller is told.
/// **Confirm with the user rather than recording the capped figure silently**; a truncated
/// number presented as fact is its own kind of wrong, and [`log_minutes`] is how the user's
/// own figure replaces it.
///
/// A timer that is not running produces [`Edit::nothing`], with the time already logged.
///
/// # Errors
///
/// If the assignment is not loaded.
pub fn stop_timer(
    snapshot: &Snapshot,
    assignment_id: AssignmentId,
    year: i16,
    cap_mins: Option<u32>,
    now: &Zoned,
) -> Result<(Edit, crate::model::Elapsed), EditError> {
    let assignment = snapshot
        .assignments
        .get(&assignment_id)
        .ok_or(EditError::NotFound { kind: "assignment" })?;
    if !assignment.is_running() {
        let logged = crate::model::Elapsed { mins: assignment.accumulated_mins, capped: false };
        // A paused sitting is still in progress; stopping it is what ends it.
        if assignment.status != crate::model::AssignmentStatus::InProgress {
            return Ok((Edit::nothing(), logged));
        }
        let mut ended = assignment.clone();
        ended.status = crate::model::AssignmentStatus::Worked;
        return Ok((
            Edit {
                description: format!("Logged {} minutes", logged.mins),
                changes: vec![Change::Assignment {
                    year,
                    transition: Box::new(Transition::updated(assignment.clone(), ended)),
                }],
            },
            logged,
        ));
    }
    let mut stopped = assignment.clone();
    let elapsed = stopped.pause(time::truncate(now.timestamp()), cap_mins);
    if stopped.status == crate::model::AssignmentStatus::InProgress {
        stopped.status = crate::model::AssignmentStatus::Worked;
    }
    Ok((
        Edit {
            description: format!("Logged {} minutes", elapsed.mins),
            changes: vec![Change::Assignment {
                year,
                transition: Box::new(Transition::updated(assignment.clone(), stopped)),
            }],
        },
        elapsed,
    ))
}

/// Pauses the timer: the running interval is folded into the accumulated total, as for
/// [`stop_timer`], but the sitting stays in progress, to be resumed with [`start_timer`] or
/// ended with [`stop_timer`].
///
/// A timer that is not running produces [`Edit::nothing`], with the time already logged.
///
/// # Errors
///
/// If the assignment is not loaded.
pub fn pause_timer(
    snapshot: &Snapshot,
    assignment_id: AssignmentId,
    year: i16,
    cap_mins: Option<u32>,
    now: &Zoned,
) -> Result<(Edit, crate::model::Elapsed), EditError> {
    let assignment = snapshot
        .assignments
        .get(&assignment_id)
        .ok_or(EditError::NotFound { kind: "assignment" })?;
    if !assignment.is_running() {
        let logged = crate::model::Elapsed { mins: assignment.accumulated_mins, capped: false };
        return Ok((Edit::nothing(), logged));
    }
    let mut paused = assignment.clone();
    let elapsed = paused.pause(time::truncate(now.timestamp()), cap_mins);
    Ok((
        Edit {
            description: format!("Paused timer, {} minutes so far", elapsed.mins),
            changes: vec![Change::Assignment {
                year,
                transition: Box::new(Transition::updated(assignment.clone(), paused)),
            }],
        },
        elapsed,
    ))
}

/// Sets how long a sitting took, by hand.
///
/// The timer is optional, so this is the other way time gets recorded — and the way
/// a capped figure from an orphaned timer is put right. A running timer stops, since the
/// figure given is the whole of the sitting.
///
/// # Errors
///
/// If the assignment is not loaded.
pub fn log_minutes(
    snapshot: &Snapshot,
    assignment_id: AssignmentId,
    year: i16,
    mins: u32,
) -> Result<Edit, EditError> {
    let assignment = snapshot
        .assignments
        .get(&assignment_id)
        .ok_or(EditError::NotFound { kind: "assignment" })?;
    let mut logged = assignment.clone();
    logged.accumulated_mins = mins;
    logged.running_since = None;
    if mins > 0
        && matches!(
            logged.status,
            crate::model::AssignmentStatus::Planned | crate::model::AssignmentStatus::InProgress
        )
    {
        logged.status = crate::model::AssignmentStatus::Worked;
    }
    if &logged == assignment {
        return Ok(Edit::nothing());
    }
    Ok(Edit {
        description: format!("Logged {mins} minutes"),
        changes: vec![Change::Assignment {
            year,
            transition: Box::new(Transition::updated(assignment.clone(), logged)),
        }],
    })
}

/// Sets how long a sitting is meant to take, or clears it.
///
/// What was planned, not what was done: the logged minutes and the status are left alone, so
/// changing the plan halfway through a sitting does not rewrite what already happened.
///
/// # Errors
///
/// If the assignment is not loaded.
pub fn plan_minutes(
    snapshot: &Snapshot,
    assignment_id: AssignmentId,
    year: i16,
    mins: Option<u32>,
) -> Result<Edit, EditError> {
    let assignment = snapshot
        .assignments
        .get(&assignment_id)
        .ok_or(EditError::NotFound { kind: "assignment" })?;
    if assignment.planned_mins == mins {
        return Ok(Edit::nothing());
    }
    let mut planned = assignment.clone();
    planned.planned_mins = mins;
    Ok(Edit {
        description: match mins {
            Some(mins) => format!("Planned {mins} minutes"),
            None => "Cleared the planned length".to_owned(),
        },
        changes: vec![Change::Assignment {
            year,
            transition: Box::new(Transition::updated(assignment.clone(), planned)),
        }],
    })
}

// ---------------------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------------------

/// Adds devices to the roster, or brings their records up to date — what pairing writes.
///
/// Membership in `devices` is the trust boundary: a device listed there is one every
/// other device will sync with. So this is only ever called once the words have been compared
/// and confirmed on both sides; nothing learned any other way belongs here.
#[must_use]
pub fn enroll_devices(snapshot: &Snapshot, devices: &[Device]) -> Edit {
    let mut changes = Vec::new();
    for device in devices {
        let before = snapshot.devices.get(&device.node_id);
        // Keep the original pairing time; it is history, not something a re-pair rewrites.
        let after = match before {
            Some(existing) => Device { paired_at: existing.paired_at, ..device.clone() },
            None => device.clone(),
        };
        if before != Some(&after) {
            changes.push(Change::Device(Box::new(Transition {
                before: before.cloned(),
                after: Some(after),
            })));
        }
    }
    let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    Edit { description: format!("Paired {}", names.join(" and ")), changes }
}

/// Renames a device.
///
/// # Errors
///
/// If it is not in the roster.
pub fn rename_device(
    snapshot: &Snapshot,
    id: crate::id::NodeId,
    name: &str,
) -> Result<Edit, EditError> {
    let before = snapshot.devices.get(&id).ok_or(EditError::NotFound { kind: "device" })?;
    if before.name == name {
        return Ok(Edit::nothing());
    }
    let after = Device { name: name.to_owned(), ..before.clone() };
    Ok(Edit {
        description: format!("Renamed {} to {name}", before.name),
        changes: vec![Change::Device(Box::new(Transition::updated(before.clone(), after)))],
    })
}

/// Takes a device out of the roster, so the others stop syncing with it.
///
/// **Not revocation**: the device keeps everything it already holds, and nothing in a
/// CRDT stops a malicious one writing itself back. Unpair a device that was replaced; one that
/// was stolen needs the account key rotated, which is a different and much larger thing.
///
/// # Errors
///
/// If it is not in the roster.
pub fn unpair_device(snapshot: &Snapshot, id: crate::id::NodeId) -> Result<Edit, EditError> {
    let before = snapshot.devices.get(&id).ok_or(EditError::NotFound { kind: "device" })?;
    Ok(Edit {
        description: format!("Unpaired {}", before.name),
        changes: vec![Change::Device(Box::new(Transition::removed(before.clone())))],
    })
}

// ---------------------------------------------------------------------------------------
// The Inbox
// ---------------------------------------------------------------------------------------

/// Folds any Inbox other than the canonical one into it.
///
/// [`ProjectId::INBOX`](crate::id::ProjectId::INBOX) is the same on every device, so two
/// devices cannot bring an Inbox each to a merge. A store older than that identifier may have
/// one of its own: its tasks move to the canonical Inbox, and it stops being an Inbox and goes to
/// the trash, which keeps the step undoable.
///
/// Nothing to do — the usual case — is [`Edit::nothing`]. So is a store whose canonical
/// Inbox is not loaded, since there would be nowhere to move anything to.
#[must_use]
pub fn adopt_inbox(snapshot: &Snapshot) -> Edit {
    let canonical = crate::id::ProjectId::INBOX;
    if snapshot.projects.get(&canonical).is_none_or(|p| p.deleted_at.is_some()) {
        return Edit::nothing();
    }
    let legacy: BTreeSet<ProjectId> = snapshot
        .projects
        .values()
        .filter(|p| p.is_inbox && p.id != canonical)
        .map(|p| p.id)
        .collect();
    if legacy.is_empty() {
        return Edit::nothing();
    }

    let stamp = time::now();
    let mut builder = Builder::new("Merged the Inbox");
    for task in snapshot.tasks.values().filter(|t| legacy.contains(&t.project_id)) {
        let mut moved = task.clone();
        moved.project_id = canonical;
        builder.task(Transition::updated(task.clone(), moved));
    }
    for id in &legacy {
        let before = &snapshot.projects[id];
        let mut retired = before.clone();
        retired.is_inbox = false;
        if retired.deleted_at.is_none() {
            retired.deleted_at = Some(stamp);
        }
        builder.changes.push(Change::Project(Box::new(Transition::updated(
            before.clone(),
            retired,
        ))));
    }
    builder.finish()
}

// ---------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------

/// Changes settings.
#[must_use]
pub fn update_settings(before: Settings, after: Settings) -> Edit {
    if before == after {
        return Edit::nothing();
    }
    Edit {
        description: "Changed settings".to_owned(),
        changes: vec![Change::Settings { before: Box::new(before), after: Box::new(after) }],
    }
}
