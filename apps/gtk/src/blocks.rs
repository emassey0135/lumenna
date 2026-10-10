//! Every block series: Enter changes one, Delete deletes it; both are the row's own actions.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_desktop::speech;
use lumenna_surface::RowView;

use crate::actions;
use crate::core::sentence;
use crate::tree::{Item, Tree};
use crate::window::App;

pub struct BlockList {
    pub widget: gtk::Box,
    count: gtk::Label,
    pub tree: Rc<Tree>,
    rows: RefCell<Vec<RowView>>,
}

impl BlockList {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        let count = gtk::Label::builder().xalign(0.0).build();
        let add = gtk::Button::builder().label("Add Block…").action_name("win.new-block").halign(gtk::Align::Start).build();
        let tree = Tree::new("Blocks");
        widget.append(&count);
        widget.append(&add);
        widget.append(&tree.widget);
        let list = Rc::new(Self { widget, count, tree, rows: RefCell::new(Vec::new()) });
        list.connect();
        list
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.tree.connect_activate(move |index| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let row = list.row(index);
            if let Some(action) = row.as_ref().and_then(|row| actions::find(&row.actions, actions::ENTER)) {
                actions::run(&app, action.clone(), None);
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_key(move |key, modifiers, index| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else {
                return glib::Propagation::Proceed;
            };
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) && modifiers.is_empty() {
                let row = list.row(index);
                if let Some(action) = row.as_ref().and_then(|row| actions::find(&row.actions, actions::DELETE)) {
                    actions::run(&app, action.clone(), None);
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_menu(move |index, point| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = list.row(index).filter(|row| !row.actions.is_empty()) else { return };
            let (menu, group) = actions::menu(&row.actions, None);
            app.popup(&menu, list.tree.view.upcast_ref(), point, Some(&group));
        });
    }

    fn row(&self, index: usize) -> Option<RowView> {
        self.rows.borrow().get(index).cloned()
    }

    /// Lands on a block just made, if it is listed.
    pub fn land_on(&self, series: &str) {
        self.tree.focus();
        self.tree.select_key_or_near(Some(series), None);
    }

    pub fn reload(&self, app: &App) {
        let key = self.tree.selected().and_then(|index| self.tree.key(index));
        let near = self.tree.selected().or(Some(0));
        match app.core.lumenna.list_blocks() {
            Ok(listing) => {
                self.count.set_label(&speech::sentence(&listing.announcement));
                let items = listing
                    .rows
                    .iter()
                    .map(|row| Item { key: row.id.clone(), text: speech::row(row, false, &app.clock), depth: 0 })
                    .collect();
                *self.rows.borrow_mut() = listing.rows;
                if self.tree.set(items) {
                    self.tree.select_key_or_near(key.as_deref(), near);
                }
            }
            Err(error) => self.count.set_label(&sentence(&error)),
        }
    }
}
