//! What can be done to a row, as the core decides it: every row the app shows offers its
//! actions in short (`offered`), and this offers them — as a context menu, a menu-bar command,
//! a button or a key.
//! Which actions a row has, what they are called and what they ask are never decided here;
//! running one is the app's (`ShellEvent::ActOn`, naming the row, with the action in short as
//! the view offers it), and the question it asks is asked as a Windows dialog (`asking.rs`).

use lumenna_app::{ActingOn, Asks, Offered};
use lumenna_ffi::ShellEvent;
use lumenna_records::{ActionKind, Question, Subject};
use windows::Win32::Foundation::HWND;

use super::app::App;
use super::prompts;

/// A context menu's commands are this plus the action's position, clear of the menu bar's.
const FIRST: u16 = 1000;

/// A list row's actions, from the places its item names in the list's `offered`.
pub fn of_row(offered: &[Offered], places: &[u32]) -> Vec<Offered> {
    places.iter().filter_map(|place| offered.get(*place as usize).cloned()).collect()
}

/// Runs a row's Expand or Collapse when the tree folded it: a click can fold a row other than
/// the one selected, which the app puts in hand to run it, so the selected one is put back after.
pub fn fold(app: &App, key: String, action: Offered, selected: Option<String>) {
    let selected = selected.filter(|selected| *selected != key);
    app.crux.send(ShellEvent::ActOn(ActingOn::Row(key), action));
    if let Some(selected) = selected {
        app.crux.send(ShellEvent::Focus(selected));
    }
}

/// A context menu of `actions`: each under its spoken name, with a separator before the
/// first that removes something.
pub fn menu(actions: &[Offered]) -> Vec<(u16, String)> {
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
pub fn label(action: &Offered) -> String {
    let details = action.subject == Subject::Task && matches!(action.kind, ActionKind::Edit | ActionKind::EditTask);
    let asks = action.asks != Asks::Nothing && !details;
    format!("{}{}", action.title.replace('&', "&&"), if asks { "..." } else { "" })
}

/// The action a context menu's command stands for.
pub fn chosen(actions: &[Offered], command: u16) -> Option<Offered> {
    command.checked_sub(FIRST).and_then(|index| actions.get(usize::from(index))).cloned()
}

/// The first of `actions` that is one of `kinds`: what a key or a menu-bar command means on
/// this row.
pub fn of_kind(actions: &[Offered], kinds: &[ActionKind]) -> Option<Offered> {
    actions.iter().find(|action| kinds.contains(&action.kind)).cloned()
}

/// Asks a line of text as `question` words it, starting from `typed`: its title, its field's
/// name with an access key, what it takes, and its own button. `None` if it is not a line of
/// text, or was cancelled. An empty answer is returned too: the core says whether it may be. A
/// question asking for a day has a calendar beside its field, and one asking for a query
/// completes (`takes`).
pub fn ask(owner: HWND, question: &Question, typed: &str, takes: prompts::Takes) -> Option<String> {
    let Question::Text { title, label, hint, yes, .. } = question else { return None };
    prompts::ask(owner, title, &format!("&{label}:"), hint, typed, yes, takes)
}
