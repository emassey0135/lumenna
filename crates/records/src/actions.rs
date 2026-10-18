//! What can be done to each thing a client lists, decided once.
//!
//! Every record a client shows as a row carries its `actions`: which apply now, in the
//! order offered, under their spoken names ("Delete Block", not "Delete"), each with the
//! question it asks before it runs. A client turns them into swipe actions, a context menu,
//! buttons or a key's command, asks the question its own way, and hands the action back with
//! the answer to `Lumenna::act`. It decides none of it: eleven clients deciding which
//! actions a row has, and what deleting asks, had drifted eleven ways.
//!
//! What a picker offers is the core's too (`Lumenna::choices`), so a task is never offered
//! as its own parent, and a project never moved inside itself.
//!
//! Only a [`Question::Form`] is the client's: the task and block forms are its own screens,
//! built on `task_fields` and `block_fields`.

use std::collections::BTreeSet;

use lumenna_core::model::{BlockSeries, Label, Project, SavedFilter, Task};
use lumenna_core::row::RowId;
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::types::{CancelledBlock, PlanAssignment, PlanBlock, PlanItem, Rows};

/// What an action does. Generic across what it is done to, so a key can mean one thing
/// everywhere: Delete is [`ActionKind::Delete`] on whatever row has focus.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    /// Complete a task.
    MarkDone,
    /// Take a completion back.
    MarkNotDone,
    /// Open the form: a task's, a block's.
    Edit,
    /// Open the form of the task a sitting is for.
    EditTask,
    /// Put a task in a work block.
    PutInBlock,
    /// Move a task to another project.
    MoveToProject,
    /// Put a task under another.
    MakeSubtaskOf,
    /// Take a task or project out from under its parent.
    MoveToTopLevel,
    /// Make a task wait for another.
    WaitFor,
    /// Stop a task waiting for one it waits for ([`Action::other`]).
    StopWaiting,
    /// Trash a task; delete a block, project, label or saved filter.
    Delete,
    /// Bring a task back from the trash.
    Restore,
    /// Erase a trashed task.
    DeleteForGood,
    /// Put a task in this block.
    AssignTask,
    /// Skip one day of a repeating block.
    CancelDay,
    /// Put one day of a repeating block back as the series has it.
    RestoreDay,
    /// Start a sitting's timer.
    StartTimer,
    /// Start a paused sitting's timer again.
    ResumeTimer,
    /// Pause a sitting's timer.
    PauseTimer,
    /// End a sitting, logging its time.
    StopTimer,
    /// Set how long a sitting is meant to take.
    PlannedLength,
    /// Set the whole of a sitting's time by hand.
    LogMinutes,
    /// Take a task out of a block.
    Unassign,
    /// Add a block in free time.
    AddBlock,
    /// Rename a project, label, saved filter or device.
    Rename,
    /// Add a project inside this one.
    NewInside,
    /// Move a project under another.
    MoveUnder,
    /// Move up among its siblings.
    MoveUp,
    /// Move down among its siblings.
    MoveDown,
    /// Set a project's weight.
    Weight,
    /// Archive a project.
    Archive,
    /// Unarchive a project.
    Unarchive,
    /// Merge a label into another.
    MergeInto,
    /// Set a label's colour.
    Colour,
    /// Change a saved filter's query.
    ChangeQuery,
    /// Add a project, label or saved filter, from its heading.
    New,
    /// Stop syncing with a device.
    Unpair,
    /// Open the app's list of a task's or block's reminders.
    Reminders,
    /// End the block under way now.
    EndNow,
    /// Give the block under way more time.
    Extend,
    /// End the block under way and clear the rest of the day.
    EndDay,
    /// Clear the rest of the day, leaving the block under way.
    ClearDay,
    /// Put the rest of a sitting's planned time in another block.
    CarryForward,
    /// Asked of today: what to work on now, and starting on it.
    WhatNow,
    /// Carry the rest of every sitting a block left short to the next block that takes it.
    CarryRest,
    /// Replan the rest of today: the app's own screen over `Lumenna::replan`.
    Replan,
    /// Plan a whole day: the app's own screen over `Lumenna::propose_day_plan`.
    PlanDay,
    /// Show what is under a row folded away: the app's, which changes nothing stored.
    Expand,
    /// Fold away what is under a row.
    Collapse,
}

/// What an action is done to: with [`ActionKind`], what runs.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum Subject {
    /// A task, by identifier.
    Task,
    /// One day of a block, `<series>@<date>`.
    Block,
    /// A block series, as the Blocks list shows it, by identifier.
    Series,
    /// A sitting, by assignment identifier.
    Sitting,
    /// Free time on a day: the day in `target`, its start (`HH:MM`) in `other`.
    FreeTime,
    /// A project, by name.
    Project,
    /// A label, by name.
    Label,
    /// A saved filter, by name.
    Filter,
    /// A paired device, by key.
    Device,
    /// A reminder of a task's or block's own, by identifier; for New, the task or block it
    /// is for.
    Reminder,
    /// A day of the plan, as an ISO date.
    Day,
}

/// One thing that can be done, and what it asks first.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Action {
    /// What it does.
    pub kind: ActionKind,
    /// What it is done to.
    pub subject: Subject,
    /// Its spoken name: "Mark Done", "Delete Block".
    pub title: String,
    /// The record it acts on (see [`Subject`]). Empty for a heading's New.
    pub target: String,
    /// A second record, where it takes one: the task Stop Waiting is about, free time's start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other: Option<String>,
    /// The same in sentence case ("Move to trash"), for a platform whose convention it is
    /// and for braille. Names in it keep their own capitals.
    #[serde(default)]
    pub sentence: String,
    /// Whether it removes something, so is shown as such.
    #[serde(default)]
    pub destructive: bool,
    /// Whether it is one of the two or three a row shows by itself — the iPhone's swipe
    /// actions, a watch row's — with the rest a menu away. Everything is still offered.
    #[serde(default)]
    pub primary: bool,
    /// What it asks before it runs.
    pub question: Question,
}

/// What an action asks before it runs, which the client asks its own way.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "ask", rename_all = "snake_case")]
pub enum Question {
    /// Nothing: it runs at once, answered [`Answer::Yes`].
    Immediate,
    /// The client's own form, which `Lumenna::act` does not run.
    Form,
    /// Whether to go ahead, for what cannot be put right by moving it back.
    Confirm {
        /// The question: "Delete Deep work?"
        title: String,
        /// What going ahead does.
        message: String,
        /// The button that goes ahead: "Delete Block". Cancel is the client's, and first.
        yes: String,
    },
    /// A line of text.
    Text {
        /// The dialog's title: "Rename Work".
        title: String,
        /// The field's name: "Name".
        label: String,
        /// What the field starts with.
        initial: String,
        /// What it takes, for under the field. Empty for nothing to say.
        hint: String,
        /// Whether an empty answer means "none" rather than going unanswered.
        optional: bool,
        /// The button that answers: "Rename", "Add", "Save". Cancel is the client's.
        yes: String,
        /// The title in sentence case, for a platform whose convention it is: names in it keep
        /// their capitals.
        #[serde(default)]
        sentence: String,
    },
    /// One of what `Lumenna::choices` offers for this action.
    Pick {
        /// The chooser's title: "Move Write report to".
        title: String,
        /// A second question once one is picked: how long the sitting is meant to take, in
        /// [`Answer::Picked::length`], optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<String>,
        /// The button that answers: "Move", "Assign", "Merge". Cancel is the client's.
        #[serde(default)]
        yes: String,
    },
    /// One of a few answers, each its own button, answered [`Answer::Picked`] by its `id`.
    Choose {
        /// The question.
        title: String,
        /// What the answers decide.
        message: String,
        /// The answers, in order. Cancel is the client's.
        answers: Vec<Choice>,
    },
}

/// Something to choose.
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Choice {
    /// What [`Answer::Picked`] sends back.
    pub id: String,
    /// What it is called.
    pub title: String,
    /// How deep it sits in a tree, for a chooser that shows one.
    #[serde(default)]
    pub depth: u32,
    /// What else tells it apart: a task's project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// A block's day, ISO, for the client to say in its own words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// A block's start, `HH:MM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// A block's end, `HH:MM`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// Why it is offered where it is, for its description: "Suggested because overdue by
    /// 2 days, priority 1, and its estimate of 30 minutes fits the 35 minutes left."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "uniffi", uniffi(default = None))]
    pub reason: Option<String>,
}

/// What a picker offers.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct Choices {
    /// How many there are; or, when there are none, why, which is all a client says.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// What to choose from, in order.
    pub choices: Vec<Choice>,
}

crate::announced!(Choices);

/// The answer to an action's question.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "answer", rename_all = "snake_case")]
pub enum Answer {
    /// Go ahead: an immediate action, or a confirmation.
    Yes,
    /// A line typed.
    Text {
        /// What was typed.
        text: String,
    },
    /// A choice picked.
    Picked {
        /// Its `id`.
        id: String,
        /// The answer to [`Question::Pick::length`], if it was asked.
        #[serde(default)]
        length: Option<String>,
    },
}

// ---------------------------------------------------------------------------------------
// The words
// ---------------------------------------------------------------------------------------

/// What erasing asks: it is gone from here, and not from history.
pub const ERASING: &str =
    "Undo can bring it back. It also stays in the history every device keeps, and in backups.";
/// What unpairing asks: what it does, and what it does not.
pub const UNPAIRING: &str = "It stops syncing with your devices but keeps everything it already has. \
                             Unpairing is for a device you replaced; it does not take data back from a lost one.";
/// What logging minutes takes.
pub const LOGGING: &str = "The whole of this sitting, replacing what is logged.";
/// How long a sitting is meant to take.
pub const LENGTH: &str = "How long is it meant to take? Such as 45m or 1h 30m; empty for no planned length.";
/// What a label's colour takes.
pub const COLOUR: &str = "A colour name, such as red or teal, or none. The name always shows too.";
/// What a project's weight takes.
pub const WEIGHT: &str =
    "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.";
/// What a saved filter's query takes.
pub const QUERY: &str = "A filter, such as p1 & due before: friday.";

fn action(kind: ActionKind, subject: Subject, title: impl Into<String>, target: impl Into<String>) -> Action {
    let title = title.into();
    Action {
        kind,
        subject,
        sentence: crate::fields::sentence_case(title.clone()),
        primary: is_primary(kind, subject),
        title,
        target: target.into(),
        other: None,
        destructive: matches!(kind, ActionKind::Delete | ActionKind::DeleteForGood | ActionKind::Unassign | ActionKind::Unpair),
        question: if matches!(
            kind,
            ActionKind::Edit
                | ActionKind::EditTask
                | ActionKind::AddBlock
                | ActionKind::Reminders
                | ActionKind::Replan
                | ActionKind::PlanDay
        ) {
            Question::Form
        } else {
            Question::Immediate
        },
    }
}

/// The two or three a row shows by itself: what is done to it most, and what removes it.
const fn is_primary(kind: ActionKind, subject: Subject) -> bool {
    use ActionKind as K;
    match kind {
        K::MarkDone | K::MarkNotDone | K::Restore | K::DeleteForGood | K::Delete | K::AssignTask | K::StartTimer
        | K::PauseTimer | K::ResumeTimer | K::StopTimer | K::AddBlock | K::RestoreDay | K::New | K::Unpair => true,
        K::Rename => matches!(subject, Subject::Project | Subject::Label | Subject::Filter | Subject::Device),
        _ => false,
    }
}

impl Action {
    /// An action with no question yet, for the modules that build their own.
    pub fn new(kind: ActionKind, subject: Subject, title: &str, target: String) -> Self {
        action(kind, subject, title, target)
    }

    fn asking(mut self, question: Question) -> Self {
        self.question = question;
        self
    }

    fn with(mut self, other: impl Into<String>) -> Self {
        self.other = Some(other.into());
        self
    }
}

fn text(title: impl Into<String>, label: &str, initial: impl Into<String>, hint: &str, optional: bool) -> Question {
    let title = title.into();
    // The button says what answering does: the verb that starts the title, else Save.
    let yes = if title.starts_with("Rename") {
        "Rename"
    } else if title.starts_with("New") {
        "Add"
    } else if label == "Minutes" {
        "Log"
    } else {
        "Save"
    };
    Question::Text {
        sentence: title.clone(),
        title,
        label: label.to_owned(),
        initial: initial.into(),
        hint: hint.to_owned(),
        optional,
        yes: yes.to_owned(),
    }
}

fn confirm(title: impl Into<String>, message: &str, yes: &str) -> Question {
    Question::Confirm { title: title.into(), message: message.to_owned(), yes: yes.to_owned() }
}

fn pick(title: impl Into<String>, yes: &str) -> Question {
    Question::Pick { title: title.into(), length: None, yes: yes.to_owned() }
}

fn minutes_text(minutes: u32) -> String {
    format!("{minutes}m")
}

// ---------------------------------------------------------------------------------------
// Which apply
// ---------------------------------------------------------------------------------------

/// A task's, wherever it is listed or shown. `shown` leaves out Edit Details, for the
/// task's own screen, which is that form.
pub fn task(snapshot: &Snapshot, task: &Task, done: bool, shown: bool) -> Vec<Action> {
    use ActionKind as K;
    let id = task.id.to_string();
    let a = |kind, title: &str| action(kind, Subject::Task, title, id.clone());
    if task.is_deleted() {
        return vec![
            a(K::Restore, "Restore"),
            a(K::DeleteForGood, "Delete from Trash").asking(confirm(
                format!("Delete {} from the trash?", task.title),
                ERASING,
                "Delete",
            )),
        ];
    }
    let mut actions = vec![if done { a(K::MarkNotDone, "Mark Not Done") } else { a(K::MarkDone, "Mark Done") }];
    if !shown {
        actions.push(a(K::Edit, "Edit Details"));
    }
    actions.push(a(K::PutInBlock, "Put in a Block").asking(Question::Pick {
        title: format!("Put {} in a block", task.title),
        length: Some(LENGTH.to_owned()),
        yes: "Put It There".to_owned(),
    }));
    actions.push(a(K::MoveToProject, "Move to Project").asking(pick(format!("Move {} to", task.title), "Move")));
    actions.push(a(K::MakeSubtaskOf, "Make Subtask Of").asking(pick(format!("Make {} a subtask of", task.title), "Move")));
    if task.parent_id.is_some() {
        actions.push(a(K::MoveToTopLevel, "Move to Top Level"));
    }
    actions.push(a(K::WaitFor, "Wait For").asking(pick(format!("What does {} wait for?", task.title), "Wait For It")));
    for on in &task.depends {
        let title = snapshot.tasks.get(on).map_or("a task not loaded here", |t| t.title.as_str());
        let mut stop = a(K::StopWaiting, &format!("Stop Waiting for {title}")).with(on.to_string());
        stop.sentence = format!("Stop waiting for {title}");
        actions.push(stop);
    }
    // Only a task with a due date has something for a reminder to count from.
    if task.due.is_some() {
        actions.push(a(K::Reminders, "Reminders"));
    }
    actions.push(a(K::Delete, "Move to Trash"));
    actions
}

/// A block's, on its day: tasks go in one that takes them, a repeating one can skip a day,
/// and a day changed apart from its series can go back to it.
#[must_use]
pub fn block(block: &PlanBlock) -> Vec<Action> {
    use ActionKind as K;
    let a = |kind, title: &str| action(kind, Subject::Block, title, block.id.clone());
    let mut actions = Vec::new();
    if block.accepts_tasks {
        actions.push(a(K::AssignTask, "Assign a Task").asking(Question::Pick {
            title: format!("Assign a task to {}", block.title),
            length: Some(LENGTH.to_owned()),
            yes: "Assign".to_owned(),
        }));
    }
    actions.push(a(K::Edit, "Edit Block"));
    // The block under way can end now or run longer: a modified day, the person's own call.
    if block.when == "now" {
        actions.push(a(K::EndNow, "End Now"));
        actions.push(a(K::Extend, &format!("Add {} Minutes", crate::day::EXTEND_MINS)));
    }
    if block.repeats {
        actions.push(a(K::CancelDay, "Cancel This Day"));
    }
    if block.changed_for_this_day {
        actions.push(a(K::RestoreDay, "Restore This Day"));
    }
    actions.push(action(K::Reminders, Subject::Series, "Reminders", block.series.clone()));
    actions.push(a(K::Delete, "Delete Block").asking(deleting_block(&block.title, block.repeats)));
    actions
}

fn deleting_block(title: &str, repeats: bool) -> Question {
    confirm(
        format!("Delete {title}?"),
        if repeats {
            "Every occurrence goes, not only this day. To skip one day, cancel it instead."
        } else {
            "It goes to the trash with its assignments."
        },
        "Delete Block",
    )
}

/// A sitting's: start, pause or resume, and stop — a paused sitting is still in progress,
/// and stopping a running or a paused one ends it — then its length, its time, its task.
#[must_use]
pub fn sitting(sitting: &PlanAssignment) -> Vec<Action> {
    use ActionKind as K;
    let a = |kind, title: &str| action(kind, Subject::Sitting, title, sitting.id.clone());
    let paused = sitting.status == "paused";
    let mut actions = vec![if sitting.running {
        a(K::PauseTimer, "Pause Timer")
    } else if paused {
        a(K::ResumeTimer, "Resume Timer")
    } else {
        a(K::StartTimer, "Start Timer")
    }];
    if sitting.running || paused {
        actions.push(a(K::StopTimer, "Stop Timer"));
    }
    actions.push(a(K::PlannedLength, "Planned Length").asking(text(
        format!("Planned length of {}", sitting.title),
        "Planned length",
        sitting.planned_mins.map(minutes_text).unwrap_or_default(),
        LENGTH,
        true,
    )));
    actions.push(a(K::LogMinutes, "Log Minutes").asking(text(
        format!("Minutes on {}", sitting.title),
        "Minutes",
        minutes_text(sitting.minutes),
        LOGGING,
        false,
    )));
    actions.push(action(K::EditTask, Subject::Task, "Edit Task Details", sitting.task.clone()));
    actions.push(a(K::Unassign, "Unassign"));
    actions
}

/// Free time's, on `date` from `start`.
#[must_use]
pub fn free_time(date: &str, start: &str) -> Vec<Action> {
    vec![action(ActionKind::AddBlock, Subject::FreeTime, "Add Block Here", date).with(start)]
}

/// A day cancelled from a repeating block's.
#[must_use]
pub fn cancelled(block: &CancelledBlock, date: &str) -> Vec<Action> {
    vec![action(ActionKind::RestoreDay, Subject::Block, "Restore This Day", format!("{}@{date}", block.series))]
}

/// A block series' own, as the Blocks list has it.
pub fn series(series: &BlockSeries) -> Vec<Action> {
    let id = series.id.to_string();
    vec![
        action(ActionKind::Edit, Subject::Series, "Edit Block", id.clone()),
        action(ActionKind::Reminders, Subject::Series, "Reminders", id.clone()),
        action(ActionKind::Delete, Subject::Series, "Delete Block", id)
            .asking(deleting_block(&series.title, series.is_recurring())),
    ]
}

/// Up and down where there is somewhere to go.
fn reorder(actions: &mut Vec<Action>, subject: Subject, target: &str, position: usize, count: usize) {
    if position > 0 {
        actions.push(action(ActionKind::MoveUp, subject, "Move Up", target));
    }
    if position + 1 < count {
        actions.push(action(ActionKind::MoveDown, subject, "Move Down", target));
    }
}

fn position_of<T: PartialEq>(siblings: &[T], item: &T) -> usize {
    siblings.iter().position(|s| s == item).unwrap_or(0)
}

/// A project's. The Inbox keeps its name and place in the tree, so it is only reordered
/// and weighed.
pub fn project(snapshot: &Snapshot, project: &Project) -> Vec<Action> {
    use ActionKind as K;
    let name = project.name.clone();
    let a = |kind, title: &str| action(kind, Subject::Project, title, name.clone());
    let mut siblings: Vec<&Project> = snapshot
        .projects
        .values()
        .filter(|p| p.deleted_at.is_none() && p.parent_id == project.parent_id)
        .collect();
    siblings.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
    let ids: Vec<_> = siblings.iter().map(|p| p.id).collect();
    let weight = a(K::Weight, "Weight").asking(text(
        format!("Weight of {name}"),
        "Weight",
        match (project.weight, project.parent_id) {
            (Some(weight), _) => format!("{weight}"),
            (None, Some(_)) => "inherit".to_owned(),
            (None, None) => format!("{}", Project::NEUTRAL_WEIGHT),
        },
        WEIGHT,
        false,
    ));
    let mut actions = Vec::new();
    if project.is_inbox {
        reorder(&mut actions, Subject::Project, &name, position_of(&ids, &project.id), ids.len());
        actions.push(weight);
        return actions;
    }
    actions.push(a(K::Rename, "Rename").asking(text(format!("Rename {name}"), "Name", name.clone(), "", false)));
    actions.push(a(K::NewInside, "New Project Inside").asking(text(format!("New project inside {name}"), "Name", "", "", false)));
    actions.push(a(K::MoveUnder, "Move Under").asking(pick(format!("Move {name} under"), "Move")));
    if project.parent_id.is_some() {
        actions.push(a(K::MoveToTopLevel, "Move to Top Level"));
    }
    reorder(&mut actions, Subject::Project, &name, position_of(&ids, &project.id), ids.len());
    actions.push(weight);
    actions.push(if project.archived { a(K::Unarchive, "Unarchive") } else { a(K::Archive, "Archive") });
    actions.push(a(K::Delete, "Delete").asking(Question::Choose {
        title: format!("Delete {name}?"),
        message: "Its tasks can go to the trash with it, or move to the Inbox.".to_owned(),
        answers: vec![
            Choice { id: "trash".to_owned(), title: "Delete and Trash Its Tasks".to_owned(), ..Choice::default() },
            Choice { id: "keep".to_owned(), title: "Delete and Keep Its Tasks".to_owned(), ..Choice::default() },
        ],
    }));
    actions
}

/// A label's.
pub fn label(snapshot: &Snapshot, label: &Label) -> Vec<Action> {
    use ActionKind as K;
    let name = label.name.clone();
    let a = |kind, title: &str| action(kind, Subject::Label, title, name.clone());
    let mut live: Vec<&Label> = snapshot.labels.values().filter(|l| l.deleted_at.is_none()).collect();
    live.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
    let ids: Vec<_> = live.iter().map(|l| l.id).collect();
    let mut actions = vec![
        a(K::Rename, "Rename").asking(text(format!("Rename {name}"), "Name", name.clone(), "", false)),
        a(K::MergeInto, "Merge Into").asking(pick(format!("Merge {name} into"), "Merge")),
        a(K::Colour, "Colour").asking(text(
            format!("Colour of {name}"),
            "Colour",
            label.color.clone().unwrap_or_default(),
            COLOUR,
            true,
        )),
    ];
    reorder(&mut actions, Subject::Label, &name, position_of(&ids, &label.id), ids.len());
    actions.push(a(K::Delete, "Delete").asking(confirm(
        format!("Delete {name}?"),
        "Tasks wearing it stay; they just stop showing it.",
        "Delete Label",
    )));
    actions
}

/// A saved filter's.
pub fn filter(snapshot: &Snapshot, filter: &SavedFilter) -> Vec<Action> {
    use ActionKind as K;
    let name = filter.name.clone();
    let a = |kind, title: &str| action(kind, Subject::Filter, title, name.clone());
    let mut live: Vec<&SavedFilter> = snapshot.saved_filters.values().filter(|f| f.deleted_at.is_none()).collect();
    live.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
    let ids: Vec<_> = live.iter().map(|f| f.id).collect();
    let mut actions = vec![
        a(K::Rename, "Rename").asking(text(format!("Rename {name}"), "Name", name.clone(), "", false)),
        a(K::ChangeQuery, "Change Query").asking(text(
            format!("Query of {name}"),
            "Query",
            filter.query.clone(),
            QUERY,
            false,
        )),
    ];
    reorder(&mut actions, Subject::Filter, &name, position_of(&ids, &filter.id), ids.len());
    actions.push(a(K::Delete, "Delete").asking(confirm(
        format!("Delete {name}?"),
        "The tasks it shows are not touched.",
        "Delete Filter",
    )));
    actions
}

/// A sidebar heading's: what adds one more of what is under it.
#[must_use]
pub fn heading(group: crate::places::SidebarGroup) -> Vec<Action> {
    use crate::places::SidebarGroup as G;
    vec![match group {
        G::Projects => action(ActionKind::New, Subject::Project, "New Project", "")
            .asking(sentenced(text("New Project", "Name", "", "", false))),
        G::Labels => action(ActionKind::New, Subject::Label, "New Label", "")
            .asking(sentenced(text("New Label", "Name", "", "", false))),
        G::Filters => action(ActionKind::New, Subject::Filter, "New Saved Filter", "").asking(Question::Form),
    }]
}

/// A question whose title holds no one's own words, in sentence case as well.
fn sentenced(mut question: Question) -> Question {
    if let Question::Text { title, sentence, .. } = &mut question {
        *sentence = crate::fields::sentence_case(title.clone());
    }
    question
}

/// What Go to Day asks, in every client that has it.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn go_to_day_question() -> Question {
    let Question::Text { title, label, initial, hint, optional, sentence, .. } =
        sentenced(text("Go to Day", "Day", "", "A date, such as Friday, or 12 October.", false))
    else {
        unreachable!()
    };
    Question::Text { title, label, initial, hint, optional, yes: "Go".to_owned(), sentence }
}

/// What a new saved filter asks, in two steps: its name, then its query.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn new_filter_questions() -> Vec<Question> {
    let step = |label: &str, hint: &str, yes: &str| {
        let Question::Text { title, initial, optional, sentence, .. } =
            sentenced(text("New Saved Filter", label, "", hint, false))
        else {
            unreachable!()
        };
        Question::Text { title, label: label.to_owned(), initial, hint: hint.to_owned(), optional, yes: yes.to_owned(), sentence }
    };
    vec![step("Name", "", "Next"), step("Query", QUERY, "Add")]
}

/// What a pick with a [`Question::Pick::length`] asks once something is picked.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn length_question() -> Question {
    let Question::Text { title, label, initial, hint, optional, sentence, .. } =
        sentenced(text("Planned Length", "Planned length", "", LENGTH, true))
    else {
        unreachable!()
    };
    Question::Text { title, label, initial, hint, optional, yes: "Save".to_owned(), sentence }
}

/// A reminder of a target's own: removing it, which undo brings back.
#[must_use]
pub fn reminder(id: &str, words: &str) -> Vec<Action> {
    let mut remove = action(ActionKind::Delete, Subject::Reminder, "Remove", id);
    remove.title = "Remove Reminder".to_owned();
    remove.sentence = crate::fields::sentence_case(remove.title.clone());
    let _ = words;
    vec![remove]
}

/// A target's list of reminders: adding one, by a phrase the core reads.
#[must_use]
pub fn add_reminder(target: &str, name: &str, block: bool) -> Vec<Action> {
    let hint = if block {
        "Such as 5m before start, at end, or 10m after end."
    } else {
        "Such as 30m before, at due, or 1h after."
    };
    vec![action(ActionKind::New, Subject::Reminder, "Add Reminder", target)
        .asking(text(format!("New reminder for {name}"), "When", "", hint, false))]
}

/// "10 Minutes" from "10 minutes": a fixed phrase in Title Case, for a button.
#[must_use]
pub fn title_case(text: &str) -> String {
    text.split(' ')
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A paired device's. A device cannot unpair itself, so that is not offered on its own row.
#[must_use]
pub fn device(name: &str, node_id: &str, this_device: bool) -> Vec<Action> {
    let mut actions = vec![action(ActionKind::Rename, Subject::Device, "Rename", node_id).asking(text(
        format!("Rename {name}"),
        "Name",
        name,
        "",
        false,
    ))];
    if !this_device {
        actions.push(
            action(ActionKind::Unpair, Subject::Device, "Unpair", node_id)
                .asking(confirm(format!("Unpair {name}?"), UNPAIRING, "Unpair")),
        );
    }
    actions
}

/// Why a row does not offer `kind`, for a key or a menu command pressed on it anyway:
/// silence would leave a screen-reader user guessing. `this_device` is a device row's own.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn not_offered(kind: ActionKind, subject: Subject, this_device: bool) -> String {
    match (subject, kind) {
        (Subject::Device, ActionKind::Unpair) if this_device => {
            "This is the device you are using; unpair it from one of your other devices.".to_owned()
        }
        (Subject::Project, ActionKind::Rename | ActionKind::Delete | ActionKind::Archive | ActionKind::MoveUnder) => {
            "The Inbox keeps its name and its place; only its order and weight change.".to_owned()
        }
        (Subject::Project | Subject::Label | Subject::Filter, ActionKind::MoveUp) => "It is already first.".to_owned(),
        (Subject::Project | Subject::Label | Subject::Filter, ActionKind::MoveDown) => "It is already last.".to_owned(),
        (Subject::Block, ActionKind::AssignTask) => "This block does not take tasks.".to_owned(),
        (Subject::Block, ActionKind::CancelDay) => {
            "It happens only once; to remove it, delete it.".to_owned()
        }
        _ => "That does not apply here.".to_owned(),
    }
}

/// Gives every row of a listing its actions, by what it is.
pub fn fill_rows(rows: &mut Rows, snapshot: &Snapshot) {
    let facts = snapshot.facts();
    let ids = rows_ids(rows);
    for (row, id) in rows.rows.iter_mut().zip(ids) {
        row.actions = match id {
            Some(RowId::Task(id)) => snapshot
                .tasks
                .get(&id)
                .map(|t| task(snapshot, t, row.checked.unwrap_or_else(|| facts.is_completed(t)), false))
                .unwrap_or_default(),
            Some(RowId::Project(id)) => snapshot.projects.get(&id).map(|p| project(snapshot, p)).unwrap_or_default(),
            Some(RowId::Label(id)) => snapshot.labels.get(&id).map(|l| label(snapshot, l)).unwrap_or_default(),
            Some(RowId::Occurrence(id, _)) => snapshot.series.get(&id).map(series).unwrap_or_default(),
            None => Vec::new(),
        };
    }
}

/// Each row's identifier read back from its text, by its role.
fn rows_ids(rows: &Rows) -> Vec<Option<RowId>> {
    rows.rows
        .iter()
        .map(|row| match row.role.as_str() {
            "task" => row.id.parse().ok().map(RowId::Task),
            "project" => row.id.parse().ok().map(RowId::Project),
            "label" => row.id.parse().ok().map(RowId::Label),
            "block" => {
                let (series, date) = row.id.split_once('@')?;
                Some(RowId::Occurrence(series.parse().ok()?, date.parse().ok()?))
            }
            _ => None,
        })
        .collect()
}

/// Gives a day's blocks, sittings, free time and cancelled days their actions.
pub fn fill_plan(plan: &mut crate::types::Plan, carries: &BTreeSet<String>, what_now: bool) {
    for block in &mut plan.blocks {
        for sitting in &mut block.assignments {
            sitting.actions = self::sitting(sitting);
            // Ended short of its planned time with the task still open: the rest can go on.
            if carries.contains(&sitting.id) {
                let left = sitting.planned_mins.unwrap_or(0).saturating_sub(sitting.minutes);
                let mut carry = action(ActionKind::CarryForward, Subject::Sitting, "Carry Forward", sitting.id.clone());
                carry.question = Question::Pick {
                    title: format!("Carry the remaining {} of {} to", crate::words::duration(left), sitting.title),
                    length: None,
                    yes: "Carry Forward".to_owned(),
                };
                let at = sitting.actions.len().saturating_sub(1);
                sitting.actions.insert(at, carry);
            }
        }
        block.actions = self::block(block);
        // What its end offers: the rest of every sitting it left short, each to its next block.
        if block.assignments.iter().filter(|s| carries.contains(&s.id)).count() > 1 {
            let mut all = action(ActionKind::CarryRest, Subject::Block, "Carry the Rest Forward", block.id.clone());
            all.question = Question::Immediate;
            let at = block.actions.len().saturating_sub(1);
            block.actions.insert(at, all);
        }
    }
    plan.actions = crate::day::day_actions(plan, what_now);
    for item in &mut plan.timeline {
        if let PlanItem::Free { start, actions, .. } = item {
            *actions = free_time(&plan.date, start);
        }
    }
    for day in &mut plan.cancelled {
        day.actions = cancelled(day, &plan.date);
    }
}





