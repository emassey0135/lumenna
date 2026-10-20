//! The question the app has open (`view.asking`), asked as a Windows dialog, its answer sent
//! back as `ShellEvent::Answer`, or `Cancel`.
//!
//! The app keeps the question open until it is answered: a refused answer comes back with
//! `refusal` set, said first, and a line of text is asked again with what was typed, so a typo
//! is fixed rather than retyped. Each is asked once the app is still, so a question is never
//! asked again from a turn made before its answer arrived.

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
        // Not asked by a dialog: the task's form is the details pane, which the list opens
        // itself, and nothing else asks this way yet.
        Question::Immediate | Question::Form => None,
        Question::Confirm { title, message, yes } => prompts::confirm(owner, title, message, yes).then_some(Answer::Yes),
        Question::Choose { title, message, answers } => {
            let names: Vec<&str> = answers.iter().map(|answer| answer.title.as_str()).collect();
            prompts::choose(owner, title, message, &names, destructive)
                .map(|index| Answer::Picked { id: answers[index].id.clone(), length: None })
        }
        Question::Text { initial, .. } => {
            let start = if asking.refusal.is_some() { app.typed.borrow().clone() } else { initial.clone() };
            let text = actions::ask(owner, &asking.question, &start);
            if let Some(text) = &text {
                *app.typed.borrow_mut() = text.clone();
            }
            text.map(|text| Answer::Text { text })
        }
        Question::Pick { title, yes, .. } => {
            let Some(offered) = &asking.choices else { return };
            let lines = app.crux.lines();
            // The core's words, then the level, which a list box has no other way to say.
            // Put in a Block's ends in the app's own Another Day, which asks which day as a
            // question of its own and then offers that day's blocks here.
            let texts: Vec<String> = offered
                .choices
                .iter()
                .zip(&lines.choices)
                .map(|(choice, line)| if choice.depth > 0 { format!("{}, level {}", line.text, choice.depth + 1) } else { line.text.clone() })
                .collect();
            let reasons: Vec<String> = offered.choices.iter().map(|choice| choice.reason.clone().unwrap_or_default()).collect();
            let named = asking.action.as_ref().map_or(title.as_str(), |action| action.title.as_str());
            prompts::pick(owner, named, &format!("{title}:"), &texts, &reasons, yes)
                .map(|index| Answer::Picked { id: offered.choices[index].id.clone(), length: None })
        }
    };
    app.crux.send(answer.map_or(ShellEvent::Cancel, ShellEvent::Answer));
}
