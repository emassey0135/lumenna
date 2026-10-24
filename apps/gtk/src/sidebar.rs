//! The places: Today, Tasks, the projects as they nest, labels, saved filters, Blocks, the
//! trash. The app's sidebar (`ViewModel::sidebar`), drawn.
//!
//! A tree like every list here, so a subproject's level is the tree's to report. Moving
//! through it shows each place in the middle pane, as Files' sidebar does; Enter goes into it.
//! What a project, label, filter or heading can have done to it is the core's, offered in its
//! context menu; Delete runs its Delete, Left and Right its Collapse and Expand. The row in hand
//! is the app's (`sidebar_held`), so a change made here lands here; where an action leads — a
//! place renamed, made or deleted — is the app's too, and the middle pane follows it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_ffi::ShellEvent;
use lumenna_records::places::{Place, SidebarEntry, SidebarGroup, SidebarKind};
use lumenna_app::{ActingOn, Offered, Side};
use lumenna_records::ActionKind;

use crate::actions;
use crate::tree::{Item, Tree};
use crate::window::App;

pub struct Sidebar {
    pub tree: Rc<Tree>,
    entries: RefCell<Vec<SidebarEntry>>,
    /// The app's landing count for the sidebar last drawn: focus moves when it goes up.
    landed: Cell<Option<u32>>,
}

impl Sidebar {
    pub fn new() -> Rc<Self> {
        let sidebar = Rc::new(Self { tree: Tree::new("Places"), entries: RefCell::new(Vec::new()), landed: Cell::new(None) });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_selected(move |index| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(entry) = index.and_then(|index| sidebar.entries.borrow().get(index).cloned()) else { return };
            let (held, place) = {
                let view = app.model.view();
                (view.sidebar_held.focus.clone(), view.place.clone())
            };
            if held.as_deref() != Some(entry.key.as_str()) {
                app.model.send(ShellEvent::Hold(Side::Sidebar, entry.key.clone()));
            }
            if let SidebarKind::Place(chosen) = entry.kind
                && place.as_ref() != Some(&chosen)
            {
                app.show_place(chosen);
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
                let Some(entry) = sidebar.entries.borrow().get(index).cloned() else { return glib::Propagation::Stop };
                // What Delete runs is the core's. The Inbox offers none, and the app says why it
                // stays; a fixed place or a heading has nothing to say.
                let deletes = lumenna_app::keys::runs("delete").unwrap_or_default();
                let offers = entry.offered.iter().any(|action| deletes.contains(&action.kind));
                if offers || matches!(&entry.kind, SidebarKind::Place(Place::Project(_) | Place::Label(_) | Place::Filter { .. })) {
                    app.model.send(ShellEvent::Hold(Side::Sidebar, entry.key.clone()));
                    app.model.send(ShellEvent::Key("delete".to_owned()));
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_menu(move |index, point| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(entry) = sidebar.entries.borrow().get(index).cloned() else { return };
            // Folding is Left and Right's, as in every tree; the menu holds what the row is for.
            let offered: Vec<Offered> =
                entry.offered.iter().filter(|a| !matches!(a.kind, ActionKind::Expand | ActionKind::Collapse)).cloned().collect();
            if offered.is_empty() {
                return;
            }
            let (menu, group) = actions::menu(&offered, &ActingOn::Held(Side::Sidebar, entry.key.clone()), Some(&entry.title));
            app.popup(&menu, sidebar.tree.view.upcast_ref(), point, Some(&group));
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_fold(move |index, expand| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(entry) = sidebar.entries.borrow().get(index).cloned() else { return };
            let kind = if expand { ActionKind::Expand } else { ActionKind::Collapse };
            if let Some(action) = entry.offered.iter().find(|action| action.kind == kind) {
                crate::window::QUIET_FOLD.set(true);
                run(&app, action.clone(), &entry.key);
            }
        });
        sidebar
    }

    /// Draws the places the app last left. Focus moves to the row in hand only when the app
    /// moved it and focus is here; otherwise the place shown is the row selected.
    pub fn draw(&self, app: &App) {
        let view = app.model.view();
        if !view.ready {
            return;
        }
        let items = view
            .sidebar
            .iter()
            .map(|entry| Item {
                key: entry.key.clone(),
                text: entry.text.clone(),
                depth: entry.depth,
                expanded: entry.offered.iter().find_map(|action| match action.kind {
                    ActionKind::Collapse => Some(true),
                    ActionKind::Expand => Some(false),
                    _ => None,
                }),
                swatch: entry.swatch.clone(),
                ..Default::default()
            })
            .collect();
        *self.entries.borrow_mut() = view.sidebar.clone();
        let rebuilt = self.tree.set(items);
        let held = &view.sidebar_held;
        let first = self.landed.get().is_none();
        let moved = self.landed.replace(Some(held.landing)).is_some_and(|landed| landed != held.landing);
        // The window opens before the places come, its focus on their empty pane: the first
        // places drawn take it, as they did when they were there from the start.
        let focus = gtk::prelude::GtkWindowExt::focus(&app.window);
        if first && focus.as_ref().is_none_or(|focus| focus == self.tree.widget.upcast_ref::<gtk::Widget>()) {
            self.tree.focus();
        }
        let shown = view.place.as_ref().map(place_key);
        if self.tree.has_focus() {
            if let Some(index) = held.focus.as_deref().and_then(|key| self.tree.index_of(key))
                && (moved || rebuilt)
            {
                self.tree.select_quietly(index);
            }
        } else if let Some(index) = shown.as_deref().and_then(|key| self.tree.index_of(key))
            && self.tree.selected() != Some(index)
        {
            self.tree.select_quietly(index);
        }
    }

    fn selected(&self) -> Option<SidebarEntry> {
        let index = self.tree.selected()?;
        self.entries.borrow().get(index).cloned()
    }
}

/// Runs one of a sidebar row's actions through the app: held there, so what it changes lands
/// back in the sidebar.
fn run(app: &App, action: Offered, key: &str) {
    app.model.act_on(ActingOn::Held(Side::Sidebar, key.to_owned()), action);
}

/// What the File menu's New Project, New Label and New Saved Filter run: the heading's own.
pub fn new_in(app: &Rc<App>, group: SidebarGroup) {
    let key = SidebarEntry::key_of(&SidebarKind::Group(group));
    let action = app.model.view().sidebar.iter().find(|entry| entry.key == key).and_then(|entry| entry.offered.first().cloned());
    if let Some(action) = action {
        run(app, action, &key);
    }
}

/// The key a place's row has in the sidebar.
fn place_key(place: &Place) -> String {
    SidebarEntry::key_of(&SidebarKind::Place(place.clone()))
}
