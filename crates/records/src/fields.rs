//! What every client's forms and pairing screens say: each field's name, what it takes, an
//! example; the pairing screen's sentences and buttons.
//!
//! The controls are each platform's; the words are not. Eleven clients had each written
//! their own, and the block form alone read "Takes tasks" in one and "Tasks can go here" in
//! another. A mnemonic or an access key is the platform's to add.

use serde::{Deserialize, Serialize};

use crate::actions::Choice;
use crate::types::Syntax;

/// How a form field's value is shaped, so a client offers the control that fits.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// A line of text, read by the core: a date, a repetition, a duration, names.
    Line,
    /// Text that may run to several lines.
    Lines,
    /// One of its `options`.
    Choice,
    /// On or off.
    Toggle,
    /// A time of day, `HH:MM`; a client may offer a time picker, or a line the core reads.
    Time,
    /// A day, ISO; a client may offer a date picker, or a line the core reads.
    Date,
    /// A whole number of minutes.
    Minutes,
}

/// One field of a form.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormField {
    /// Which field of [`TaskFields`](crate::TaskFields) or [`BlockFields`](crate::BlockFields)
    /// it is, by its field name; `date` for a new block's day.
    pub key: String,
    /// Its name: "Due".
    pub label: String,
    /// What it takes, for under the field or as its description. Empty for nothing to say.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hint: String,
    /// A placeholder showing the shape of an answer. Empty for none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub example: String,
    /// The control that fits.
    pub kind: FieldKind,
    /// For a choice, what to choose from: `id` is the value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Choice>,
    /// For a block: shown only while it repeats.
    #[serde(default)]
    pub repeating_only: bool,
    /// When it is written in the filter language (a block's Tasks from), which a client offers
    /// completion in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax: Option<Syntax>,
    /// For a block: whether one day of a repeating block can be changed apart from the rest
    /// (`day_block_fields`); the others belong to the series.
    #[serde(default)]
    pub one_day: bool,
}

fn field(key: &str, label: &str, kind: FieldKind, hint: &str, example: &str) -> FormField {
    FormField {
        key: key.to_owned(),
        label: label.to_owned(),
        hint: hint.to_owned(),
        example: example.to_owned(),
        kind,
        options: Vec::new(),
        repeating_only: false,
        syntax: None,
        one_day: false,
    }
}

/// The task form's fields, in order.
#[must_use]
pub fn task_form() -> Vec<FormField> {
    use FieldKind as K;
    vec![
        // One line: a title is short, and Enter saves.
        field("title", "Title", K::Line, "", "What to do"),
        field(
            "due",
            "Due",
            K::Line,
            "A date, such as tomorrow or next Friday, with a time if it has one. Empty for none. A new date keeps how it repeats.",
            "tomorrow",
        ),
        field(
            "repeat",
            "Repeats",
            K::Line,
            "Such as every Monday, or every! 2 weeks to count from when it is done. Empty for no repetition.",
            "every monday",
        ),
        FormField { options: crate::form::priorities(), ..field("priority", "Priority", K::Choice, "", "") },
        field("estimate", "Estimate", K::Line, "How long it should take, such as 45m or 1h30m. Empty for none.", "45m"),
        // A list, filled from `Lumenna::project_options`: a task only ever moves to a project
        // that exists, so a list cannot hold a typo, and typing still finds one.
        field("project", "Project", K::Choice, "", ""),
        field("labels", "Labels", K::Line, "Names separated by commas. A new name becomes a label.", "calls, errands"),
        field("notes", "Notes", K::Lines, "", ""),
    ]
}



/// The block form's fields, in order. `date` is a new block's day; an edit has none.
#[must_use]
pub fn block_form() -> Vec<FormField> {
    use FieldKind as K;
    let kinds = [("work", "Work"), ("break", "Break"), ("event", "Event")]
        .into_iter()
        .map(|(id, title)| Choice { id: id.to_owned(), title: title.to_owned(), ..Choice::default() })
        .collect();
    let repeating = |f: FormField| FormField { repeating_only: true, ..f };
    // What `day_block_fields` holds, and an occurrence's edit can change.
    let day = |f: FormField| FormField { one_day: true, ..f };
    vec![
        day(field("title", "Name", K::Line, "", "Deep work")),
        field("date", "Day", K::Date, "The day it happens, or the first day it repeats.", "today"),
        day(field("start", "Starts at", K::Time, "A time, such as 9am or 14:30.", "9am")),
        day(field("minutes", "Lasts, in minutes", K::Minutes, "", "60")),
        FormField {
            options: kinds,
            one_day: true,
            ..field("kind", "Kind", K::Choice, "Changing it sets the three choices after it to the kind's own.", "")
        },
        day(field("accepts_tasks", "Takes tasks", K::Toggle, "", "")),
        day(field("counts_capacity", "Counts toward hours for work", K::Toggle, "", "")),
        day(field("anchored", "Anchored, never moved when the day slips", K::Toggle, "", "")),
        field("repeat", "Repeats", K::Line, "Such as every weekday. Empty for once.", "every weekday"),
        repeating(field("until", "Until", K::Line, "The last day it happens. Empty for no end.", "31 January")),
        field(
            "min_minutes",
            "Shortest length, in minutes",
            K::Minutes,
            "How short a slipping day may make it. Empty for the kind's own.",
            "30",
        ),
        FormField {
            syntax: Some(Syntax::Filter),
            ..field("task_filter", "Tasks from", K::Line, "Which tasks it is offered, such as #Work. Empty for all.", "#Work")
        },
        FormField { options: crate::colours::colour_choices(None), ..field("colour", "Colour", K::Choice, crate::actions::COLOUR, "") },
        field("notes", "Notes", K::Lines, "", ""),
    ]
}

/// What the block form says under Repeats when the block repeats by a rule the repetition
/// words cannot say, so the field starts empty and saving leaves the rule alone.
#[must_use]
pub fn unsayable_repeat_note(block: crate::BlockShown) -> Option<String> {
    let rule = block.rrule.filter(|_| block.repetition.is_none())?;
    Some(repeat_rule_note(&rule))
}

/// What a block's Repeats field says of a rule it repeats by that the repetition words cannot
/// say, which an empty field keeps.
#[must_use]
pub fn repeat_rule_note(rule: &str) -> String {
    format!("It repeats by the rule {rule}, which the repetition words cannot say. Leave Repeats empty to keep it.")
}

/// The button that saves a form: "Add" for something new, "Save" for a change.
#[must_use]
pub fn form_yes(adding: bool) -> String {
    if adding { "Add" } else { "Save" }.to_owned()
}

/// What the pairing screen says, every sentence and button of it. Only what names the
/// device is the client's: how it calls itself where it finds the other by itself.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingWords {
    /// The screen's title.
    pub title: String,
    /// What it says before anything starts.
    pub intro: String,
    /// The button that waits to be found, and shows a code.
    pub wait: String,
    /// The button that joins with the other device's code.
    pub join: String,
    /// This device's code, as a label.
    pub my_code: String,
    /// The field for the other device's code.
    pub their_code: String,
    /// Said once a session opens to wait.
    pub opening: String,
    /// Said once it dials the other device.
    pub connecting: String,
    /// Said when a code is entered while waiting: the wait stops first.
    pub switching: String,
    /// Said when it waits, the code shown.
    pub waiting: String,
    /// Said when the code was copied.
    pub copied: String,
    /// Said when the code field is empty and nothing is on the clipboard.
    pub need_code: String,
    /// The question once both sides have words.
    pub match_title: String,
    /// What it says under the question, before the words.
    pub match_message: String,
    /// The answer that they match.
    pub match_yes: String,
    /// The answer that they differ.
    pub match_no: String,
    /// Said once the words are confirmed, while the devices finish pairing.
    pub finishing: String,
    /// Said once the words are refused, while the other device is told.
    pub refusing: String,
    /// The button that copies this device's code.
    pub copy_code: String,
    /// [`intro`](Self::intro) in sentence case, its buttons named as they read there.
    pub intro_sentence: String,
}

/// The pairing screen's words. `this_device` is how the device calls itself ("this Mac",
/// "this PC", "this phone"); `local` whether it can be found on the network by itself, which a
/// browser cannot.
#[must_use]
pub fn pairing_words(this_device: String, local: bool) -> PairingWords {
    let waiting = if local {
        format!(
            "Waiting for the other device. On this network it finds {this_device} by itself. On another network, \
             enter this code on the other device. Waiting up to ten minutes."
        )
    } else {
        "Waiting for the other device. Enter this code on the other device. Waiting up to ten minutes.".to_owned()
    };
    let intro = if local {
        "On the same network, start pairing on both devices and they find each other: choose Wait for the Other \
         Device here, and pair on the other one too. On different networks, one shows a code and the other enters it."
    } else {
        "One device shows a code and the other enters it: choose Wait for the Other Device to show one here, or \
         enter the code the other device shows."
    };
    PairingWords {
        title: "Pair a Device".to_owned(),
        intro: intro.to_owned(),
        wait: "Wait for the Other Device".to_owned(),
        join: "Pair Using This Code".to_owned(),
        my_code: "This device's code".to_owned(),
        their_code: "Code from the other device".to_owned(),
        opening: "Opening a pairing session.".to_owned(),
        connecting: "Connecting to the other device.".to_owned(),
        switching: "Stopping the wait, then connecting with this code.".to_owned(),
        waiting,
        copied: "The code is copied, so it can be pasted on the other device.".to_owned(),
        need_code: "Type or paste the code the other device shows.".to_owned(),
        match_title: "Do These Words Match?".to_owned(),
        match_message: "The other device shows three words too. Pair only if they are the same, in the same order:".to_owned(),
        match_yes: "Yes, They Match".to_owned(),
        match_no: "No, They Differ".to_owned(),
        finishing: "The words match. Finishing pairing.".to_owned(),
        refusing: "The words differ, so the devices are not paired.".to_owned(),
        copy_code: "Copy Code".to_owned(),
        intro_sentence: intro.replace("Wait for the Other Device", "Wait for the other device"),
    }
}

/// What a shell's own day or time picker says, where the platform's control does not say it
/// itself: the button beside a line that opens a calendar or a clock, the picker's answer and
/// cancel, and a time chosen as hours and minutes. A picker only fills the line beside it, which
/// stays the way in, so its words are few. Title Case; `sentence_case` for the others.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PickerWords {
    /// The button that opens a calendar: "Choose a Day".
    pub day: String,
    /// The button that opens a clock: "Choose a Time".
    pub time: String,
    /// The picker's answer, which puts what it holds in the line.
    pub choose: String,
    /// The picker's cancel, which leaves the line as it was.
    pub cancel: String,
    /// A time's hours, where a time is chosen as two numbers.
    pub hours: String,
    /// A time's minutes.
    pub minutes: String,
}

/// What a shell's own day or time picker says (`PickerWords`).
#[must_use]
pub fn picker_words() -> PickerWords {
    PickerWords {
        day: "Choose a Day".to_owned(),
        time: "Choose a Time".to_owned(),
        choose: "Choose".to_owned(),
        cancel: "Cancel".to_owned(),
        hours: "Hours".to_owned(),
        minutes: "Minutes".to_owned(),
    }
}

/// Text with no one's own words in it — a button, a fixed title — in sentence case, for a
/// platform whose convention it is (Material, the web) and for braille, where every capital
/// costs a cell. Never for a task's title or a name, whose capitals are their own.
#[must_use]
pub fn sentence_case(text: String) -> String {
    // An acronym keeps its capitals: "JSON" is a name, not a word. So does "I", and "I'm".
    let lower = |word: &str| {
        let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
        let pronoun = word.trim_matches(|c: char| !c.is_alphabetic()) == "I" || word.starts_with("I'") || word.starts_with("I\u{2019}");
        if pronoun || (letters.len() > 1 && letters.iter().all(|c| c.is_uppercase())) { word.to_owned() } else { word.to_lowercase() }
    };
    let mut words = text.split(' ');
    let first = words.next().unwrap_or_default().to_owned();
    std::iter::once(first).chain(words.map(lower)).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_case_keeps_the_pronoun_i_and_acronyms() {
        assert_eq!(sentence_case("What Should I Work On?".to_owned()), "What should I work on?");
        assert_eq!(sentence_case("Export As JSON".to_owned()), "Export as JSON");
        assert_eq!(sentence_case("Where I'm Going".to_owned()), "Where I'm going");
        assert_eq!(sentence_case("Mark Done".to_owned()), "Mark done");
    }

    #[test]
    fn every_task_field_and_block_field_has_a_form_field() {
        let task = serde_json::to_value(crate::TaskFields::default()).unwrap();
        let named: Vec<String> = task_form().into_iter().map(|f| f.key).collect();
        for key in task.as_object().unwrap().keys() {
            assert!(named.contains(key), "the task form says nothing of {key}");
        }
        let block = serde_json::to_value(crate::BlockFields::default()).unwrap();
        let named: Vec<String> = block_form().into_iter().map(|f| f.key).collect();
        for key in block.as_object().unwrap().keys() {
            assert!(named.contains(key), "the block form says nothing of {key}");
        }
    }

    #[test]
    fn sentence_case_keeps_the_first_capital_only() {
        assert_eq!(sentence_case("Delete and Keep Its Tasks".to_owned()), "Delete and keep its tasks");
        assert_eq!(sentence_case("Export JSON, Complete".to_owned()), "Export JSON, complete");
    }
}

/// Every word of a proposal's screen: Replan from Now's or Plan the Day's.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalWords {
    /// The screen's title.
    pub title: String,
    /// What a change in the list is, as its check box's name says it: "Make this change".
    pub item: String,
    /// The button that makes the changes checked.
    pub apply: String,
    /// The button that closes the screen, making nothing.
    pub cancel: String,
    /// Under the list: what becomes of a change left unchecked.
    pub hint: String,
    /// Said, and shown in place of the list, when nothing is proposed.
    pub nothing: String,
    /// The heading over what does not fit.
    pub unfit: String,
    /// The heading over other options (Replan's `alternatives`), each a button that asks again
    /// with its pins. Empty where a screen has none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub others: String,
}

/// The words of `kind`'s screen: [`ActionKind::Replan`](crate::ActionKind::Replan) or
/// [`ActionKind::PlanDay`](crate::ActionKind::PlanDay); any other kind has Replan's.
#[must_use]
pub fn proposal_words(kind: crate::ActionKind) -> ProposalWords {
    match kind {
        crate::ActionKind::PlanDay => ProposalWords {
            title: "Plan the Day".to_owned(),
            item: "Plan this".to_owned(),
            apply: "Plan the Ones Checked".to_owned(),
            cancel: "Cancel".to_owned(),
            hint: "A sitting left unchecked stays out of its block, and the day is planned again around it.".to_owned(),
            nothing: "There is nothing to plan.".to_owned(),
            unfit: "Due, and fits in no block".to_owned(),
            others: String::new(),
        },
        _ => ProposalWords {
            title: "Replan from Now".to_owned(),
            item: "Make this change".to_owned(),
            apply: "Make the Changes Checked".to_owned(),
            cancel: "Cancel".to_owned(),
            hint: "A change left unchecked is not made: what it would move stays where it is, and the rest of the day is \
                   proposed again around it."
                .to_owned(),
            nothing: "Nothing needs to change: the rest of the day still fits.".to_owned(),
            unfit: "Does not fit".to_owned(),
            others: "Other options".to_owned(),
        },
    }
}

/// When a new block on `date` (a date phrase, today when absent) starts unless the person says
/// otherwise, `HH:MM`: today, the next whole hour, never before the day starts; any other day,
/// when the day starts. A fixed 09:00 was over before most of the day was.
///
/// # Errors
///
/// If `date` cannot be read.
pub fn new_block_start(snapshot: &lumenna_core::snapshot::Snapshot, date: Option<&str>, now: &jiff::Zoned) -> crate::Result<String> {
    let day = crate::resolve::date(date, now, snapshot.settings.week_start)?;
    let day_start = snapshot.settings.day_window.0;
    let start = if day == now.date() {
        // The hour after this one, or the last hour of the day once there is no next one today.
        let next = jiff::civil::time((now.hour() + 1).min(23), 0, 0, 0);
        next.max(day_start)
    } else {
        day_start
    };
    Ok(crate::words::time_text(start))
}

/// What saving a form says when nothing in it changed: the task details', the block form's.
#[must_use]
pub fn nothing_changed() -> String {
    "Nothing changed".to_owned()
}

/// Which occurrences of the repeating block `block` a change is for, asked before its form
/// opens: that day alone (`day`, as said: "Tomorrow", "Monday 5 October", or empty for "this
/// day"), whose answer is
/// `day`, or every occurrence, `every`. In Title Case, or sentence case where `sentence`; the
/// block's name and the day keep their own capitals either way.
#[must_use]
pub fn edit_which_question(block: &str, day: &str, sentence: bool) -> crate::Question {
    let cased = |text: &str| if sentence { sentence_case(text.to_owned()) } else { text.to_owned() };
    crate::Question::Choose {
        title: format!("{} {block}", cased("Change")),
        message: cased("Which Occurrences?"),
        answers: vec![
            Choice {
                id: "day".to_owned(),
                // A block with no day of its own is this one.
                title: if day.trim().is_empty() {
                    cased("This Day Only")
                } else {
                    format!("{day} {}", if sentence { "only" } else { "Only" })
                },
                ..Choice::default()
            },
            Choice { id: "every".to_owned(), title: cased("Every Occurrence"), ..Choice::default() },
        ],
    }
}

/// The block form's heading for one day of a repeating block, for every occurrence of one, and
/// for a block that does not repeat: "Change Standup, This Day Only", or in sentence case where
/// `sentence`, the block's name keeping its own capitals.
#[must_use]
pub fn block_form_heading(block: &str, scope: BlockFormScope, sentence: bool) -> String {
    let cased = |text: &str| if sentence { sentence_case(text.to_owned()) } else { text.to_owned() };
    match scope {
        BlockFormScope::New => cased("New Block"),
        BlockFormScope::Day => format!("{} {block}, {}", cased("Change"), if sentence { "this day only" } else { "This Day Only" }),
        BlockFormScope::Every => format!("{} {block}, {}", cased("Change"), if sentence { "every occurrence" } else { "Every Occurrence" }),
        BlockFormScope::Whole => format!("{} {block}", cased("Change")),
    }
}

/// What a block form is for, as its heading says it.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockFormScope {
    /// A new block.
    New,
    /// One day of a repeating block.
    Day,
    /// Every occurrence of a repeating block.
    Every,
    /// A block that does not repeat.
    Whole,
}

/// One line the task details list besides their form: its label, then what it says.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailsLine {
    /// What it is: `state`, `rule`, `waits`, `history`, so a shell can place it.
    pub key: String,
    /// Its label: "State".
    pub label: String,
    /// What it says: "overdue, recurring".
    pub value: String,
}

/// `text` as written where `sentence` says: sentence case, else as given (Title Case).
fn cased(text: &str, sentence: bool) -> String {
    if sentence { sentence_case(text.to_owned()) } else { text.to_owned() }
}

/// What the task details say around their form, every label and button: the same on every
/// platform. The buttons a details pane shows by themselves are the task's `primary` offered
/// actions, the rest a menu away, as on any row; Waits For's two are its own.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailsWords {
    /// The pane's name, for a screen reader: "Task Details".
    pub title: String,
    /// What it shows with no task in hand.
    pub none: String,
    /// What it says once the task it showed is gone: the core's [`no_longer_here`](crate::no_longer_here).
    pub gone: String,
    /// The list of what the task waits for: its label.
    pub waits: String,
    /// The same list's description.
    pub waits_hint: String,
    /// What that list says with nothing in it.
    pub waits_none: String,
    /// The button that adds something it waits for (`WaitFor`), which asks.
    pub add_wait: String,
    /// The button that stops it waiting for the task in hand in that list (`StopWaiting`).
    pub stop_wait: String,
    /// The heading over what it is: its state and history (`DetailsView::lines`).
    pub about: String,
    /// The heading over what can be done to it.
    pub actions: String,
    /// The button that saves what changed.
    pub save: String,
}

/// The task details' words (`DetailsWords`), in sentence case where `sentence`.
#[must_use]
pub fn details_words(sentence: bool) -> DetailsWords {
    let c = |text: &str| cased(text, sentence);
    DetailsWords {
        title: c("Task Details"),
        none: "No task selected".to_owned(),
        gone: crate::no_longer_here(),
        waits: c("Waits For"),
        waits_hint: "The tasks this one waits for. It is blocked until they are done.".to_owned(),
        waits_none: "Nothing".to_owned(),
        add_wait: c("Wait for Another Task"),
        stop_wait: c("Stop Waiting for the Selected Task"),
        about: c("About"),
        actions: c("Actions"),
        save: c("Save"),
    }
}

/// What the details list of `task` besides its form, in order: its state ("open" when nothing
/// is worth saying), the rule it repeats by when the Repeats field cannot say it, what it waits
/// for, and its history. A line with nothing to say is left out, but for the state. Labels in
/// sentence case where `sentence`.
#[must_use]
pub fn details_lines(task: &crate::types::TaskDetail, sentence: bool) -> Vec<DetailsLine> {
    let line = |key: &str, label: &str, value: String| DetailsLine { key: key.to_owned(), label: cased(label, sentence), value };
    let states: Vec<&str> = task.state.iter().map(String::as_str).filter(|state| *state != "ready").collect();
    let mut lines = vec![line("state", "State", if states.is_empty() { "open".to_owned() } else { states.join(", ") })];
    if task.repetition.is_none()
        && let Some(rule) = &task.recurrence
    {
        lines.push(line("rule", "Repeats by the Rule", rule.clone()));
    }
    if !task.depends.is_empty() {
        lines.push(line("waits", "Waits For", crate::words::listed(&task.depends.iter().map(|d| d.title.clone()).collect::<Vec<_>>())));
    }
    if !task.history.is_empty() {
        lines.push(line("history", "History", task.history.join(", ")));
    }
    lines
}

/// Put in a Block's last choice, which asks for another day and offers that day's blocks.
#[must_use]
pub fn another_day() -> String {
    "Another Day".to_owned()
}

/// A button a settings page offers beside its settings: Back Up Now, Import or Restore.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsButton {
    /// What it does: `back-up`, `restore-backup`, `import`, `sync-now`, `pair`.
    pub id: String,
    /// Its name.
    pub title: String,
    /// Whether it asks something first (a file, a folder), which a platform marks its own way.
    pub asks: bool,
}

/// One page of settings, as every settings screen groups them: in order, each setting on
/// exactly one page.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsPage {
    /// What it is: `general`, `planning`, `reminders`, `devices`, `backups`, `export`.
    pub id: String,
    /// Its title: "Planning".
    pub title: String,
    /// What is on it, for a list of pages that shows a second line: "The day, the week,
    /// completing subtasks, announcements".
    pub description: String,
    /// What it says under its settings: "These sync to all your devices." Empty for nothing.
    pub caption: String,
    /// Its settings, by key, in order. One the store does not list here (the background
    /// setting, where the shell keeps nothing running) is left out.
    pub keys: Vec<String>,
    /// Its buttons, in order.
    pub buttons: Vec<SettingsButton>,
}

/// Every settings page, in order. `own_clock` leaves out the `clock` setting, for a shell that
/// follows its platform's clock; a terminal, which has none, shows it. Titles and buttons in
/// sentence case where `sentence`.
#[must_use]
pub fn settings_pages(own_clock: bool, sentence: bool) -> Vec<SettingsPage> {
    let c = |text: &str| cased(text, sentence);
    let button = |id: &str, title: &str, asks: bool| SettingsButton { id: id.to_owned(), title: c(title), asks };
    let keys = |keys: &[&str]| keys.iter().filter(|key| !(own_clock && **key == "clock")).map(|key| (*key).to_owned()).collect();
    let page = |id: &str, title: &str, description: &str, caption: &str, members: Vec<String>, buttons: Vec<SettingsButton>| SettingsPage {
        id: id.to_owned(),
        title: c(title),
        description: description.to_owned(),
        caption: caption.to_owned(),
        keys: members,
        buttons,
    };
    vec![
        page("general", "General", "On this device: running in the background, the clock", "These settings are this device's alone.", keys(&[crate::background::KEY, "clock"]), Vec::new()),
        page(
            "planning",
            "Planning",
            "The day, the week, completing subtasks, announcements",
            "These sync to all your devices.",
            keys(&[
                "day-start",
                "day-end",
                "week-start",
                "task-order",
                "show-completed-subtasks",
                "cascade-complete-subtasks",
                "verbosity",
                "urgency-weights",
            ]),
            Vec::new(),
        ),
        page(
            "reminders",
            "Reminders",
            "When reminders come, and whether this device gives them",
            "The defaults sync to all your devices. Whether reminders are given here is this device's alone.",
            keys(&["reminders", "default-task-reminders", "default-block-reminders", "all-day-reminder-hour"]),
            Vec::new(),
        ),
        page("devices", "Devices and Sync", "Paired devices and how syncing is going", "", Vec::new(), vec![button("sync-now", "Sync Now", false), button("pair", "Pair a Device", true)]),
        page(
            "backups",
            "Backups",
            "On this device only",
            "These settings are this device's alone.",
            keys(&["backup-every", "backup-keep", "backup-dir"]),
            vec![button("back-up", "Back Up Now", false), button("restore-backup", "Restore from a Backup", true)],
        ),
        page(
            "export",
            "Export and Import",
            "JSON, Markdown, org, calendar; erasing what was deleted",
            &export_caption(),
            Vec::new(),
            vec![button("import", "Import or Restore", true)],
        ),
    ]
}

/// What the Export and Import page says under its buttons.
#[must_use]
pub fn export_caption() -> String {
    "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a backup adds what this \
     device lacks and removes nothing."
        .to_owned()
}

/// What a setting of each shape says it takes, where a shell offers its own control: a time's
/// hint and example, a number's.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingWords {
    /// A time setting's hint, the same as its own: "A time, such as 8:00 or 8am."
    pub time_hint: String,
    /// A time setting's example, for a placeholder: "8am".
    pub time_example: String,
    /// A number setting's hint: "A whole number."
    pub number_hint: String,
    /// The button that applies a setting typed in a dialog of its own.
    pub set: String,
    /// The button beside a folder setting that opens the platform's folder dialog.
    pub choose: String,
    /// That dialog's title.
    pub choose_folder: String,
    /// The Devices page's line of how syncing is going (`DevicesView::status`): its label.
    pub sync_status: String,
    /// The Devices page's list of devices: its label.
    pub devices: String,
    /// The Devices page's button that renames the device held, which asks.
    pub rename: String,
    /// The Devices page's button that unpairs the device held, which asks.
    pub unpair: String,
}

/// What a setting's own control says (`SettingWords`), Title Case.
#[must_use]
pub fn setting_words() -> SettingWords {
    SettingWords {
        time_hint: TIME_HINT.to_owned(),
        time_example: "8am".to_owned(),
        number_hint: "A whole number.".to_owned(),
        set: "Set".to_owned(),
        choose: "Choose".to_owned(),
        choose_folder: "Choose a Folder".to_owned(),
        sync_status: "Sync Status".to_owned(),
        devices: "Paired Devices".to_owned(),
        rename: "Rename".to_owned(),
        unpair: "Unpair".to_owned(),
    }
}

/// What every time setting says it takes.
pub const TIME_HINT: &str = "A time, such as 8:00 or 8am.";

/// What is said once a backup is taken into a folder: "Backed up to Documents as
/// lumenna-2026-10-26.lumbak".
#[must_use]
pub fn backed_up_to(folder: &str, name: &str) -> String {
    format!("Backed up to {folder} as {name}")
}

/// What quick add and a list's filter say, every label and button, the same on every platform.
/// Title Case for headings and buttons; `quick_add_words(true)` for sentence case.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAddWords {
    /// Quick add's title: "New Task".
    pub title: String,
    /// Its field's label: "Task".
    pub label: String,
    /// What its field takes, for under it or as its description.
    pub hint: String,
    /// The field's placeholder.
    pub example: String,
    /// The label of what it will add, read back as typed: "Will Add".
    pub will_add: String,
    /// The button that adds it.
    pub add: String,
    /// The button that closes it.
    pub cancel: String,
    /// A list's filter field: its label.
    pub filter: String,
    /// What the filter field takes, for its placeholder or description.
    pub filter_hint: String,
    /// What is said when completion offers nothing where the cursor is.
    pub nothing_to_complete: String,
    /// The button or menu that offers what could come next.
    pub show_completions: String,
    /// How a line that completes offers what fits, where a platform says it as the field's help:
    /// "Down arrow offers what could come next."
    pub completes: String,
}

/// What quick add and a list's filter say (`QuickAddWords`), in sentence case where `sentence`.
#[must_use]
pub fn quick_add_words(sentence: bool) -> QuickAddWords {
    let c = |text: &str| cased(text, sentence);
    QuickAddWords {
        title: c("New Task"),
        label: "Task".to_owned(),
        hint: "Such as: write the chapter tomorrow p1 #Work. Down arrow offers what could come next.".to_owned(),
        example: "call the bank tomorrow at 3pm p1 #Home @calls".to_owned(),
        will_add: c("Will Add"),
        add: c("Add"),
        cancel: c("Cancel"),
        filter: c("Filter"),
        filter_hint: "#Work & overdue, or search: words".to_owned(),
        nothing_to_complete: "Nothing to complete here.".to_owned(),
        show_completions: c("Show Completions"),
        completes: "Down arrow offers what could come next.".to_owned(),
    }
}

/// A completion offered as a button or a menu item names what it puts in: "Complete with Work,
/// project", `line` being the completion's line (`speech::candidate`).
#[must_use]
pub fn complete_with(line: &str) -> String {
    format!("Complete with {line}")
}

/// What a device says when it cannot give reminders as asked, and the buttons that fix it, for
/// its reminders' settings; and its notification channel's name and description, where the
/// platform has channels (Android).
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationWords {
    /// Notifications are off for Lumenna on this device.
    pub off: String,
    /// The button that asks to allow them.
    pub allow: String,
    /// Alarms for an exact time are not allowed, so reminders may come late.
    pub late: String,
    /// The button that asks to allow exact alarms.
    pub allow_exact: String,
    /// The notification channel's name.
    pub channel: String,
    /// The notification channel's description.
    pub channel_description: String,
}

/// What a device says of its notifications (`NotificationWords`), `this_device` being how it
/// calls itself ("this device", "this watch"), buttons in sentence case where `sentence`.
#[must_use]
pub fn notification_words(this_device: &str, sentence: bool) -> NotificationWords {
    let c = |text: &str| cased(text, sentence);
    NotificationWords {
        off: format!("Notifications are off for Lumenna, so no reminder can be shown on {this_device}."),
        allow: c("Allow Notifications"),
        late: "Reminders may come a few minutes late: Lumenna is not allowed to set alarms for an exact time.".to_owned(),
        allow_exact: c("Allow Exact Alarms"),
        channel: "Reminders".to_owned(),
        channel_description: "Tasks coming due and blocks starting or ending, as you set them.".to_owned(),
    }
}

/// The question pairing asks once both devices show their words, whole: what it says, then the
/// words in order. "The other device shows three words too. … same order: apple, river, stone."
#[must_use]
pub fn pairing_match(words: &PairingWords, three: &[String]) -> String {
    format!("{} {}.", words.match_message, three.join(", "))
}

/// This device's code, as a line: "This device's code: 1234-5678".
#[must_use]
pub fn my_code_line(words: &PairingWords, code: &str) -> String {
    format!("{}: {code}", words.my_code)
}

/// The lengths a question asking how long offers, the usual ones a short list holds, each its
/// answer (`45m`) and its words: and None first where the question may be left empty.
#[must_use]
pub fn usual_lengths(optional: bool) -> Vec<Choice> {
    let none = optional.then(|| Choice { id: String::new(), title: "None".to_owned(), ..Choice::default() });
    none.into_iter()
        .chain([15, 25, 30, 45, 60, 90, 120].into_iter().map(|minutes| Choice {
            id: format!("{minutes}m"),
            title: crate::words::capitalised(&crate::words::duration(minutes)),
            ..Choice::default()
        }))
        .collect()
}

/// A setting or field that is on or off, as a choice of two: "Yes", "No", for a shell whose
/// form asks it so (BTSpeak's block form).
#[must_use]
pub fn yes_no() -> Vec<Choice> {
    [("yes", "Yes"), ("no", "No")].into_iter().map(|(id, title)| Choice { id: id.to_owned(), title: title.to_owned(), ..Choice::default() }).collect()
}

/// What a shell's option of reading a quick-add line back before adding it says, where it
/// offers one (BTSpeak): its name with its state, what turning it says, and the question.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadBackWords {
    /// The option while on: "Read a task back before adding it, on".
    pub title_on: String,
    /// The option while off.
    pub title_off: String,
    /// Said once turned on.
    pub turned_on: String,
    /// Said once turned off.
    pub turned_off: String,
    /// The answer that adds the line read back.
    pub add: String,
    /// The answer that goes back to change it.
    pub change: String,
}

/// What reading a line back says (`ReadBackWords`), its answers in sentence case where `sentence`.
#[must_use]
pub fn read_back_words(sentence: bool) -> ReadBackWords {
    ReadBackWords {
        title_on: "Read a task back before adding it, on".to_owned(),
        title_off: "Read a task back before adding it, off".to_owned(),
        turned_on: "A task is read back before it is added".to_owned(),
        turned_off: "A task is added as soon as it is entered".to_owned(),
        add: cased("Add It", sentence),
        change: cased("Change It", sentence),
    }
}

/// The question a line read back asks: "New task Pay rent, due tomorrow. Add it?", `readback`
/// being quick add's (`LineView::readback`).
#[must_use]
pub fn add_it(readback: &str) -> String {
    format!("{}. Add it?", readback.trim_end_matches('.'))
}

/// What a shell's own file and folder dialogs ask, where it words them (BTSpeak), and the
/// export's button.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWords {
    /// Choosing a backup to restore: "Restore which backup?"
    pub restore: String,
    /// Choosing where an export or a backup goes: "Save it in which folder?"
    pub save_in: String,
    /// Choosing a file to import: "Import which file?"
    pub import: String,
    /// The button or item that exports.
    pub export: String,
}

/// What a shell's own file dialogs ask (`FileWords`), the button in sentence case where `sentence`.
#[must_use]
pub fn file_words(sentence: bool) -> FileWords {
    FileWords {
        restore: "Restore which backup?".to_owned(),
        save_in: "Save it in which folder?".to_owned(),
        import: "Import which file?".to_owned(),
        export: cased("Export", sentence),
    }
}

/// What backing up into a folder of the person's says where the device keeps none of its own (a
/// browser): what a backup is, what the browser takes by itself, and each button and sentence.
/// Buttons in sentence case where `sentence`.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderWords {
    /// What a backup is.
    pub intro: String,
    /// Where a folder can be given: what the device then takes by itself.
    pub takes_itself: String,
    /// Where none can: that it takes none by itself.
    pub takes_none: String,
    /// What restoring does.
    pub restoring: String,
    /// The section's name.
    pub section: String,
    /// The button that backs up into the folder now.
    pub back_up_now: String,
    /// The button that chooses a folder while one is chosen.
    pub choose_another: String,
    /// The button that stops backing up into a folder.
    pub stop: String,
    /// The button that chooses a folder while none is.
    pub choose: String,
    /// Said when asking again was refused.
    pub still_not_allowed: String,
    /// Said once stopped.
    pub stopped: String,
}

/// What backing up into a folder says (`FolderWords`), `this_device` being how the device calls
/// itself ("this browser").
#[must_use]
pub fn folder_words(this_device: &str, sentence: bool) -> FolderWords {
    let c = |text: &str| cased(text, sentence);
    let device = crate::words::capitalised(this_device);
    FolderWords {
        intro: "A backup is everything, its whole history and the trash included.".to_owned(),
        takes_itself: format!(
            "Choose a folder below and {this_device} takes them by itself; or download one now and then, and keep it somewhere safe."
        ),
        takes_none: format!("{device} takes none by itself: download one now and then, and keep it somewhere safe."),
        restoring: format!("Restoring merges: it adds what {this_device} lacks, and never takes back a later change."),
        section: c("Automatic Backups"),
        back_up_now: c("Back Up to the Folder Now"),
        choose_another: c("Choose Another Folder"),
        stop: c("Stop Backing Up to a Folder"),
        choose: c("Choose a Folder for Backups"),
        still_not_allowed: "Still not allowed".to_owned(),
        stopped: "No longer backing up to a folder".to_owned(),
    }
}

/// Where backups go, while a folder is chosen and allowed: "Backing up to the folder Documents."
#[must_use]
pub fn backing_up_to_folder(folder: &str) -> String {
    format!("Backing up to the folder {folder}.")
}

/// A folder chosen that must be allowed again first, `this_device` being how the device calls
/// itself: "This browser needs asking again before it backs up to Documents."
#[must_use]
pub fn needs_asking_again(this_device: &str, folder: &str) -> String {
    format!("{} needs asking again before it backs up to {folder}.", crate::words::capitalised(this_device))
}

/// The button that allows backups into a folder again: "Allow Backups to Documents".
#[must_use]
pub fn allow_backups_to(folder: &str, sentence: bool) -> String {
    format!("{} {folder}", cased("Allow Backups to", sentence))
}

/// Said once backups go into a folder: "Backing up to Documents".
#[must_use]
pub fn backing_up_to(folder: &str) -> String {
    format!("Backing up to {folder}")
}
