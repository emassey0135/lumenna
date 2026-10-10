//! What can be done to a row, as the core decides it: every record the app lists carries its
//! `actions`, and this offers them — as a context menu, a menu-bar command, a button or a
//! key — asks each one's question as a Windows dialog, and hands the answer back to the core.
//! Which actions a row has, what they are called and what they ask are never decided here.
//!
//! Only a [`Question::Form`] is the app's own: the task details and the block form.

use lumenna_surface::{Action, ActionKind, Answer, Change, Choice, Question, Subject};
use windows::Win32::Foundation::HWND;

use super::app::App;
use super::clock::Locale;
use super::core::sentence;
use super::prompts;
use crate::speech::{self, Clock};

/// A context menu's commands are this plus the action's position, clear of the menu bar's.
const FIRST: u16 = 1000;

/// A context menu of `actions`: each under its spoken name, with a separator before the
/// first that removes something.
pub fn menu(actions: &[Action]) -> Vec<(u16, String)> {
    let mut items = Vec::new();
    for (index, action) in actions.iter().enumerate() {
        if action.destructive && index > 0 && !actions[..index].iter().any(|a| a.destructive) {
            items.push((0, String::new()));
        }
        items.push((FIRST + index as u16, label(action)));
    }
    items
}

/// An action's name on a menu or a button: an ellipsis says it asks something first, as
/// Windows' own commands do. A task's form is the details pane, which focus moves to rather
/// than a dialog opening.
pub fn label(action: &Action) -> String {
    let details = action.subject == Subject::Task && matches!(action.kind, ActionKind::Edit | ActionKind::EditTask);
    let asks = !matches!(action.question, Question::Immediate) && !details;
    format!("{}{}", action.title.replace('&', "&&"), if asks { "..." } else { "" })
}

/// The action a context menu's command stands for.
pub fn chosen(actions: &[Action], command: u16) -> Option<Action> {
    command.checked_sub(FIRST).and_then(|index| actions.get(usize::from(index))).cloned()
}

/// The first of `actions` that is one of `kinds`: what a key or a menu-bar command means on
/// this row.
pub fn of_kind(actions: &[Action], kinds: &[ActionKind]) -> Option<Action> {
    actions.iter().find(|action| kinds.contains(&action.kind)).cloned()
}

/// How a choice reads in a list: a block by its day and time, a task with its project.
fn choice_text(choice: &Choice) -> String {
    if let (Some(date), Some(start), Some(end)) = (&choice.date, &choice.start, &choice.end) {
        return format!("{}, {} to {}, {}", Locale.day(date), Locale.time(start), Locale.time(end), choice.title);
    }
    let mut text = choice.title.clone();
    if let Some(detail) = &choice.detail {
        text = format!("{text}, {detail}");
    }
    if choice.depth > 0 {
        text = format!("{text}, level {}", choice.depth + 1);
    }
    text
}

/// Asks `action`'s question, in dialogs owned by `owner`, and runs it. A form is opened by
/// `form` instead. Returns what it did and the answer given; `None` when it was cancelled,
/// refused (and said why), or a form.
///
/// A line of text the core refuses is asked again with what was typed, so a typo is fixed
/// rather than retyped.
pub fn run(app: &App, owner: HWND, action: &Action, form: impl FnOnce()) -> Option<(Change, Answer)> {
    let act = |answer: Answer| match app.core.lumenna.act(action.clone(), answer.clone()) {
        Ok(change) => {
            app.store_changed();
            Ok((change, answer))
        }
        Err(error) => {
            prompts::fail(owner, &sentence(&error));
            Err(())
        }
    };
    match &action.question {
        Question::Form => {
            form();
            None
        }
        Question::Immediate => act(Answer::Yes).ok(),
        Question::Confirm { title, message, yes } => {
            prompts::confirm(owner, title, message, yes).then(|| act(Answer::Yes).ok()).flatten()
        }
        Question::Choose { title, message, answers } => {
            let names: Vec<&str> = answers.iter().map(|a| a.title.as_str()).collect();
            let index = prompts::choose(owner, title, message, &names, action.destructive)?;
            act(Answer::Picked { id: answers[index].id.clone(), length: None }).ok()
        }
        Question::Text { title, label, initial, hint, .. } => {
            let mut typed = initial.clone();
            loop {
                let text = prompts::ask(owner, title, &format!("&{label}:"), hint, &typed)?;
                match act(Answer::Text { text: text.clone() }) {
                    Ok(done) => return Some(done),
                    Err(()) => typed = text,
                }
            }
        }
        Question::Pick { title, length } => {
            let offered = match app.core.lumenna.choices(action.clone()) {
                Ok(offered) => offered,
                Err(error) => {
                    prompts::fail(owner, &sentence(&error));
                    return None;
                }
            };
            if offered.choices.is_empty() {
                app.say(&speech::sentence(&speech::announcement(&offered.announcement, &offered.notices)));
                return None;
            }
            let texts: Vec<String> = offered.choices.iter().map(choice_text).collect();
            let index = prompts::pick(owner, &action.title, &format!("{title}:"), &texts)?;
            let id = offered.choices[index].id.clone();
            let Some(hint) = length else {
                return act(Answer::Picked { id, length: None }).ok();
            };
            let mut typed = String::new();
            loop {
                let text = prompts::ask(owner, &action.title, "&Planned length:", hint, &typed)?;
                match act(Answer::Picked { id: id.clone(), length: Some(text.clone()) }) {
                    Ok(done) => return Some(done),
                    Err(()) => typed = text,
                }
            }
        }
    }
}
