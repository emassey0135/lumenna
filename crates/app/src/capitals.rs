//! The fixed words of a view in the shell's capitals: Title Case on Apple, Windows, GTK and
//! Emacs, sentence case on Android, Wear OS, the web and BTSpeak, where every capital also costs
//! a braille cell. Names keep their capitals: the core marks what is a name, or words the
//! sentence-case title itself.

use lumenna_records::actions::{Action, Question};
use lumenna_records::sentence_case;
use lumenna_records::places::SidebarKind;
use serde::{Deserialize, Serialize};

use crate::ViewModel;

/// How a shell writes its buttons, headings and questions.
#[boltffi::data]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capitals {
    /// "Mark Done", "Saved Filters": the core's own.
    #[default]
    Title,
    /// "Mark done", "Saved filters".
    Sentence,
}

/// `view` with its fixed words in sentence case.
pub(crate) fn in_sentence_case(view: &mut ViewModel) {
    for entry in &mut view.sidebar {
        if matches!(entry.kind, SidebarKind::Group(_)) {
            entry.title = sentence_case(std::mem::take(&mut entry.title));
            entry.text = sentence_case(std::mem::take(&mut entry.text));
        }
    }
    if let Some(asking) = &mut view.asking {
        asking.question = question(std::mem::replace(&mut asking.question, Question::Immediate), asking.action.as_ref());
        if let Some(day) = &mut asking.another_day {
            *day = sentence_case(std::mem::take(day));
        }
        for choice in asking.choices.iter_mut().flat_map(|c| c.choices.iter_mut()) {
            if choice.id == crate::asking::ANOTHER_DAY {
                choice.title = sentence_case(std::mem::take(&mut choice.title));
            }
        }
    }
    if let Some(devices) = &mut view.devices {
        let words = &mut devices.words;
        for word in [&mut words.title, &mut words.wait, &mut words.join, &mut words.match_title, &mut words.match_yes, &mut words.match_no, &mut words.copy_code] {
            *word = sentence_case(std::mem::take(word));
        }
    }
    if let Some(proposal) = &mut view.proposal {
        let words = &mut proposal.words;
        for word in [&mut words.title, &mut words.item, &mut words.apply, &mut words.cancel, &mut words.unfit, &mut words.others] {
            *word = sentence_case(std::mem::take(word));
        }
    }
    for due in &mut view.due {
        due.dismiss = sentence_case(std::mem::take(&mut due.dismiss));
        for button in [&mut due.snooze, &mut due.carry, &mut due.replan].into_iter().flatten() {
            *button = sentence_case(std::mem::take(button));
        }
    }
    if let Some(settings) = &mut view.settings {
        for export in &mut settings.exports {
            export.label = sentence_case(std::mem::take(&mut export.label));
        }
    }
    if let Some(form) = &mut view.block_form {
        // A new block's heading is fixed words; a change's names the block, which keeps its
        // own capitals, after fixed words already in sentence case.
        if form.fields.iter().any(|field| field.key == "date") {
            form.heading = sentence_case(std::mem::take(&mut form.heading));
        }
        form.yes = sentence_case(std::mem::take(&mut form.yes));
    }
}

/// A question's title as its action's sentence-case name where it is that name, and a text
/// question's the core's own; its buttons and answers in sentence case.
fn question(question: Question, action: Option<&Action>) -> Question {
    // A title naming something ("Move Write report to") keeps it; one that is the action's own
    // name takes the action's sentence, which knows which words are names.
    let asks = |title: String| match action {
        Some(action) if action.title == title && !action.sentence.is_empty() => action.sentence.clone(),
        _ => title,
    };
    match question {
        Question::Confirm { title, message, yes } => Question::Confirm { title: asks(title), message, yes: sentence_case(yes) },
        Question::Text { title, label, initial, hint, optional, yes, sentence } => Question::Text {
            title: if sentence.is_empty() { title.clone() } else { sentence.clone() },
            label,
            initial,
            hint,
            optional,
            yes: sentence_case(yes),
            sentence,
        },
        Question::Pick { title, length, yes } => Question::Pick { title: asks(title), length, yes: sentence_case(yes) },
        Question::Choose { title, message, answers } => Question::Choose {
            title: asks(title),
            message,
            answers: answers.into_iter().map(|answer| lumenna_records::Choice { title: sentence_case(answer.title.clone()), ..answer }).collect(),
        },
        other => other,
    }
}
