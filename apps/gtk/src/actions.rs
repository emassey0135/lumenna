//! Doing what a row offers. Which actions a row has, what each is called and what each asks
//! first are the core's (`Action`, on every record a list shows), and running one is the
//! app's: this offers them — a row's menu, the keys, the menu bar — and hands them on.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use lumenna_app::{Asks, Offered};
use lumenna_records::{Action, ActionKind, Question, Subject};

use crate::window::{App, spawn};

/// What a key does on whatever row has focus: one meaning everywhere.
pub const SPACE: &[ActionKind] = &[
    ActionKind::MarkDone,
    ActionKind::MarkNotDone,
    ActionKind::StartTimer,
    ActionKind::PauseTimer,
    ActionKind::ResumeTimer,
    ActionKind::Restore,
];
/// What Delete does: whatever removes the row.
pub const DELETE: &[ActionKind] = &[ActionKind::Delete, ActionKind::DeleteForGood, ActionKind::Unassign, ActionKind::Unpair];
/// What Enter does: open it, or what free time and a cancelled day are for.
pub const ENTER: &[ActionKind] = &[ActionKind::Edit, ActionKind::EditTask, ActionKind::AddBlock, ActionKind::RestoreDay];

/// The first of `actions` of one of `kinds`.
pub fn find<'a>(actions: &'a [Action], kinds: &[ActionKind]) -> Option<&'a Action> {
    kinds.iter().find_map(|kind| actions.iter().find(|action| action.kind == *kind))
}

/// A menu of `actions`, each handed to `run` when chosen: the app's rows hand theirs to the
/// app. Those that remove something come last, apart.
pub fn menu_running(actions: &[Action], run: Rc<dyn Fn(Action)>) -> (gio::Menu, gio::SimpleActionGroup) {
    menu_of(actions.iter().map(|action| (action.title.as_str(), action.destructive, action.question != Question::Immediate, action.clone())), run)
}

/// The same of a list row's actions in short (`ListView::offered`), handed back with `Act`.
pub fn menu_offering(offered: &[Offered], run: Rc<dyn Fn(Offered)>) -> (gio::Menu, gio::SimpleActionGroup) {
    menu_of(offered.iter().map(|action| (action.title.as_str(), action.destructive, action.asks != Asks::Nothing, action.clone())), run)
}

/// A menu of entries, each its title, whether it removes something, whether it asks first, and
/// what `run` is handed when it is chosen.
fn menu_of<'a, T: Clone + 'static>(entries: impl Iterator<Item = (&'a str, bool, bool, T)>, run: Rc<dyn Fn(T)>) -> (gio::Menu, gio::SimpleActionGroup) {
    let menu = gio::Menu::new();
    let (keeping, removing) = (gio::Menu::new(), gio::Menu::new());
    let group = gio::SimpleActionGroup::new();
    for (index, (title, destructive, asks, value)) in entries.enumerate() {
        let name = format!("a{index}");
        // The core's titles, as spoken; an underscore in one is the title's, not a mnemonic.
        let title = title.replace('_', "__");
        let title = if asks { format!("{title}…") } else { title };
        let section = if destructive { &removing } else { &keeping };
        section.append(Some(&title), Some(&format!("row.{name}")));
        let simple = gio::SimpleAction::new(&name, None);
        let run = Rc::clone(&run);
        simple.connect_activate(move |_, _| run(value.clone()));
        group.add_action(&simple);
    }
    menu.append_section(None, &keeping);
    menu.append_section(None, &removing);
    (menu, group)
}

/// Opens what a form the app hands back opens here: a task's details.
pub fn open_form(app: &Rc<App>, action: Action) {
    if action.subject == Subject::Task && matches!(action.kind, ActionKind::Edit | ActionKind::EditTask) {
        let app = Rc::clone(app);
        spawn(async move { app.detail.open(&app, &action.target) });
    }
}
