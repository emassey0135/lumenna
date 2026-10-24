//! Doing what a row offers. Which actions a row has, what each is called and what each asks
//! first are the core's (`Action`, on every record a list shows), and running one is the
//! app's: this offers them — a row's menu, the keys, the menu bar — and hands them on.

use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;
use lumenna_app::{ActingOn, Asks, Offered};
use lumenna_records::{Action, ActionKind, Subject};

use crate::window::{App, spawn};

/// What Enter does: open it, or what free time and a cancelled day are for.
pub const ENTER: &[ActionKind] = &[ActionKind::Edit, ActionKind::EditTask, ActionKind::AddBlock, ActionKind::RestoreDay];

/// The first of `offered` of one of `kinds`.
pub fn find<'a>(offered: &'a [Offered], kinds: &[ActionKind]) -> Option<&'a Offered> {
    kinds.iter().find_map(|kind| offered.iter().find(|action| action.kind == *kind))
}

/// A menu of what a row offers, in short, each chosen handed to the app for the row `on`
/// names (`ActOn`); `name` is what the row is called, for a reminder list it opens. Those that
/// remove something come last, apart.
pub fn menu(offered: &[Offered], on: &ActingOn, name: Option<&str>) -> (gio::Menu, gio::SimpleActionGroup) {
    let menu = gio::Menu::new();
    let (keeping, removing) = (gio::Menu::new(), gio::Menu::new());
    let group = gio::SimpleActionGroup::new();
    for (index, action) in offered.iter().enumerate() {
        let name_of = format!("a{index}");
        // The core's titles, as spoken; an underscore in one is the title's, not a mnemonic.
        let title = action.title.replace('_', "__");
        let title = if action.asks == Asks::Nothing { title } else { format!("{title}…") };
        let section = if action.destructive { &removing } else { &keeping };
        section.append(Some(&title), Some(&format!("row.{name_of}")));
        let simple = gio::SimpleAction::new(&name_of, None);
        let (action, on, name) = (action.clone(), on.clone(), name.map(str::to_owned));
        simple.connect_activate(move |_, _| {
            if let Some(app) = crate::window::app() {
                crate::asking::act_on(&app, on.clone(), action.clone(), name.as_deref());
            }
        });
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
