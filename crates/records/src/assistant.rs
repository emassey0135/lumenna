//! What an assistant can do with Lumenna, once for every assistant: `lum mcp`'s tools, Siri,
//! Shortcuts and Apple Intelligence's App Intents, and Gemini's Android App Functions. Each is
//! drawn from [`operations`] and runs through the host's one dispatcher (`Lumenna::assist`), so
//! no assistant can do what another cannot, and none words an operation its own way.
//!
//! An operation says what it is (its `id`, a name for a person and one for an assistant, a
//! category and words to find it by), what it takes ([`AssistantParameter`]: each by name, what
//! kind of thing it is, and what it means), and what it does to the store: whether it writes,
//! removes something, can be repeated to no further effect, and whether it asks first
//! (`confirms`: it refuses without `confirm`, saying the core's question, for the assistant to
//! put to the person). What it answers is an [`Assisted`], each a record the surface already
//! has, whose `announcement` is the sentence to say.

use serde::{Deserialize, Serialize};

use crate::proposals::{DayPlan, DayPlanned, ProposedSitting, Replan, ReplanChange, Replanned};
use crate::{Change, Choices, Filters, Plan, Rows, TaskShown, Timer};

/// One thing an assistant can do.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantOperation {
    /// What it is called in code and by `lum mcp`: "complete_task".
    pub id: String,
    /// Its name for a person, in Title Case: "Complete a Task".
    pub title: String,
    /// The same in sentence case: "Complete a task".
    pub sentence: String,
    /// What it does and when to use it, for an assistant choosing among them.
    pub description: String,
    /// Where it is listed among the others.
    pub category: AssistantCategory,
    /// Words it is found by, beside its name.
    pub keywords: Vec<String>,
    /// Whether it changes the store; one that does not only reads.
    pub writes: bool,
    /// Whether it removes something, or takes it out of where it was.
    pub destructive: bool,
    /// Whether doing it twice does no more than doing it once.
    pub idempotent: bool,
    /// Whether it refuses without its `confirm` parameter, answering with the question to ask
    /// the person first: what nothing can undo.
    pub confirms: bool,
    /// What it takes, in the order a person would give it.
    pub parameters: Vec<AssistantParameter>,
}

/// Where an operation is listed.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantCategory {
    /// Finding, adding and changing tasks.
    Tasks,
    /// The trash, and erasing permanently.
    Trash,
    /// The day's blocks and the tasks in them.
    Day,
    /// A sitting's timer.
    Timers,
    /// What to work on, and the planner's proposals.
    Planning,
    /// Undo and redo.
    Undo,
}

impl AssistantCategory {
    /// Its heading, in Title Case.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Tasks => "Tasks",
            Self::Trash => "The Trash",
            Self::Day => "The Day",
            Self::Timers => "Timers",
            Self::Planning => "Planning",
            Self::Undo => "Undo",
        }
    }
}

/// One thing an operation takes.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssistantParameter {
    /// Its name: "id", "query".
    pub name: String,
    /// What a person calls it, in Title Case: "Task", "Filter". A form that asks for it labels its
    /// field with this, and its `description` is the field's hint.
    pub title: String,
    /// What to give, for an assistant.
    pub description: String,
    /// What kind of thing it is.
    pub kind: ParameterKind,
    /// Whether it may be left out.
    pub optional: bool,
}

/// What kind of thing a parameter is: what an assistant asks the person for, and what a
/// platform offers to choose from (a task, a project, a block).
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKind {
    /// Words.
    Text,
    /// A quick-add line, read as a person's would be.
    Line,
    /// A filter in the filter language.
    Query,
    /// A day, or a date and time, as a person says it.
    Date,
    /// A number of minutes.
    Minutes,
    /// A priority, 1 to 4, 1 highest.
    Priority,
    /// Yes or no.
    Flag,
    /// A task, by identifier.
    Task,
    /// A block on a day, by identifier, as the day lists it.
    Block,
    /// A sitting (a task's place in a block), by identifier.
    Sitting,
    /// A project, by name.
    Project,
    /// Labels, each by name.
    Labels,
    /// One of What Should I Work On?'s choices, by identifier.
    Choice,
    /// A replan, as it was proposed.
    Replan,
    /// A replan's changes accepted.
    ReplanChanges,
    /// A day plan, as it was proposed.
    DayPlan,
    /// A day plan's sittings accepted.
    DayPlanSittings,
}

/// A value given to an operation's parameter.
#[boltffi::data]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Argument {
    /// The parameter's name.
    pub name: String,
    /// What is given.
    pub value: ArgumentValue,
}

/// What a parameter is given.
#[boltffi::data]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArgumentValue {
    /// Words, an identifier or a name.
    Text(String),
    /// A number.
    Number(i64),
    /// Yes or no.
    Flag(bool),
    /// Several names.
    Texts(Vec<String>),
    /// A replan, as proposed.
    Replan(Replan),
    /// A replan's changes.
    ReplanChanges(Vec<ReplanChange>),
    /// A day plan, as proposed.
    DayPlan(DayPlan),
    /// A day plan's sittings.
    DayPlanSittings(Vec<ProposedSitting>),
}

/// What an operation answers: a record of the surface's, its `announcement` the sentence to say.
#[boltffi::data]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Assisted {
    /// A change made.
    Change(Change),
    /// A listing.
    Rows(Rows),
    /// One task whole.
    Task(TaskShown),
    /// A day.
    Plan(Plan),
    /// The saved filters.
    Filters(Filters),
    /// What to work on.
    Choices(Choices),
    /// A replan proposed.
    Replan(Replan),
    /// A day plan proposed.
    DayPlan(DayPlan),
    /// A timer paused or stopped.
    Timer(Timer),
    /// A replan's changes made, and the next proposal.
    Replanned(Replanned),
    /// A day plan's sittings made, and the next plan.
    DayPlanned(DayPlanned),
}

impl Assisted {
    /// The sentence to say.
    #[must_use]
    pub fn announcement(&self) -> &str {
        use crate::Announced;
        match self {
            Self::Change(x) => x.announcement(),
            Self::Rows(x) => x.announcement(),
            Self::Task(x) => x.announcement(),
            Self::Plan(x) => x.announcement(),
            Self::Filters(x) => x.announcement(),
            Self::Choices(x) => x.announcement(),
            Self::Replan(x) => x.announcement(),
            Self::DayPlan(x) => x.announcement(),
            Self::Timer(x) => x.announcement(),
            Self::Replanned(x) => x.announcement(),
            Self::DayPlanned(x) => x.announcement(),
        }
    }
}

/// An operation as the table writes it.
struct Op {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    category: AssistantCategory,
    keywords: &'static [&'static str],
    writes: bool,
    destructive: bool,
    idempotent: bool,
    confirms: bool,
    parameters: &'static [Param],
}

/// A parameter as the table writes it.
/// Its name, its title, its kind, whether it may be left out, and what it means.
struct Param(&'static str, &'static str, ParameterKind, bool, &'static str);

const fn read(id: &'static str, title: &'static str, category: AssistantCategory, keywords: &'static [&'static str], parameters: &'static [Param], description: &'static str) -> Op {
    Op { id, title, description, category, keywords, writes: false, destructive: false, idempotent: true, confirms: false, parameters }
}

const fn write(id: &'static str, title: &'static str, category: AssistantCategory, keywords: &'static [&'static str], parameters: &'static [Param], description: &'static str) -> Op {
    Op { id, title, description, category, keywords, writes: true, destructive: false, idempotent: false, confirms: false, parameters }
}

impl Op {
    const fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    const fn idempotent(mut self) -> Self {
        self.idempotent = true;
        self
    }

    const fn confirms(mut self) -> Self {
        self.confirms = true;
        self
    }
}

use AssistantCategory as C;
use ParameterKind as K;

const ID: Param = Param("id", "Task", K::Task, false, "The task's identifier: a whole UUID, or a prefix that names one task.");
const DAY: Param = Param("date", "Day", K::Date, true, "The day, as a person says it (\"tomorrow\", \"friday\", \"2026-11-03\"); today if left out.");
const SITTING: Param = Param("assignment", "Sitting", K::Sitting, false, "The sitting's identifier: a task's place in a block, as the day lists it.");
const RUNNING: Param = Param("assignment", "Sitting", K::Sitting, true, "The sitting's identifier, as the day lists it; the sitting whose timer runs when left out.");
const CONFIRM: Param = Param("confirm", "Confirm", K::Flag, true, "True only once the person has said yes to the question this asks without it.");

const OPERATIONS: &[Op] = &[
    read("list_tasks", "List Tasks", C::Tasks, &["tasks", "to do", "overdue", "due"], &[
        Param("query", "Filter", K::Query, true, "A filter in Lumenna's filter language, such as `overdue`, `today & #Work`, `@calls`, `p1`, or `search: words`; empty for every open task."),
    ], "Lists tasks: by default every open task, most urgent first, subtasks under their task; or those a filter admits."),
    read("search_tasks", "Search Tasks", C::Tasks, &["find", "search"], &[
        Param("text", "Words", K::Text, false, "Words to search titles and notes for."),
    ], "Searches task titles and notes for words."),
    read("show_task", "Show a Task", C::Tasks, &["details", "notes"], &[ID],
        "One task whole: its fields, states, subtasks, what it waits for, its sittings and its history."),
    read("day", "Show the Day", C::Day, &["today", "schedule", "agenda", "blocks", "plan"], &[DAY],
        "A day as it is lived: its blocks in time order, each with the tasks in it (its sittings), the free time between them, and where now falls."),
    read("list_blocks", "List Blocks", C::Day, &["blocks", "routine", "schedule"], &[],
        "Every time block, each with when it happens and how often."),
    read("list_projects", "List Projects", C::Tasks, &["projects"], &[], "Every project, its subprojects under it."),
    read("list_labels", "List Labels", C::Tasks, &["labels", "tags"], &[], "Every label."),
    read("list_filters", "List Saved Filters", C::Tasks, &["filters", "views"], &[], "Every saved filter, with its query."),
    read("what_should_i_work_on", "What Should I Work On?", C::Planning, &["next", "focus", "now", "suggest"], &[],
        "What to work on now, ranked, each choice with the reason it is offered: in a block under way its own unfinished sittings, then its suggestions; in free time, what fits before the next block. Starting on one is `start_on`."),
    read("propose_replan", "Propose a Replan of Today", C::Planning, &["replan", "behind", "reschedule", "running late"], &[],
        "Proposes how to replan the rest of today, when the day has slipped: every change as a whole sentence. Nothing changes until `apply_replan` is given the ones the person accepted; say them to the person first."),
    read("timer", "Show the Timer", C::Timers, &["timer", "running", "how long", "tracking"], &[],
        "What the timer is doing now: running, on which task and how long so far; else what `toggle_timer` would start, or why nothing would."),
    read("propose_day_plan", "Propose a Plan for a Day", C::Planning, &["plan the day", "fill", "schedule"], &[DAY],
        "Proposes which tasks to put in each of a day's work blocks still to come, each with why. Nothing changes until `apply_day_plan` is given the sittings the person accepted."),
    write("add_task", "Add a Task", C::Tasks, &["new", "create", "remember", "capture", "remind"], &[
        Param("text", "Line", K::Line, false, "A quick-add line, read as a person's would be: the title, then any of a due date or time (\"tomorrow at 3pm\", \"every weekday\"), a priority (p1 to p4), labels (@calls), a project (#Work, #\"Home Office\"), an estimate (~30m). A line starting with `block` makes a time block instead (\"block Deep work 9-11am weekdays\"). The answer says how it was read."),
    ], "Adds a task (or, from a line starting with `block`, a time block) from one quick-add line, and says how it read it."),
    write("edit_task", "Edit a Task", C::Tasks, &["change", "rename", "reschedule", "due date"], &[
        ID,
        Param("title", "Title", K::Text, true, "A new title."),
        Param("due", "Due", K::Date, true, "A date phrase (\"next monday\", \"in 3 days at 9am\"), or `none` to clear it."),
        Param("repeat", "Repeats", K::Text, true, "A repetition (\"every monday\", \"every! 2 weeks\"), or `none` to stop it repeating."),
        Param("priority", "Priority", K::Priority, true, "1 to 4, where 1 is highest."),
        Param("estimate", "Estimate", K::Text, true, "How long it should take (\"45m\", \"1h30m\"), or `none`."),
        Param("notes", "Notes", K::Text, true, "Its notes, whole."),
        Param("project", "Project", K::Project, true, "The project to move it to, by name."),
        Param("labels", "Labels", K::Labels, true, "Its labels, all of them, by name."),
    ], "Changes a task's fields: only those given."),
    write("complete_task", "Complete a Task", C::Tasks, &["done", "finish", "check off", "tick"], &[ID],
        "Marks a task done. A repeating one moves on to its next date.").idempotent(),
    write("uncomplete_task", "Mark a Task Not Done", C::Tasks, &["undone", "reopen"], &[ID], "Marks a done task not done.").idempotent(),
    write("trash_task", "Move a Task to the Trash", C::Trash, &["delete", "remove", "trash"], &[ID],
        "Moves a task to the trash, from which `restore_task` brings it back.").destructive().idempotent(),
    write("erase_task_permanently", "Erase a Task Permanently", C::Trash, &["erase", "permanently", "forever"], &[
        Param("id", "Task", K::Task, false, "The identifier of a task in the trash."),
        CONFIRM,
    ], "Erases a task in the trash permanently: from the history every device keeps, and with it the history of everything deleted before. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.").destructive().confirms(),
    write("erase_everything_deleted", "Erase Everything Deleted", C::Trash, &["erase", "empty the trash", "permanently"], &[CONFIRM],
        "Erases everything deleted permanently: every task in the trash, and the history of everything ever deleted, on every device. Nothing can undo it, so without `confirm` it answers with the question to ask the person first.").destructive().confirms(),
    write("restore_task", "Restore a Task", C::Trash, &["undelete", "bring back", "restore"], &[ID],
        "Brings a task back from the trash.").idempotent(),
    write("move_task", "Move a Task", C::Tasks, &["subtask", "project", "move"], &[
        ID,
        Param("parent", "Parent Task", K::Task, true, "A task to make it a subtask of, by identifier."),
        Param("project", "Project", K::Project, true, "A project to move it to, by name."),
        Param("top", "A Task of Its Own", K::Flag, true, "True to make it a task of its own again, under no other."),
    ], "Makes a task a subtask of another, moves it to a project, or makes it a task of its own."),
    write("assign", "Put a Task in a Block", C::Day, &["schedule", "block", "time block", "plan"], &[
        Param("task", "Task", K::Task, false, "The task's identifier."),
        Param("block", "Block", K::Block, false, "The block's identifier, as the day lists it."),
        Param("date", "Day", K::Date, true, "The day, for a block that repeats; the block's own day otherwise."),
        Param("minutes", "Minutes", K::Minutes, true, "How long the sitting is planned to take, in minutes."),
    ], "Puts a task in a block, as a sitting: one session of work on it there."),
    write("unassign", "Take a Task out of a Block", C::Day, &["unschedule", "remove from block"], &[SITTING],
        "Takes a sitting out of its block; the task itself stays.").destructive(),
    write("start_timer", "Start a Timer", C::Timers, &["start", "begin", "work on", "resume"], &[SITTING],
        "Starts, or resumes, a sitting's timer."),
    write("pause_timer", "Pause a Timer", C::Timers, &["pause", "break"], &[RUNNING],
        "Pauses a sitting's timer, keeping what it has logged: the timer running, unless a sitting is named. Refused when none runs."),
    write("stop_timer", "Stop a Timer", C::Timers, &["stop", "finish", "log time"], &[
        RUNNING,
        Param("minutes", "Minutes", K::Minutes, true, "The minutes to log instead of the timer's, when it ran on by mistake."),
    ], "Stops a sitting's timer and logs its time: the timer running, unless a sitting is named. Refused when none runs."),
    write("toggle_timer", "Start or Stop the Timer", C::Timers, &["timer", "toggle", "start", "stop", "track time"], &[],
        "Presses the timer's one control: stops the timer running, or with none running starts on the first of What Should I Work On?'s choices, its timer running. `timer` says first which it would do."),
    write("start_on", "Start on a Choice", C::Planning, &["start", "work on", "focus"], &[
        Param("choice", "Choice", K::Choice, false, "The `id` of one of the choices What Should I Work On? offered."),
    ], "Starts on one of What Should I Work On?'s choices at once, its timer running: in the block under way, or in a block of its own in free time."),
    write("apply_replan", "Apply a Replan", C::Planning, &["replan", "accept"], &[
        Param("proposal", "Proposal", K::Replan, false, "The proposal as `propose_replan` returned it, whole."),
        Param("accepted", "Accepted", K::ReplanChanges, false, "Those of its `changes` the person accepted, each as it was proposed."),
    ], "Makes the replan changes the person accepted, as one change they can undo, and proposes again around what they turned down."),
    write("apply_day_plan", "Apply a Day Plan", C::Planning, &["plan", "accept"], &[
        Param("proposal", "Proposal", K::DayPlan, false, "The proposal as `propose_day_plan` returned it, whole."),
        Param("accepted", "Accepted", K::DayPlanSittings, false, "Those of its `sittings` the person accepted, each as it was proposed."),
    ], "Puts the sittings the person accepted into their blocks, as one change they can undo, and plans again around what they turned down."),
    write("undo", "Undo", C::Undo, &["undo", "take back"], &[], "Undoes the last change made on this device, whoever made it: an app, a command, or an assistant."),
    write("redo", "Redo", C::Undo, &["redo"], &[], "Redoes what was last undone."),
];

/// Every operation an assistant can do, in the order they are listed.
#[must_use]
pub fn operations() -> Vec<AssistantOperation> {
    OPERATIONS
        .iter()
        .map(|op| AssistantOperation {
            id: op.id.to_owned(),
            title: op.title.to_owned(),
            sentence: crate::fields::sentence_case(op.title.to_owned()),
            description: op.description.to_owned(),
            category: op.category,
            keywords: op.keywords.iter().map(|&k| k.to_owned()).collect(),
            writes: op.writes,
            destructive: op.destructive,
            idempotent: op.idempotent,
            confirms: op.confirms,
            parameters: op
                .parameters
                .iter()
                .map(|Param(name, title, kind, optional, description)| AssistantParameter {
                    name: (*name).to_owned(),
                    title: (*title).to_owned(),
                    description: (*description).to_owned(),
                    kind: *kind,
                    optional: *optional,
                })
                .collect(),
        })
        .collect()
}

/// The operation called `id`, if there is one.
#[must_use]
pub fn operation(id: &str) -> Option<AssistantOperation> {
    operations().into_iter().find(|op| op.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_is_named_once_and_what_asks_first_takes_its_confirm() {
        let all = operations();
        let mut ids: Vec<&str> = all.iter().map(|op| op.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for op in &all {
            assert_eq!(op.confirms, op.parameters.iter().any(|p| p.name == "confirm"), "{}", op.id);
            assert!(!op.confirms || op.destructive, "{}", op.id);
            assert!(op.writes || !op.destructive, "{}", op.id);
            assert!(!op.description.is_empty() && !op.keywords.is_empty(), "{}", op.id);
        }
    }
}
