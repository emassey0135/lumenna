//! The places (§16.1: project tree, label list, saved filters): Today, Tasks, the projects as
//! they nest, labels, saved filters, Blocks, the trash.
//!
//! A tree like every list here, so a subproject's level is the tree's to report. Moving
//! through it shows each place in the middle pane, as Files' sidebar does; Enter goes into it.

use std::cell::RefCell;
use std::rc::Rc;

use lumenna_desktop::places::{self, Entry, Kind, Place};

use crate::tree::{Item, Tree};
use crate::window::App;

pub struct Sidebar {
    pub tree: Rc<Tree>,
    entries: RefCell<Vec<Entry>>,
    /// The place shown, by key.
    current: RefCell<String>,
}

impl Sidebar {
    pub fn new() -> Rc<Self> {
        let sidebar = Rc::new(Self { tree: Tree::new("Places"), entries: RefCell::new(Vec::new()), current: RefCell::new(String::new()) });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_selected(move |_| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            if let Some(Entry { kind: Kind::Place(place), .. }) = sidebar.selected() {
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
            if matches!(sidebar.selected(), Some(Entry { kind: Kind::Place(_), .. })) {
                app.focus_content();
            }
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

    fn selected(&self) -> Option<Entry> {
        let index = self.tree.selected()?;
        self.entries.borrow().get(index).cloned()
    }
}

/// The key a place's row has in the sidebar.
fn place_key(place: &Place) -> String {
    Entry { kind: Kind::Place(place.clone()), text: String::new(), depth: 0, archived: false }.key()
}
