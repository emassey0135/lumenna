//! An assistant's operations set up once and run unattended by another app on the device:
//! Tasker's actions on Android. The person chooses the operation and fills its parameters on a
//! screen of Lumenna's own, which is the consent; the other app then runs it whenever it likes,
//! each value given as text (perhaps from one of its variables, filled in only as it runs), and
//! takes back what was answered as variables of its own.
//!
//! So everything such a screen and such a run decide is here: which operations may be set up
//! ([`operations`]: never one that asks first, since nobody is there to answer, nor one only a
//! proposal just made can fill), each field's words ([`fields`]), what text each kind of parameter
//! takes ([`arguments`]), what each answer gives back ([`outputs`], [`automated`]) and the line
//! that sums up an action set up ([`summary`]). A platform draws them and decides nothing.

use serde::{Deserialize, Serialize};

use crate::assistant::{Argument, ArgumentValue, AssistantOperation, Assisted, ParameterKind};
use crate::{LumennaError, Result};

/// What a screen setting up an action says, beside the operations' and their parameters' own
/// words. Title Case; `sentence_case` for the platforms whose convention it is.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationWords {
    /// The screen's title.
    pub title: String,
    /// The heading over the operations to choose from.
    pub operations: String,
    /// The heading over the chosen operation's parameters.
    pub parameters: String,
    /// What the screen says about the values it takes: a variable of the app running it.
    pub variables: String,
    /// Said of an operation that removes something, or takes it out of where it was.
    pub destructive: String,
    /// The button that keeps what was set up.
    pub save: String,
    /// The button that leaves without setting anything up.
    pub cancel: String,
    /// What a pick says with nothing in it to choose.
    pub nothing_to_choose: String,
    /// The refusal of Save with no operation chosen.
    pub choose_first: String,
    /// The refusal of a run the person did not set up on this device, or set up again since.
    pub not_set_up: String,
}

/// What a screen setting up an action says (`AutomationWords`).
#[must_use]
pub fn words() -> AutomationWords {
    AutomationWords {
        title: "Set Up an Action".to_owned(),
        operations: "What It Does".to_owned(),
        parameters: "What It Takes".to_owned(),
        variables: "Any of these can be a variable of the app running it, such as %name, given its value each time it runs.".to_owned(),
        destructive: "Removes something".to_owned(),
        save: "Save".to_owned(),
        cancel: "Cancel".to_owned(),
        nothing_to_choose: "There is nothing to choose from yet.".to_owned(),
        choose_first: "Choose what it does first.".to_owned(),
        not_set_up: "This action was not set up in Lumenna on this device, or was set up again since. Open it to set it up here.".to_owned(),
    }
}

/// Whether `op` may be set up to run unattended: not one that asks first, since nobody is there
/// to answer, and not one whose parameters only a proposal just made can fill.
#[must_use]
pub fn offered(op: &AssistantOperation) -> bool {
    !op.confirms && op.parameters.iter().all(|p| typed(p.kind))
}

/// Whether a person can give a parameter of this kind as a line of text.
fn typed(kind: ParameterKind) -> bool {
    !matches!(kind, ParameterKind::Replan | ParameterKind::ReplanChanges | ParameterKind::DayPlan | ParameterKind::DayPlanSittings)
}

/// Every operation that may be set up to run unattended, in the table's order.
#[must_use]
pub fn operations() -> Vec<AssistantOperation> {
    crate::assistant::operations().into_iter().filter(offered).collect()
}

/// The operation called `id`, if it may be set up.
///
/// # Errors
///
/// If there is none, or it may not run unattended.
pub fn operation(id: &str) -> Result<AssistantOperation> {
    operations().into_iter().find(|op| op.id == id).ok_or_else(|| {
        LumennaError::new("This cannot run unattended: it asks first, or needs a proposal just made.")
    })
}

/// One of an operation's parameters as a screen setting it up asks for it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationField {
    /// The parameter's name, which its value is given back under.
    pub name: String,
    /// The field's label, in Title Case.
    pub title: String,
    /// What to give, said and shown with the field.
    pub hint: String,
    /// Whether it may be left empty.
    pub optional: bool,
    /// The button beside the line that offers what to choose (`Device::automation_choices`),
    /// in Title Case: "Choose a Task". None where there is nothing to offer.
    pub choose: Option<String>,
    /// Whether what is chosen adds to the line, as one of several names, rather than replacing it.
    pub several: bool,
    /// Whether it takes a day, so a calendar beside the line can fill it.
    pub day: bool,
    /// Whether it takes a number.
    pub number: bool,
}

/// The fields of the operation `id`, in the order it takes them; none for an operation not offered.
#[must_use]
pub fn fields(id: &str) -> Vec<AutomationField> {
    let Ok(op) = operation(id) else { return Vec::new() };
    op.parameters
        .into_iter()
        .map(|p| AutomationField {
            hint: if p.optional { format!("{} It can be left empty.", p.description) } else { p.description },
            choose: choose(p.kind).map(str::to_owned),
            several: p.kind == ParameterKind::Labels,
            day: p.kind == ParameterKind::Date,
            number: matches!(p.kind, ParameterKind::Minutes | ParameterKind::Priority),
            name: p.name,
            title: p.title,
            optional: p.optional,
        })
        .collect()
}

/// The button that offers what a parameter of this kind can be.
fn choose(kind: ParameterKind) -> Option<&'static str> {
    Some(match kind {
        ParameterKind::Task => "Choose a Task",
        ParameterKind::Project => "Choose a Project",
        ParameterKind::Labels => "Choose a Label",
        ParameterKind::Priority => "Choose a Priority",
        ParameterKind::Flag => "Choose Yes or No",
        ParameterKind::Block => "Choose a Block",
        ParameterKind::Sitting => "Choose a Sitting",
        ParameterKind::Choice => "Choose What to Work On",
        _ => return None,
    })
}

/// A parameter's value as text: what a field holds, or what the app running it gives once its
/// variables are filled in.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationGiven {
    /// The parameter's name.
    pub name: String,
    /// What it was given.
    pub text: String,
}

/// What a parameter can be given, offered by the button beside its line: a task, a project, a
/// priority. `value` goes in the line; `title` and `detail` are what is said and shown.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationChoice {
    /// What goes in the line: an identifier, a name, a number.
    pub value: String,
    /// What it is called.
    pub title: String,
    /// More about it, if anything: a task's due date, the block a sitting is in.
    pub detail: Option<String>,
}

/// The choices a flag offers.
#[must_use]
pub fn flag_choices() -> Vec<AutomationChoice> {
    [("yes", "Yes"), ("no", "No")]
        .into_iter()
        .map(|(value, title)| AutomationChoice { value: value.to_owned(), title: title.to_owned(), detail: None })
        .collect()
}

/// The choices a priority offers: its number, said as the form says it.
#[must_use]
pub fn priority_choices() -> Vec<AutomationChoice> {
    crate::form::priorities().into_iter().map(|c| AutomationChoice { value: c.id, title: c.title, detail: None }).collect()
}

/// What `op` was given as text, typed as its parameters take it, leaving out what was left
/// empty. A flag takes yes or no, a priority 1 to 4 (or p1 to p4), minutes a length ("45",
/// "1h30m"), labels names separated by commas.
///
/// # Errors
///
/// If a value is not what its parameter takes, or names no parameter of `op`.
pub fn arguments(op: &AssistantOperation, given: Vec<AutomationGiven>) -> Result<Vec<Argument>> {
    let mut arguments = Vec::new();
    for AutomationGiven { name, text } in given {
        let Some(parameter) = op.parameters.iter().find(|p| p.name == name) else {
            return Err(LumennaError::new(format!("{} takes nothing called {name}.", op.sentence)));
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let title = &parameter.title;
        let value = match parameter.kind {
            ParameterKind::Flag => ArgumentValue::Flag(match text.to_lowercase().as_str() {
                "yes" | "true" | "on" | "1" => true,
                "no" | "false" | "off" | "0" => false,
                _ => return Err(LumennaError::new(format!("{title} takes yes or no, not {text}."))),
            }),
            ParameterKind::Priority => {
                let number = text.strip_prefix(['p', 'P']).unwrap_or(text);
                match number.parse::<i64>() {
                    Ok(n @ 1..=4) => ArgumentValue::Number(n),
                    _ => return Err(LumennaError::new(format!("{title} takes 1 to 4, where 1 is highest, not {text}."))),
                }
            }
            ParameterKind::Minutes => match text.parse::<u32>().ok().map(Ok).unwrap_or_else(|| crate::resolve::minutes(text)) {
                Ok(minutes) => ArgumentValue::Number(i64::from(minutes)),
                Err(_) => return Err(LumennaError::new(format!("{title} takes a number of minutes, not {text}."))),
            },
            ParameterKind::Labels => ArgumentValue::Texts(
                text.split(',').map(|l| l.trim().trim_start_matches('@').to_owned()).filter(|l| !l.is_empty()).collect(),
            ),
            _ => ArgumentValue::Text(text.to_owned()),
        };
        arguments.push(Argument { name, value });
    }
    Ok(arguments)
}

/// Whether what was given to the operation `id` can be saved: every parameter it needs is
/// given, and every value it can know now is what its parameter takes. A value holding a
/// variable (`%name`) is known only as it runs, so it is checked then.
///
/// # Errors
///
/// If the operation may not run unattended, something it needs is left empty, or a value is not
/// what its parameter takes.
pub fn check(id: &str, given: Vec<AutomationGiven>) -> Result<()> {
    let op = operation(id)?;
    for parameter in op.parameters.iter().filter(|p| !p.optional) {
        if !given.iter().any(|g| g.name == parameter.name && !g.text.trim().is_empty()) {
            return Err(LumennaError::new(format!("{} cannot be left empty.", parameter.title)));
        }
    }
    arguments(&op, given.into_iter().filter(|g| !g.text.contains('%')).collect()).map(|_| ())
}

/// The line that sums up an action set up, for the app running it to list it by: the
/// operation's name, then each value given with its parameter's name. `shown` is each value as
/// the person chose it (a task's title rather than its identifier). In sentence case where
/// `sentence` is set.
#[must_use]
pub fn summary(id: &str, shown: Vec<AutomationGiven>, sentence: bool) -> String {
    let Ok(op) = operation(id) else { return String::new() };
    let cased = |text: &str| if sentence { crate::fields::sentence_case(text.to_owned()) } else { text.to_owned() };
    let mut line = cased(&op.title);
    for parameter in &op.parameters {
        if let Some(given) = shown.iter().find(|g| g.name == parameter.name && !g.text.trim().is_empty()) {
            line.push_str(&format!(". {}: {}", cased(&parameter.title), given.text.trim()));
        }
    }
    line
}

/// What an operation answers, as a kind: which variables it gives back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answers {
    /// A change made.
    Change,
    /// A listing.
    Rows,
    /// One task whole.
    Task,
    /// A day.
    Plan,
    /// The saved filters.
    Filters,
    /// What to work on.
    Choices,
    /// A replan proposed.
    Replan,
    /// A day plan proposed.
    DayPlan,
    /// A timer paused or stopped.
    Timer,
    /// A replan's changes made.
    Replanned,
    /// A day plan's sittings made.
    DayPlanned,
}

impl Answers {
    /// The kind of `answer`.
    #[must_use]
    pub fn of(answer: &Assisted) -> Answers {
        match answer {
            Assisted::Change(_) => Answers::Change,
            Assisted::Rows(_) => Answers::Rows,
            Assisted::Task(_) => Answers::Task,
            Assisted::Plan(_) => Answers::Plan,
            Assisted::Filters(_) => Answers::Filters,
            Assisted::Choices(_) => Answers::Choices,
            Assisted::Replan(_) => Answers::Replan,
            Assisted::DayPlan(_) => Answers::DayPlan,
            Assisted::Timer(_) => Answers::Timer,
            Assisted::Replanned(_) => Answers::Replanned,
            Assisted::DayPlanned(_) => Answers::DayPlanned,
        }
    }

    /// What the operation `id` answers. The host's tests check each against what it does answer.
    #[must_use]
    pub fn to(id: &str) -> Option<Answers> {
        Some(match id {
            "list_tasks" | "search_tasks" | "list_blocks" | "list_projects" | "list_labels" => Answers::Rows,
            "show_task" => Answers::Task,
            "day" => Answers::Plan,
            "list_filters" => Answers::Filters,
            "what_should_i_work_on" => Answers::Choices,
            "propose_replan" => Answers::Replan,
            "propose_day_plan" => Answers::DayPlan,
            "pause_timer" | "stop_timer" | "timer" => Answers::Timer,
            "apply_replan" => Answers::Replanned,
            "apply_day_plan" => Answers::DayPlanned,
            "add_task" | "edit_task" | "complete_task" | "uncomplete_task" | "trash_task" | "erase_task_permanently"
            | "erase_everything_deleted" | "restore_task" | "move_task" | "assign" | "unassign" | "start_timer" | "start_on"
            | "toggle_timer" | "undo" | "redo" => Answers::Change,
            _ => return None,
        })
    }

    /// The variables this kind of answer gives back, beyond the announcement and notices.
    fn outputs(self) -> &'static [Output] {
        match self {
            Answers::Change | Answers::Replanned | Answers::DayPlanned => &[TASK_ID, TASK_TITLE],
            Answers::Rows => &[COUNT, IDS, TITLES],
            Answers::Task => &[TASK_ID, TASK_TITLE],
            Answers::Plan => &[SUMMARY, BLOCK_IDS, BLOCK_TITLES, SITTING_IDS, SITTING_TITLES],
            Answers::Filters => &[IDS, NAMES, QUERIES],
            Answers::Choices => &[IDS, TITLES, REASONS],
            Answers::Replan | Answers::DayPlan => &[SENTENCES],
            Answers::Timer => &[SITTING_ID, MINUTES],
        }
    }
}

/// A variable as the table writes it: its name, title, description, and whether it holds several.
struct Output(&'static str, &'static str, &'static str, bool);

const ANNOUNCEMENT: Output = Output("announcement", "Announcement", "What Lumenna says it did or found, as one sentence.", false);
const NOTICES: Output = Output("notices", "Notices", "Anything else worth knowing about it, a sentence each.", true);
const TASK_ID: Output = Output("task_id", "Task Identifier", "The task's identifier, which another action can be given.", false);
const TASK_TITLE: Output = Output("task_title", "Task Title", "The task's title.", false);
const COUNT: Output = Output("count", "Count", "How many it lists.", false);
const IDS: Output = Output("ids", "Identifiers", "Each one's identifier, in the order listed.", true);
const TITLES: Output = Output("titles", "Titles", "Each one's title, in the order listed.", true);
const NAMES: Output = Output("names", "Names", "Each saved filter's name, in the order listed.", true);
const QUERIES: Output = Output("queries", "Queries", "Each saved filter's query, in the order listed.", true);
const REASONS: Output = Output("reasons", "Reasons", "Why each is offered, in the order listed.", true);
const SUMMARY: Output = Output("summary", "Summary", "The day in a sentence.", false);
const BLOCK_IDS: Output = Output("block_ids", "Block Identifiers", "Each block's identifier, in time order.", true);
const BLOCK_TITLES: Output = Output("block_titles", "Block Titles", "Each block's title, in time order.", true);
const SITTING_IDS: Output = Output("sitting_ids", "Sitting Identifiers", "Each task's sitting in a block, by identifier, in time order.", true);
const SITTING_TITLES: Output = Output("sitting_titles", "Sitting Titles", "The title of each task in a block, in time order.", true);
const SENTENCES: Output = Output("sentences", "Changes", "Each change it proposes, as a sentence.", true);
const SITTING_ID: Output = Output("sitting_id", "Sitting Identifier", "The sitting's identifier.", false);
const MINUTES: Output = Output("minutes", "Minutes", "The minutes the sitting has logged.", false);

/// A variable an action set up gives back, as the app running it lists it for the person to
/// pick: its name (lower case, as an app's own variables go), title and description.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationOutput {
    /// Its name: "announcement", "task_id".
    pub name: String,
    /// What it is called, in Title Case.
    pub title: String,
    /// What it holds.
    pub description: String,
    /// Whether it holds several values, in order: an array.
    pub several: bool,
}

/// The variables the operation `id` gives back: its announcement and notices, then what its
/// answer holds that another action can use (identifiers and titles).
#[must_use]
pub fn outputs(id: &str) -> Vec<AutomationOutput> {
    let answers = Answers::to(id).map_or(&[][..], Answers::outputs);
    [ANNOUNCEMENT, NOTICES]
        .iter()
        .chain(answers)
        .map(|Output(name, title, description, several)| AutomationOutput {
            name: (*name).to_owned(),
            title: (*title).to_owned(),
            description: (*description).to_owned(),
            several: *several,
        })
        .collect()
}

/// One variable given back, by name: one value, or several in order where its output says so.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationValue {
    /// Its name, as [`outputs`] declares it.
    pub name: String,
    /// Its values: one, unless it holds several.
    pub values: Vec<String>,
    /// Whether it holds several values.
    pub several: bool,
}

/// What an action run answered, as the app running it takes it back.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Automated {
    /// The sentence to say, also the first variable.
    pub announcement: String,
    /// Every variable, as [`outputs`] declares them; one the answer has no value for is left out.
    pub values: Vec<AutomationValue>,
}

/// `answer` as variables.
#[must_use]
pub fn automated(answer: &Assisted) -> Automated {
    let mut values = Vec::new();
    let mut one = |output: &Output, value: Option<String>| {
        if let Some(value) = value {
            values.push(AutomationValue { name: output.0.to_owned(), values: vec![value], several: false });
        }
    };
    let task = |detail: Option<&crate::TaskDetail>| (detail.map(|t| t.id.clone()), detail.map(|t| t.title.clone()));
    let (notices, several): (Vec<String>, Vec<(&Output, Vec<String>)>) = match answer {
        Assisted::Change(x) => {
            let (id, title) = task(x.task.as_ref());
            one(&TASK_ID, id);
            one(&TASK_TITLE, title);
            (x.notices.clone(), Vec::new())
        }
        Assisted::Replanned(x) => {
            let (id, title) = task(x.change.task.as_ref());
            one(&TASK_ID, id);
            one(&TASK_TITLE, title);
            (x.notices.clone(), Vec::new())
        }
        Assisted::DayPlanned(x) => {
            let (id, title) = task(x.change.task.as_ref());
            one(&TASK_ID, id);
            one(&TASK_TITLE, title);
            (x.notices.clone(), Vec::new())
        }
        Assisted::Task(x) => {
            let (id, title) = task(Some(&x.task));
            one(&TASK_ID, id);
            one(&TASK_TITLE, title);
            (x.notices.clone(), Vec::new())
        }
        Assisted::Rows(x) => {
            one(&COUNT, Some(x.count.to_string()));
            (x.notices.clone(), vec![
                (&IDS, x.rows.iter().map(|r| r.id.clone()).collect()),
                (&TITLES, x.rows.iter().map(|r| r.title.clone()).collect()),
            ])
        }
        Assisted::Plan(x) => {
            one(&SUMMARY, Some(x.summary.clone()));
            let sittings = || x.blocks.iter().flat_map(|b| &b.assignments);
            (x.notices.clone(), vec![
                (&BLOCK_IDS, x.blocks.iter().map(|b| b.id.clone()).collect()),
                (&BLOCK_TITLES, x.blocks.iter().map(|b| b.title.clone()).collect()),
                (&SITTING_IDS, sittings().map(|s| s.id.clone()).collect()),
                (&SITTING_TITLES, sittings().map(|s| s.title.clone()).collect()),
            ])
        }
        Assisted::Filters(x) => (x.notices.clone(), vec![
            (&IDS, x.filters.iter().map(|f| f.id.clone()).collect()),
            (&NAMES, x.filters.iter().map(|f| f.name.clone()).collect()),
            (&QUERIES, x.filters.iter().map(|f| f.query.clone()).collect()),
        ]),
        Assisted::Choices(x) => (x.notices.clone(), vec![
            (&IDS, x.choices.iter().map(|c| c.id.clone()).collect()),
            (&TITLES, x.choices.iter().map(|c| c.title.clone()).collect()),
            (&REASONS, x.choices.iter().map(|c| c.reason.clone().unwrap_or_default()).collect()),
        ]),
        Assisted::Replan(x) => (x.notices.clone(), vec![(&SENTENCES, x.changes.iter().map(|c| c.sentence.clone()).collect())]),
        Assisted::DayPlan(x) => (x.notices.clone(), vec![(&SENTENCES, x.sittings.iter().map(|s| s.sentence.clone()).collect())]),
        Assisted::Timer(x) => {
            // The timer as it is names no sitting with none running.
            one(&SITTING_ID, Some(x.assignment.clone()).filter(|id| !id.is_empty()));
            one(&MINUTES, Some(x.minutes.to_string()));
            (x.notices.clone(), Vec::new())
        }
    };
    let mut all = vec![
        AutomationValue { name: ANNOUNCEMENT.0.to_owned(), values: vec![answer.announcement().to_owned()], several: false },
        AutomationValue { name: NOTICES.0.to_owned(), values: notices, several: true },
    ];
    all.append(&mut values);
    all.extend(several.into_iter().map(|(output, values)| AutomationValue { name: output.0.to_owned(), values, several: true }));
    Automated { announcement: answer.announcement().to_owned(), values: all }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::operations as table;

    #[test]
    fn what_asks_first_or_takes_a_proposal_is_never_offered_and_everything_else_is() {
        let offered: Vec<String> = operations().into_iter().map(|op| op.id).collect();
        for op in table() {
            let takes_a_proposal = op.parameters.iter().any(|p| !typed(p.kind));
            assert_eq!(offered.contains(&op.id), !op.confirms && !takes_a_proposal, "{}", op.id);
        }
        assert!(!offered.iter().any(|id| id.starts_with("erase")));
        assert!(!offered.iter().any(|id| id.starts_with("apply")));
    }

    #[test]
    fn every_operation_says_what_it_answers() {
        for op in table() {
            assert!(Answers::to(&op.id).is_some(), "{} answers nothing it says", op.id);
        }
    }

    #[test]
    fn every_variable_is_a_lower_case_name_and_named_once() {
        for op in table() {
            let outputs = outputs(&op.id);
            let mut names: Vec<&str> = outputs.iter().map(|o| o.name.as_str()).collect();
            assert!(names.iter().all(|n| n.chars().all(|c| c.is_ascii_lowercase() || c == '_') && n.len() > 2), "{}", op.id);
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), outputs.len(), "{}", op.id);
        }
    }

    #[test]
    fn text_is_typed_as_each_parameter_takes_it_and_empty_is_left_out() {
        let edit = operation("edit_task").unwrap();
        let given = |name: &str, text: &str| AutomationGiven { name: name.to_owned(), text: text.to_owned() };
        let arguments = arguments(&edit, vec![
            given("id", " 0190 "),
            given("priority", "p2"),
            given("labels", "@calls, home,"),
            given("notes", ""),
        ])
        .unwrap();
        assert_eq!(arguments.len(), 3);
        assert!(matches!(&arguments[0].value, ArgumentValue::Text(t) if t == "0190"));
        assert!(matches!(arguments[1].value, ArgumentValue::Number(2)));
        assert!(matches!(&arguments[2].value, ArgumentValue::Texts(l) if l == &["calls", "home"]));

        let refused = super::arguments(&edit, vec![given("priority", "5")]).unwrap_err();
        assert_eq!(refused.message(), "Priority takes 1 to 4, where 1 is highest, not 5.");
        let moving = operation("move_task").unwrap();
        assert!(matches!(super::arguments(&moving, vec![given("top", "Yes")]).unwrap()[0].value, ArgumentValue::Flag(true)));
        assert!(super::arguments(&moving, vec![given("top", "perhaps")]).is_err());
        let stopping = operation("stop_timer").unwrap();
        assert!(matches!(super::arguments(&stopping, vec![given("minutes", "1h30m")]).unwrap()[0].value, ArgumentValue::Number(90)));
        assert!(super::arguments(&stopping, vec![given("colour", "red")]).is_err());
    }

    #[test]
    fn saving_asks_for_what_is_needed_and_leaves_a_variable_to_be_checked_as_it_runs() {
        let given = |name: &str, text: &str| AutomationGiven { name: name.to_owned(), text: text.to_owned() };
        assert_eq!(check("edit_task", vec![given("priority", "2")]).unwrap_err().message(), "Task cannot be left empty.");
        assert!(check("edit_task", vec![given("id", "%task"), given("priority", "%priority")]).is_ok());
        assert!(check("edit_task", vec![given("id", "%task"), given("priority", "high")]).is_err());
        assert!(check("undo", Vec::new()).is_ok());
        assert!(check("erase_everything_deleted", Vec::new()).is_err());
    }

    #[test]
    fn an_action_is_summed_up_by_its_name_and_what_it_was_given() {
        let shown = vec![
            AutomationGiven { name: "id".to_owned(), text: "Write the report".to_owned() },
            AutomationGiven { name: "title".to_owned(), text: " ".to_owned() },
            AutomationGiven { name: "project".to_owned(), text: "Work".to_owned() },
        ];
        assert_eq!(summary("edit_task", shown.clone(), true), "Edit a task. Task: Write the report. Project: Work");
        assert_eq!(summary("edit_task", shown, false), "Edit a Task. Task: Write the report. Project: Work");
        assert_eq!(summary("erase_everything_deleted", Vec::new(), true), "");
    }

    #[test]
    fn a_field_says_whether_it_can_be_left_empty_and_what_offers_its_values() {
        let fields = fields("edit_task");
        let task = &fields[0];
        assert_eq!((task.title.as_str(), task.optional, task.choose.as_deref()), ("Task", false, Some("Choose a Task")));
        let labels = fields.iter().find(|f| f.name == "labels").unwrap();
        assert!(labels.several && labels.hint.ends_with("It can be left empty."));
        assert!(fields.iter().find(|f| f.name == "due").unwrap().day);
        assert!(super::fields("erase_task_permanently").is_empty());
    }
}
