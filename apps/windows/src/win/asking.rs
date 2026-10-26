//! The question the app has open (`view.asking`), asked as a Windows dialog, its answer sent
//! back as `ShellEvent::Answer`, or `Cancel`.
//!
//! The app keeps the question open until it is answered: a refused answer comes back with
//! `refusal` set, said first, and a line of text is asked again with what was typed, so a typo
//! is fixed rather than retyped. Each is asked by its number (`Asking::number`): a new number
//! is a new question, and the same one is asked again only once its answer is refused.

use lumenna_ffi::ShellEvent;
use lumenna_records::{Answer, Question};

use super::app::App;
use super::{actions, prompts};
use crate::speech;

/// Asks the question the app has open now, if any, and sends the answer.
pub fn ask(app: &App) {
    let view = app.crux.view();
    let Some(asking) = view.asking else { return };
    let owner = app.owner();
    if let Some(refusal) = &asking.refusal {
        prompts::fail(owner, &speech::sentence(refusal));
    }
    let destructive = asking.action.as_ref().is_some_and(|action| action.destructive);
    let answer = match &asking.question {
        Question::Immediate => None,
        // The one form the app hands back, a task's (Properties, run as the app's): the
        // details pane, which shows the task in hand. The question is closed as it opens.
        Question::Form => {
            app.open_detail();
            None
        }
        Question::Confirm { title, message, yes } => prompts::confirm(owner, title, message, yes).then_some(Answer::Yes),
        Question::Choose { title, message, answers } => {
            let names: Vec<&str> = answers.iter().map(|answer| answer.title.as_str()).collect();
            prompts::choose(owner, title, message, &names, destructive)
                .map(|index| Answer::Picked { id: answers[index].id.clone(), length: None })
        }
        Question::Text { initial, .. } => {
            // Refused, the line starts as it was typed, the app's to keep.
            let start = asking.typed.clone().filter(|_| asking.refusal.is_some()).unwrap_or_else(|| initial.clone());
            let text = actions::ask(owner, &asking.question, &start, prompts::Takes { day: asking.day.as_deref(), syntax: asking.syntax });
            text.map(|text| Answer::Text { text })
        }
        Question::Pick { title, yes, .. } => {
            let Some(offered) = &asking.choices else { return };
            let lines = app.crux.lines();
            // The core's words, then the level where it changes (`Line::level`), which a list box
            // has no other way to say.
            // Put in a Block's ends in the app's own Another Day, which asks which day as a
            // question of its own and then offers that day's blocks here.
            let texts: Vec<String> = lines
                .choices
                .iter()
                .take(offered.choices.len())
                .map(|line| match &line.level {
                    Some(level) => format!("{}, {level}", line.text),
                    None => line.text.clone(),
                })
                .collect();
            let reasons: Vec<String> = offered.choices.iter().map(|choice| choice.reason.clone().unwrap_or_default()).collect();
            // A colour pick's marks, beside each colour's name.
            let swatches: Vec<_> = offered.choices.iter().map(|choice| choice.swatch.clone()).collect();
            let named = asking.action.as_ref().map_or(title.as_str(), |action| action.title.as_str());
            prompts::pick(owner, named, &format!("{title}:"), &texts, &reasons, &swatches, yes)
                .map(|index| Answer::Picked { id: offered.choices[index].id.clone(), length: None })
        }
    };
    if answer.is_some() {
        app.answered();
    }
    app.crux.send(answer.map_or(ShellEvent::Cancel, ShellEvent::Answer));
}
