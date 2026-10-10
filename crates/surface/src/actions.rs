//! What can be done to each thing a client lists, decided once.
//!
//! Every record a client shows as a row carries its `actions`: which apply now, in the
//! order offered, under their spoken names ("Delete Block", not "Delete"), each with the
//! question it asks before it runs. A client turns them into swipe actions, a context menu,
//! buttons or a key's command, asks the question its own way, and hands the action back with
//! the answer to [`Lumenna::act`]. It decides none of it: eleven clients deciding which
//! actions a row has, and what deleting asks, had drifted eleven ways.
//!
//! What a picker offers is the core's too ([`Lumenna::choices`]), so a task is never offered
//! as its own parent, and a project never moved inside itself.
//!
//! Only a [`Question::Form`] is the client's: the task and block forms are its own screens,
//! built on `task_fields` and `block_fields`.

use std::collections::{BTreeMap, BTreeSet};

use jiff::Zoned;
use lumenna_core::filter::Context;
use lumenna_core::id::TaskId;
use lumenna_core::model::{BlockSeries, Label, Project, SavedFilter, Task};
use lumenna_core::row::RowId;
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::error::{LumennaError, Result};
use crate::types::{
    CancelledBlock, Change, Direction, MoveTarget, PlanAssignment, PlanBlock, PlanItem,
    Rows,
};
use crate::{Lumenna, repaired, resolve};

/// What an action does. Generic across what it is done to, so a key can mean one thing
/// everywhere: Delete is [`ActionKind::Delete`] on whatever row has focus.
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
}

/// What an action is done to: with [`ActionKind`], what runs.
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
}

/// One thing that can be done, and what it asks first.
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(tag = "ask", rename_all = "snake_case")]
pub enum Question {
    /// Nothing: it runs at once, answered [`Answer::Yes`].
    Immediate,
    /// The client's own form, which [`Lumenna::act`] does not run.
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
    },
    /// One of what [`Lumenna::choices`] offers for this action.
    Pick {
        /// The chooser's title: "Move Write report to".
        title: String,
        /// A second question once one is picked: how long the sitting is meant to take, in
        /// [`Answer::Picked::length`], optional.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<String>,
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
}

/// What a picker offers.
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
        question: if matches!(kind, ActionKind::Edit | ActionKind::EditTask | ActionKind::AddBlock) {
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

fn pick(title: impl Into<String>) -> Question {
    Question::Pick { title: title.into(), length: None }
}

fn minutes_text(minutes: u32) -> String {
    format!("{minutes}m")
}

// ---------------------------------------------------------------------------------------
// Which apply
// ---------------------------------------------------------------------------------------

/// A task's, wherever it is listed or shown. `shown` leaves out Edit Details, for the
/// task's own screen, which is that form.
pub(crate) fn task(snapshot: &Snapshot, task: &Task, done: bool, shown: bool) -> Vec<Action> {
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
    }));
    actions.push(a(K::MoveToProject, "Move to Project").asking(pick(format!("Move {} to", task.title))));
    actions.push(a(K::MakeSubtaskOf, "Make Subtask Of").asking(pick(format!("Make {} a subtask of", task.title))));
    if task.parent_id.is_some() {
        actions.push(a(K::MoveToTopLevel, "Move to Top Level"));
    }
    actions.push(a(K::WaitFor, "Wait For").asking(pick(format!("What does {} wait for?", task.title))));
    for on in &task.depends {
        let title = snapshot.tasks.get(on).map_or("a task not loaded here", |t| t.title.as_str());
        let mut stop = a(K::StopWaiting, &format!("Stop Waiting for {title}")).with(on.to_string());
        stop.sentence = format!("Stop waiting for {title}");
        actions.push(stop);
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
        }));
    }
    actions.push(a(K::Edit, "Edit Block"));
    if block.repeats {
        actions.push(a(K::CancelDay, "Cancel This Day"));
    }
    if block.changed_for_this_day {
        actions.push(a(K::RestoreDay, "Restore This Day"));
    }
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
pub(crate) fn series(series: &BlockSeries) -> Vec<Action> {
    let id = series.id.to_string();
    vec![
        action(ActionKind::Edit, Subject::Series, "Edit Block", id.clone()),
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
pub(crate) fn project(snapshot: &Snapshot, project: &Project) -> Vec<Action> {
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
    actions.push(a(K::MoveUnder, "Move Under").asking(pick(format!("Move {name} under"))));
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
pub(crate) fn label(snapshot: &Snapshot, label: &Label) -> Vec<Action> {
    use ActionKind as K;
    let name = label.name.clone();
    let a = |kind, title: &str| action(kind, Subject::Label, title, name.clone());
    let mut live: Vec<&Label> = snapshot.labels.values().filter(|l| l.deleted_at.is_none()).collect();
    live.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
    let ids: Vec<_> = live.iter().map(|l| l.id).collect();
    let mut actions = vec![
        a(K::Rename, "Rename").asking(text(format!("Rename {name}"), "Name", name.clone(), "", false)),
        a(K::MergeInto, "Merge Into").asking(pick(format!("Merge {name} into"))),
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
pub(crate) fn filter(snapshot: &Snapshot, filter: &SavedFilter) -> Vec<Action> {
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
            .asking(text("New Project", "Name", "", "", false)),
        G::Labels => action(ActionKind::New, Subject::Label, "New Label", "")
            .asking(text("New Label", "Name", "", "", false)),
        G::Filters => action(ActionKind::New, Subject::Filter, "New Saved Filter", "").asking(Question::Form),
    }]
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
pub(crate) fn fill_rows(rows: &mut Rows, snapshot: &Snapshot) {
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
pub(crate) fn fill_plan(plan: &mut crate::types::Plan) {
    for block in &mut plan.blocks {
        for sitting in &mut block.assignments {
            sitting.actions = self::sitting(sitting);
        }
        block.actions = self::block(block);
    }
    for item in &mut plan.timeline {
        if let PlanItem::Free { start, actions, .. } = item {
            *actions = free_time(&plan.date, start);
        }
    }
    for day in &mut plan.cancelled {
        day.actions = cancelled(day, &plan.date);
    }
}

// ---------------------------------------------------------------------------------------
// What a picker offers
// ---------------------------------------------------------------------------------------

/// Every task under `root`, at any depth.
fn descendants(snapshot: &Snapshot, root: TaskId) -> BTreeSet<TaskId> {
    let mut children: BTreeMap<TaskId, Vec<TaskId>> = BTreeMap::new();
    for task in snapshot.tasks.values() {
        if let Some(parent) = task.parent_id {
            children.entry(parent).or_default().push(task.id);
        }
    }
    reach(root, |id| children.get(&id).cloned().unwrap_or_default())
}

/// Every task waiting on `root`, directly or through others: what waiting for any of them
/// would close a cycle with.
fn waiting_on(snapshot: &Snapshot, root: TaskId) -> BTreeSet<TaskId> {
    let mut waiters: BTreeMap<TaskId, Vec<TaskId>> = BTreeMap::new();
    for task in snapshot.tasks.values() {
        for on in &task.depends {
            waiters.entry(*on).or_default().push(task.id);
        }
    }
    reach(root, |id| waiters.get(&id).cloned().unwrap_or_default())
}

/// Everything reachable from `root` by `next`, without `root` itself; a visited set keeps an
/// unrepaired cycle finite.
fn reach(root: TaskId, next: impl Fn(TaskId) -> Vec<TaskId>) -> BTreeSet<TaskId> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![root];
    while let Some(id) = queue.pop() {
        for child in next(id) {
            if child != root && seen.insert(child) {
                queue.push(child);
            }
        }
    }
    seen
}

fn choices(choices: Vec<Choice>, noun: &str, none: &str) -> Choices {
    Choices {
        announcement: if choices.is_empty() { none.to_owned() } else { crate::words::count_line(choices.len(), noun) },
        notices: Vec::new(),
        choices,
    }
}

/// Open tasks in tree order, as choices, but for those `skip` refuses.
fn task_choices(snapshot: &Snapshot, query: &str, skip: impl Fn(&Task) -> bool) -> Result<Vec<Choice>> {
    let now = Zoned::now();
    let expr = resolve::query(snapshot, query)?;
    let cx = Context::new(snapshot, &now);
    Ok(snapshot
        .task_rows(&expr, &cx)
        .into_iter()
        .filter_map(|row| {
            let RowId::Task(id) = row.id else { return None };
            let task = snapshot.tasks.get(&id)?;
            (!skip(task)).then(|| Choice {
                id: id.to_string(),
                title: row.title,
                depth: row.depth,
                detail: snapshot.projects.get(&task.project_id).map(|p| p.name.clone()),
                ..Choice::default()
            })
        })
        .collect())
}

fn project_choices(snapshot: &Snapshot, skip: impl Fn(&Project) -> bool) -> Vec<Choice> {
    crate::organise::in_tree_order(snapshot)
        .into_iter()
        .filter(|(project, _)| !skip(project))
        .map(|(project, depth)| Choice { id: project.name.clone(), title: project.name.clone(), depth, ..Choice::default() })
        .collect()
}

fn answer_text(answer: &Answer) -> Result<String> {
    match answer {
        Answer::Text { text } => Ok(text.trim().to_owned()),
        _ => Err(LumennaError::new("this needs a line of text")),
    }
}

fn answer_pick(answer: &Answer) -> Result<(String, Option<String>)> {
    match answer {
        Answer::Picked { id, length } => Ok((id.clone(), length.clone())),
        _ => Err(LumennaError::new("this needs a choice")),
    }
}

/// An optional length: empty is none.
fn length(text: Option<&str>) -> Result<Option<u32>> {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(None),
        Some(text) => resolve::minutes(text).map(Some),
    }
}

fn split_day(target: &str) -> Result<(&str, &str)> {
    target.split_once('@').ok_or_else(|| LumennaError::new(format!("'{target}' does not name a day of a block")))
}

fn timer(timer: crate::types::Timer) -> Change {
    Change {
        announcement: timer.announcement,
        notices: timer.notices,
        changed: timer.changed,
        affected: crate::types::Affected { assignments: vec![timer.assignment], ..Default::default() },
        task: None,
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What a [`Question::Pick`] offers for `action`, in order: never a task as its own
    /// parent or a project inside itself, never a wait that would close a cycle.
    ///
    /// # Errors
    ///
    /// If the record it acts on cannot be found, or the action picks nothing.
    pub fn choices(&self, action: Action) -> Result<Choices> {
        use ActionKind as K;
        if (action.subject, action.kind) == (Subject::Task, K::PutInBlock) {
            return self.open_work_blocks();
        }
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let task = || -> Result<&Task> {
                let id = resolve::task_id(&snapshot, &action.target)?;
                snapshot.tasks.get(&id).ok_or_else(|| LumennaError::new("that task is not here"))
            };
            Ok(match (action.subject, action.kind) {
                (Subject::Task, K::MoveToProject) => {
                    let task = task()?;
                    choices(
                        project_choices(&snapshot, |p| p.id == task.project_id || p.archived),
                        "project",
                        "There is no other project to move it to.",
                    )
                }
                (Subject::Task, K::MakeSubtaskOf) => {
                    let task = task()?;
                    let under = descendants(&snapshot, task.id);
                    choices(
                        task_choices(&snapshot, "", |t| {
                            t.id == task.id || under.contains(&t.id) || Some(t.id) == task.parent_id
                        })?,
                        "task",
                        "There is no task it could go under.",
                    )
                }
                (Subject::Task, K::WaitFor) => {
                    let task = task()?;
                    let waiting = waiting_on(&snapshot, task.id);
                    choices(
                        task_choices(&snapshot, "", |t| {
                            t.id == task.id || task.depends.contains(&t.id) || waiting.contains(&t.id)
                        })?,
                        "task",
                        "There is no task it could wait for.",
                    )
                }
                (Subject::Block, K::AssignTask) => {
                    let (series, date) = split_day(&action.target)?;
                    let series = snapshot
                        .series
                        .get(&resolve::series_id(&snapshot, series)?)
                        .ok_or_else(|| LumennaError::new("that block is not here"))?;
                    let day: jiff::civil::Date =
                        date.parse().map_err(|_| LumennaError::new(format!("'{date}' is not a day")))?;
                    let occurrence = if series.is_recurring() {
                        lumenna_core::model::BlockRef::Occurrence(series.id, day)
                    } else {
                        lumenna_core::model::BlockRef::OneOff(series.id)
                    };
                    let assigned: BTreeSet<TaskId> = snapshot
                        .assignments
                        .values()
                        .filter(|a| a.block_ref == occurrence)
                        .map(|a| a.task_id)
                        .collect();
                    let filter = series.task_filter.clone().unwrap_or_default();
                    let none = if filter.trim().is_empty() {
                        "There is no open task to assign.".to_owned()
                    } else {
                        format!("No open task matches this block's filter, {filter}.")
                    };
                    choices(task_choices(&snapshot, &filter, |t| assigned.contains(&t.id))?, "task", &none)
                }
                (Subject::Project, K::MoveUnder) => {
                    let project = resolve::project(&snapshot, &action.target)?;
                    choices(
                        project_choices(&snapshot, |p| {
                            p.id == project.id
                                || p.is_inbox
                                || Some(p.id) == project.parent_id
                                || snapshot.is_within(p.id, project.id)
                        }),
                        "project",
                        "There is no other project to move it under.",
                    )
                }
                (Subject::Label, K::MergeInto) => {
                    let label = resolve::label(&snapshot, &action.target)?;
                    let mut live: Vec<&Label> =
                        snapshot.labels.values().filter(|l| l.deleted_at.is_none() && l.id != label.id).collect();
                    live.sort_by(|x, y| x.order.cmp_with(&x.id, &y.order, &y.id));
                    choices(
                        live.into_iter()
                            .map(|l| Choice { id: l.name.clone(), title: l.name.clone(), ..Choice::default() })
                            .collect(),
                        "label",
                        "There is no other label to merge it into.",
                    )
                }
                _ => return Err(LumennaError::new(format!("{} picks nothing", action.title))),
            })
        })
    }
    /// Runs `action` with the answer to its question: [`Answer::Yes`] for one that asks
    /// nothing or a confirmation.
    ///
    /// # Errors
    ///
    /// If the answer is not the kind its question asks, the action is a client's form, or
    /// the operation it runs refuses.
    pub fn act(&self, action: Action, answer: Answer) -> Result<Change> {
        use ActionKind as K;
        use Subject as S;
        if action.question == Question::Form {
            return Err(LumennaError::new(format!("{} opens a form of the app's own", action.title)));
        }
        if matches!(action.question, Question::Confirm { .. } | Question::Immediate) && answer != Answer::Yes {
            return Ok(Change::unchanged("nothing was done"));
        }
        let target = action.target.as_str();
        match (action.subject, action.kind) {
            (S::Task, K::MarkDone) => self.complete_task(target),
            (S::Task, K::MarkNotDone) => self.uncomplete_task(target),
            (S::Task, K::Delete) => self.trash_task(target),
            (S::Task, K::Restore) => self.restore_task(target),
            (S::Task, K::DeleteForGood) => self.erase_task(target),
            (S::Task, K::MoveToTopLevel) => self.move_task(target, MoveTarget::Top),
            (S::Task, K::MoveToProject) => {
                self.move_task(target, MoveTarget::Project { name: answer_pick(&answer)?.0 })
            }
            (S::Task, K::MakeSubtaskOf) => self.move_task(target, MoveTarget::Parent { id: answer_pick(&answer)?.0 }),
            (S::Task, K::WaitFor) => self.add_dependency(target, &answer_pick(&answer)?.0),
            (S::Task, K::StopWaiting) => self.remove_dependency(target, action.other.as_deref().unwrap_or_default()),
            (S::Task, K::PutInBlock) => {
                let (block, minutes) = answer_pick(&answer)?;
                self.assign(target, &block, None, length(minutes.as_deref())?)
            }
            (S::Block, K::AssignTask) => {
                let (task, minutes) = answer_pick(&answer)?;
                self.assign(&task, target, None, length(minutes.as_deref())?)
            }
            (S::Block, K::CancelDay) => {
                let (series, date) = split_day(target)?;
                self.cancel_occurrence(series, date)
            }
            (S::Block, K::RestoreDay) => {
                let (series, date) = split_day(target)?;
                self.restore_occurrence(series, date)
            }
            (S::Block | S::Series, K::Delete) => self.delete_block(target),
            (S::Sitting, K::StartTimer | K::ResumeTimer) => self.start_timer(target),
            (S::Sitting, K::PauseTimer) => self.pause_timer(target).map(timer),
            (S::Sitting, K::StopTimer) => self.stop_timer(target, None).map(timer),
            (S::Sitting, K::PlannedLength) => self.plan_minutes(target, length(Some(&answer_text(&answer)?))?),
            (S::Sitting, K::LogMinutes) => {
                let text = answer_text(&answer)?;
                if text.is_empty() {
                    return Err(LumennaError::new("say how many minutes, such as 45m"));
                }
                self.stop_timer(target, Some(resolve::minutes(&text)?)).map(timer)
            }
            (S::Sitting, K::Unassign) => self.unassign(target),
            (S::Project, K::Rename) => self.rename_project(target, &answer_text(&answer)?),
            (S::Project, K::NewInside) => self.add_project(&answer_text(&answer)?, Some(target.to_owned())),
            (S::Project, K::New) => self.add_project(&answer_text(&answer)?, None),
            (S::Project, K::MoveUnder) => self.move_project(target, Some(answer_pick(&answer)?.0)),
            (S::Project, K::MoveToTopLevel) => self.move_project(target, None),
            (S::Project, K::MoveUp) => self.reorder_project(target, Direction::Up),
            (S::Project, K::MoveDown) => self.reorder_project(target, Direction::Down),
            (S::Project, K::Weight) => self.weigh_project(target, crate::form::parse_weight(answer_text(&answer)?)?),
            (S::Project, K::Archive | K::Unarchive) => {
                let archived = resolve_archived(self, target)?;
                if archived == (action.kind == K::Archive) {
                    Ok(Change::unchanged(if archived { "it is already archived" } else { "it is not archived" }))
                } else {
                    self.archive_project(target)
                }
            }
            (S::Project, K::Delete) => self.delete_project(target, answer_pick(&answer)?.0 == "keep"),
            (S::Label, K::Rename) => self.rename_label(target, &answer_text(&answer)?),
            (S::Label, K::New) => self.add_label(&answer_text(&answer)?),
            (S::Label, K::MergeInto) => self.merge_labels(target, &answer_pick(&answer)?.0),
            (S::Label, K::Colour) => {
                let colour = answer_text(&answer)?;
                let colour = (!colour.is_empty() && !colour.eq_ignore_ascii_case("none")).then_some(colour);
                self.recolour_label(target, colour)
            }
            (S::Label, K::MoveUp) => self.reorder_label(target, Direction::Up),
            (S::Label, K::MoveDown) => self.reorder_label(target, Direction::Down),
            (S::Label, K::Delete) => self.delete_label(target),
            (S::Filter, K::Rename) => self.edit_filter(target, Some(answer_text(&answer)?), None),
            (S::Filter, K::ChangeQuery) => {
                let query = answer_text(&answer)?;
                if query.is_empty() {
                    return Err(LumennaError::new("a saved filter needs a query"));
                }
                self.edit_filter(target, None, Some(query))
            }
            (S::Filter, K::MoveUp) => self.reorder_filter(target, Direction::Up),
            (S::Filter, K::MoveDown) => self.reorder_filter(target, Direction::Down),
            (S::Filter, K::Delete) => self.delete_filter(target),
            #[cfg(feature = "link")]
            (S::Device, K::Rename) => {
                let name = answer_text(&answer)?;
                if name.is_empty() {
                    return Err(LumennaError::new("a device needs a name"));
                }
                self.rename_device(target, &name)
            }
            #[cfg(feature = "link")]
            (S::Device, K::Unpair) => self.unpair_device(target),
            _ => Err(LumennaError::new(format!("{} is not something this build can do", action.title))),
        }
    }
}

fn resolve_archived(lumenna: &Lumenna, name: &str) -> Result<bool> {
    lumenna.with(|store| Ok(resolve::project(&repaired(store), name)?.archived))
}

impl Lumenna {
    /// The work blocks a task could go in this week, but for those already over: which
    /// they are is the planner's own question.
    fn open_work_blocks(&self) -> Result<Choices> {
        self.work_block_choices(None, Some(7))
    }
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What Put in a Block offers from another day: `days` days of work blocks from `from`
    /// (a date phrase), but for those already over. [`Lumenna::choices`] offers this week's.
    ///
    /// # Errors
    ///
    /// If `from` is not a date.
    pub fn work_block_choices(&self, from: Option<String>, days: Option<u32>) -> Result<Choices> {
        let blocks = self.work_blocks(from, days)?;
        let open: Vec<Choice> = blocks
            .blocks
            .into_iter()
            .filter(|b| b.when != "past")
            .map(|b| Choice {
                id: b.id,
                title: b.title,
                date: Some(b.date),
                start: Some(b.start),
                end: Some(b.end),
                ..Choice::default()
            })
            .collect();
        let none = match (blocks.days, blocks.from == jiff::Zoned::now().date().to_string()) {
            (7, true) => "There are no work blocks this week. Add one from Today.".to_owned(),
            (1, _) => format!("There are no work blocks on {}.", blocks.from),
            (days, _) => format!("There are no work blocks in the {days} days from {}.", blocks.from),
        };
        Ok(choices(open, "work block", &none))
    }
}
