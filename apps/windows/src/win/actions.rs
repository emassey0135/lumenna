//! What can be done to a row, as the core decides it: every record the app lists carries its
//! `actions`, and this offers them — as a context menu, a menu-bar command, a button or a key.
//! Which actions a row has, what they are called and what they ask are never decided here;
//! running one is the app's (`ShellEvent::Perform`), and the question it asks is asked as a
//! Windows dialog (`asking.rs`).

use lumenna_records::{Action, ActionKind, Question, Subject};
use windows::Win32::Foundation::HWND;

use super::prompts;

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

/// Asks a line of text as `question` words it, starting from `typed`: its title, its field's
/// name with an access key, what it takes, and its own button. `None` if it is not a line of
/// text, or was cancelled. An empty answer is returned too: the core says whether it may be.
pub fn ask(owner: HWND, question: &Question, typed: &str) -> Option<String> {
    let Question::Text { title, label, hint, yes, .. } = question else { return None };
    prompts::ask(owner, title, &format!("&{label}:"), hint, typed, yes)
}
