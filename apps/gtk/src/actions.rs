//! Doing what a row offers. Which actions a row has, what each is called and what each asks
//! first are the core's (`Action`, on every record a list shows), and running one is the
//! app's: this offers them — a row's menu, the keys, the menu bar — and hands them on.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
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
    let menu = gio::Menu::new();
    let (keeping, removing) = (gio::Menu::new(), gio::Menu::new());
    let group = gio::SimpleActionGroup::new();
    for (index, action) in actions.iter().enumerate() {
        let name = format!("a{index}");
        // The core's titles, as spoken; an underscore in one is the title's, not a mnemonic.
        let title = action.title.replace('_', "__");
        let title = if action.question == Question::Immediate { title } else { format!("{title}…") };
        let section = if action.destructive { &removing } else { &keeping };
        section.append(Some(&title), Some(&format!("row.{name}")));
        let simple = gio::SimpleAction::new(&name, None);
        let (action, run) = (action.clone(), Rc::clone(&run));
        simple.connect_activate(move |_, _| run(action.clone()));
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
