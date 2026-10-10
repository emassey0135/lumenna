//! The places: Today, Tasks, the projects as they nest, labels, saved filters, Blocks, the
//! trash.
//!
//! A tree like every list here, so a subproject's level is the tree's to report. Moving
//! through it shows each place in the middle pane, as Files' sidebar does; Enter goes into it.
//! What a project, label, filter or heading can have done to it is the core's (`actions`),
//! offered in its context menu; Delete runs its Delete.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_surface::actions::{ActionKind, Answer, Subject};
use lumenna_surface::places::{self, Place, SidebarEntry, SidebarGroup, SidebarKind};

use crate::actions;
use crate::tree::{Item, Tree};
use crate::window::App;

pub struct Sidebar {
    pub tree: Rc<Tree>,
    entries: RefCell<Vec<SidebarEntry>>,
    /// The place shown, by key.
    current: RefCell<String>,
}

impl Sidebar {
    pub fn new() -> Rc<Self> {
        let sidebar = Rc::new(Self { tree: Tree::new("Places"), entries: RefCell::new(Vec::new()), current: RefCell::new(String::new()) });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_selected(move |_| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            if let Some(SidebarEntry { kind: SidebarKind::Place(place), .. }) = sidebar.selected() {
                let key = place_key(&place);
                if *sidebar.current.borrow() != key {
                    *sidebar.current.borrow_mut() = key;
                    app.show_place(place);
                }
            }
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_activate(move |_| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            if matches!(sidebar.selected(), Some(SidebarEntry { kind: SidebarKind::Place(_), .. })) {
                app.focus_content();
            }
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_key(move |key, modifiers, index| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else {
                return glib::Propagation::Proceed;
            };
            // Delete here does what it does in every other list: the row's own Delete.
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) && modifiers.is_empty() {
                let entry = sidebar.entries.borrow().get(index).cloned();
                if let Some(entry) = entry
                    && let Some(action) = actions::find(&entry.actions, actions::DELETE)
                {
                    actions::run(&app, action.clone(), Some(after(&entry)));
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_menu(move |index, point| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(entry) = sidebar.entries.borrow().get(index).cloned() else { return };
            if entry.actions.is_empty() {
                return;
            }
            let (menu, group) = actions::menu(&entry.actions, Some(after(&entry)));
            app.popup(&menu, sidebar.tree.view.upcast_ref(), point, Some(&group));
        });
        sidebar
    }

    /// Lists the places again, keeping the one shown selected without showing it again —
    /// which would throw away whatever the middle pane was doing.
    pub fn reload(&self, app: &App) {
        let entries = places::sidebar(&app.core.lumenna);
        let items = entries.iter().map(|entry| Item { key: entry.key(), text: entry.text.clone(), depth: entry.depth }).collect();
        *self.entries.borrow_mut() = entries;
        if self.tree.set(items) {
            let current = self.current.borrow().clone();
            if let Some(index) = self.tree.index_of(&current) {
                self.tree.select_quietly(index);
            }
        }
    }

    /// Selects a place, as going there from the menu does.
    pub fn select(&self, place: &Place) {
        let key = place_key(place);
        *self.current.borrow_mut() = key.clone();
        if let Some(index) = self.tree.index_of(&key) {
            self.tree.select_quietly(index);
        }
    }

    fn selected(&self) -> Option<SidebarEntry> {
        let index = self.tree.selected()?;
        self.entries.borrow().get(index).cloned()
    }
}

/// Where to go once one of `entry`'s actions is done: to the place it renamed, merged into or
/// made, or to Tasks from one it deleted. Anything else stays put.
fn after(entry: &SidebarEntry) -> actions::After {
    let place = match &entry.kind {
        SidebarKind::Place(place) => Some(place.clone()),
        SidebarKind::Group(_) => None,
    };
    Rc::new(move |app, action, answer| {
        let text = match answer {
            Answer::Text { text } => Some(text.clone()),
            Answer::Picked { id, .. } => Some(id.clone()),
            Answer::Yes => None,
        };
        let then = match (action.subject, action.kind, text) {
            (_, ActionKind::Delete, _) => Some(Place::Tasks),
            (Subject::Project, ActionKind::Rename | ActionKind::New | ActionKind::NewInside, Some(name)) => {
                Some(Place::Project(name))
            }
            (Subject::Label, ActionKind::Rename | ActionKind::New | ActionKind::MergeInto, Some(name)) => {
                Some(Place::Label(name))
            }
            (Subject::Filter, ActionKind::Rename, Some(name)) => match &place {
                Some(Place::Filter { query, .. }) => Some(Place::Filter { name, query: query.clone() }),
                _ => None,
            },
            (Subject::Filter, ActionKind::ChangeQuery, Some(query)) => match &place {
                Some(Place::Filter { name, .. }) => Some(Place::Filter { name: name.clone(), query }),
                _ => None,
            },
            _ => None,
        };
        if let Some(then) = then {
            app.go(then, false);
        }
    })
}

/// What the File menu's New Project, New Label and New Saved Filter run: the heading's own.
pub fn new_in(app: &Rc<App>, group: SidebarGroup) {
    let entry = SidebarEntry {
        kind: SidebarKind::Group(group),
        text: String::new(),
        depth: 0,
        archived: false,
        actions: lumenna_surface::actions::heading(group),
    };
    if let Some(action) = entry.actions.first() {
        actions::run(app, action.clone(), Some(after(&entry)));
    }
}

/// The key a place's row has in the sidebar.
fn place_key(place: &Place) -> String {
    SidebarEntry { kind: SidebarKind::Place(place.clone()), text: String::new(), depth: 0, archived: false, actions: Vec::new() }
        .key()
}
