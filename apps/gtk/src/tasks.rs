//! Tasks, with the filter above them.
//!
//! A tree, so a subtask's level and every row's position are the tree's to report. A row's
//! actions are the core's (`actions`): Space checks a task off (in the trash, restores it),
//! Delete trashes it (in the trash, deletes it from there), Enter opens its details, and the
//! Menu key or Shift+F10 offers the rest. After a change the selection, which is the screen
//! reader's focus, lands on the same task if it is still listed and otherwise on whatever now
//! holds its place.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_surface::places::Place;
use lumenna_desktop::speech;
use lumenna_surface::RowView;

use crate::core::sentence;
use crate::actions;
use crate::tree::{Item, Tree};
use crate::window::App;

pub struct TaskList {
    pub place: Place,
    trash: bool,
    pub widget: gtk::Box,
    filter: gtk::Entry,
    readback: gtk::Label,
    pub tree: Rc<Tree>,
    rows: RefCell<Vec<RowView>>,
}

impl TaskList {
    pub fn new(place: Place, lumenna: std::sync::Arc<lumenna_surface::Lumenna>) -> Rc<Self> {
        let trash = place == Place::Trash;
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        let filter = gtk::Entry::builder().text(place.query()).build();
        let label = gtk::Label::builder().label("Filte_r").use_underline(true).xalign(0.0).build();
        label.set_mnemonic_widget(Some(&filter));
        filter.update_property(&[gtk::accessible::Property::Description(
            "Such as p1 & due before: friday, or #Work. Down arrow offers what could come next. Enter goes to what it finds.",
        )]);
        crate::completion::attach(&filter, lumenna, lumenna_surface::Syntax::Filter);
        let readback = gtk::Label::builder().xalign(0.0).wrap(true).build();
        let tree = Tree::new(&place.title());
        // The trash is everything deleted, and nothing else: there is no filter to change.
        if !trash {
            widget.append(&label);
            widget.append(&filter);
        }
        widget.append(&readback);
        widget.append(&tree.widget);
        let list = Rc::new(Self { place, trash, widget, filter, readback, tree, rows: RefCell::new(Vec::new()) });
        list.connect();
        list
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.filter.connect_changed(move |_| {
            // Typing is silent; the rows follow it, and Enter says what was found.
            if let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) {
                list.list(&app);
            }
        });
        let weak = Rc::downgrade(self);
        self.filter.connect_activate(move |_| {
            // Finishing the filter says what it found, then goes to it.
            if let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) {
                app.say(&list.readback.label());
                list.tree.focus();
            }
        });
        let escape = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(list) = weak.upgrade() {
                    list.tree.focus();
                    return glib::Propagation::Stop;
                }
            glib::Propagation::Proceed
        });
        self.filter.add_controller(escape);

        let weak = Rc::downgrade(self);
        self.tree.connect_selected(move |_| {
            if let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) {
                let id = list.selected().filter(|_| !list.trash).map(|row| row.id);
                app.detail.follow(&app, id.as_deref());
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_activate(move |index| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let row = list.rows.borrow().get(index).cloned();
            if let Some(action) = row.as_ref().and_then(|row| actions::find(&row.actions, actions::ENTER)) {
                actions::run(&app, action.clone(), None);
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_key(move |key, modifiers, index| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else {
                return glib::Propagation::Proceed;
            };
            if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let Some(row) = list.rows.borrow().get(index).cloned() else { return glib::Propagation::Proceed };
            let kinds = match key {
                gdk::Key::space | gdk::Key::KP_Space => actions::SPACE,
                gdk::Key::Delete | gdk::Key::KP_Delete => actions::DELETE,
                _ => return glib::Propagation::Proceed,
            };
            if let Some(action) = actions::find(&row.actions, kinds) {
                actions::run(&app, action.clone(), None);
            }
            glib::Propagation::Stop
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_menu(move |index, point| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = list.rows.borrow().get(index).cloned() else { return };
            let (menu, group) = actions::menu(&row.actions, None);
            app.popup(&menu, list.tree.view.upcast_ref(), point, Some(&group));
        });
    }

    /// The row selected, if any.
    pub fn selected(&self) -> Option<RowView> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// Lists again, keeping the selection. A filter still being typed may not read yet; then
    /// the old rows stay and the readback says what is wrong with it.
    pub fn list(&self, app: &App) {
        let key = self.tree.selected().and_then(|index| self.tree.key(index));
        let near = self.tree.selected().or(Some(0));
        let query = if self.trash { self.place.query() } else { self.filter.text().to_string() };
        match app.core.lumenna.list_tasks(&query) {
            Ok(listing) => {
                let mut said = Vec::new();
                if !self.trash {
                    said.extend(listing.query.as_ref().map(|q| q.description.clone()));
                }
                // An empty list says what is empty, in place of a count of nothing.
                said.push(if listing.rows.is_empty() && !listing.empty.is_empty() {
                    listing.empty.clone()
                } else {
                    listing.announcement.clone()
                });
                said.extend(listing.notices.iter().cloned());
                self.readback.set_label(&speech::sentence(&said.join(". ")));
                let items = listing
                    .rows
                    .iter()
                    .map(|row| Item {
                        key: row.id.clone(),
                        // Orca does not read a tree item's checked state, so "completed" is
                        // said in words (`tree`).
                        text: if self.trash { speech::trashed(row, &app.clock) } else { speech::row(row, false, &app.clock) },
                        depth: row.depth,
                    })
                    .collect();
                *self.rows.borrow_mut() = listing.rows;
                if self.tree.set(items) {
                    self.tree.select_key_or_near(key.as_deref(), near);
                }
            }
            Err(error) => self.readback.set_label(&sentence(&error)),
        }
    }

    /// Whether this is the trash, where only restoring and erasing apply.
    pub fn is_trash(&self) -> bool {
        self.trash
    }

    /// What a new task typed here starts with.
    pub fn quick_add_prefix(&self) -> String {
        self.place.quick_add_prefix()
    }

    /// Selects a task just made, if this list shows it.
    pub fn land_on(&self, id: &str) {
        self.tree.select_key_or_near(Some(id), None);
    }

    /// Puts focus in the filter, all of it selected so typing replaces it.
    pub fn focus_filter(&self) {
        if !self.trash {
            self.filter.grab_focus();
        }
    }
}
