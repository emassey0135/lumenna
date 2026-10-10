//! What every client's forms and pairing screens say: each field's name, what it takes, an
//! example; the pairing screen's sentences and buttons.
//!
//! The controls are each platform's; the words are not. Eleven clients had each written
//! their own, and the block form alone read "Takes tasks" in one and "Tasks can go here" in
//! another. A mnemonic or an access key is the platform's to add.

use serde::{Deserialize, Serialize};

use crate::actions::Choice;

/// How a form field's value is shaped, so a client offers the control that fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
        one_day: false,
    }
}

/// The task form's fields, in order.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn task_form() -> Vec<FormField> {
    use FieldKind as K;
    vec![
        field("title", "Title", K::Lines, "", "What to do"),
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
        field("project", "Project", K::Line, "", "Inbox"),
        field("labels", "Labels", K::Line, "Names separated by commas. A new name becomes a label.", "calls, errands"),
        field("notes", "Notes", K::Lines, "", ""),
    ]
}

/// The block form's fields, in order. `date` is a new block's day; an edit has none.
#[cfg_attr(feature = "uniffi", uniffi::export)]
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
        field("task_filter", "Tasks from", K::Line, "Which tasks it is offered, such as #Work. Empty for all.", "#Work"),
        field("colour", "Colour", K::Line, crate::actions::COLOUR, "teal"),
        field("notes", "Notes", K::Lines, "", ""),
    ]
}

/// What the pairing screen says, every sentence and button of it. Only what names the
/// device is the client's: how it calls itself where it finds the other by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
    /// What leaving that field empty does, where the clipboard can be read.
    pub empty_means: String,
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
#[cfg_attr(feature = "uniffi", uniffi::export)]
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
        empty_means: "Left empty, the code on the clipboard is used.".to_owned(),
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

/// Text with no one's own words in it — a button, a fixed title — in sentence case, for a
/// platform whose convention it is (Material, the web) and for braille, where every capital
/// costs a cell. Never for a task's title or a name, whose capitals are their own.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn sentence_case(text: String) -> String {
    // An acronym keeps its capitals: "JSON" is a name, not a word.
    let lower = |word: &str| {
        let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
        if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) { word.to_owned() } else { word.to_lowercase() }
    };
    let mut words = text.split(' ');
    let first = words.next().unwrap_or_default().to_owned();
    std::iter::once(first).chain(words.map(lower)).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

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
