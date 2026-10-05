//! Every block series (§16.1: block editor): Enter changes one, Delete deletes it.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_desktop::speech;
use lumenna_surface::RowView;

use crate::block_form::{self, Fields, Purpose};
use crate::core::sentence;
use crate::prompts;
use crate::tree::{Item, Tree};
use crate::window::{App, spawn};

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
            if let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) {
                list.edit(&app, index);
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_key(move |key, modifiers, index| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else {
                return glib::Propagation::Proceed;
            };
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) && modifiers.is_empty() {
                list.delete(&app, index);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_menu(move |index, point| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let menu = gio::Menu::new();
            menu.append(Some("_Change…"), Some("row.edit"));
            menu.append(Some("_Delete…"), Some("row.delete"));
            let actions = gio::SimpleActionGroup::new();
            let edit = gio::SimpleAction::new("edit", None);
            let weak_list = Rc::downgrade(&list);
            edit.connect_activate(move |_, _| {
                if let (Some(list), Some(app)) = (weak_list.upgrade(), crate::window::app()) {
                    list.edit(&app, index);
                }
            });
            let delete = gio::SimpleAction::new("delete", None);
            let weak_list = Rc::downgrade(&list);
            delete.connect_activate(move |_, _| {
                if let (Some(list), Some(app)) = (weak_list.upgrade(), crate::window::app()) {
                    list.delete(&app, index);
                }
            });
            actions.add_action(&edit);
            actions.add_action(&delete);
            app.popup(&menu, list.tree.view.upcast_ref(), point, Some(&actions));
        });
    }

    fn row(&self, index: usize) -> Option<RowView> {
        self.rows.borrow().get(index).cloned()
    }

    fn edit(self: &Rc<Self>, app: &Rc<App>, index: usize) {
        let Some(row) = self.row(index) else { return };
        let shown = match app.core.lumenna.show_block(&row.id) {
            Ok(shown) => shown,
            Err(error) => return app.fail(&sentence(&error)),
        };
        let fields = Fields {
            title: shown.title,
            start: shown.start,
            minutes: shown.minutes.to_string(),
            kind: shown.kind,
            repeat: shown.repetition.unwrap_or_default(),
            rule: shown.rrule.filter(|_| shown.repeats),
        };
        let purpose = Purpose::Series { id: shown.id };
        let (list, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let window = app.window.clone().upcast::<gtk::Window>();
            if let Some(change) = block_form::run(&window, app.core.lumenna.clone(), purpose, fields).await {
                app.store_changed();
                list.tree.select_key_or_near(Some(&row.id), list.tree.selected());
                app.say_change(&change);
            }
        });
    }

    fn delete(self: &Rc<Self>, app: &Rc<App>, index: usize) {
        let Some(row) = self.row(index) else { return };
        let (list, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let heading = format!("Delete {}?", row.title);
            if prompts::confirm(&app.window, &heading, "Every occurrence goes, with what is assigned to it.", "Delete").await {
                let near = list.tree.selected();
                if let Some(change) = app.perform(|lumenna| lumenna.delete_block(&row.id)) {
                    list.tree.select_key_or_near(None, near);
                    app.say_change(&change);
                }
            }
        });
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
                    .map(|row| Item { key: row.id.clone(), text: speech::row(row, false), depth: 0 })
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
