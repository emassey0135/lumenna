//! The places: Today, Tasks, the projects as
//! they nest, labels, saved filters, Blocks, the trash.
//!
//! A tree view like every list here, so a subproject's level is the control's to report.
//! Moving through it shows each place in the middle pane, as Explorer's folder tree does.
//! What a project, label, filter or heading can have done to it is the core's, offered in its
//! context menu; Delete runs a row's Delete.

use std::cell::RefCell;

use lumenna_surface::actions::heading;
use lumenna_surface::{Action, ActionKind, Answer, Subject, new_filter_questions};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{
    NMHDR, NMTREEVIEWW, NMTVKEYDOWN, TVN_ITEMEXPANDEDW, TVN_KEYDOWN, TVN_SELCHANGEDW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_F2};

use super::app::App;
use super::controls::{self, rect};
use super::tree::{Item, Tree};
use super::view::Metrics;
use super::actions;
use lumenna_surface::places::{self, Place, SidebarEntry, SidebarGroup, SidebarKind};

const TREE: u16 = 200;
pub struct Sidebar {
    pub tree: Tree,
    entries: RefCell<Vec<SidebarEntry>>,
    /// The place shown, by key.
    current: RefCell<String>,
}

impl Sidebar {
    pub fn create(pane: HWND) -> Self {
        Self { tree: Tree::create(pane, TREE, "Places", false), entries: RefCell::new(Vec::new()), current: RefCell::new(String::new()) }
    }

    pub fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        controls::place(self.tree.hwnd, rect(gap, gap, width - gap, height - 2 * gap));
    }

    /// Lists the places again, keeping the one shown selected without showing it again —
    /// which would throw away whatever the middle pane was doing.
    pub fn reload(&self, app: &App) {
        let entries = places::sidebar(&app.core.lumenna);
        let items = entries
            .iter()
            .map(|entry| Item { key: entry.key(), text: entry.text.clone(), depth: entry.depth, checked: None })
            .collect();
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

    pub fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW if !self.tree.busy() => {
                if let Some(SidebarEntry { kind: SidebarKind::Place(place), .. }) = self.selected() {
                    let key = place_key(&place);
                    if *self.current.borrow() != key {
                        *self.current.borrow_mut() = key;
                        app.show_place(place);
                    }
                }
                Some(0)
            }
            TVN_ITEMEXPANDEDW => {
                self.tree.expansion_changed(unsafe { &*(lparam.0 as *const NMTREEVIEWW) });
                Some(0)
            }
            TVN_KEYDOWN => {
                // Delete and F2 do what they do in Explorer: the row's own Delete, asking first
                // as the context menu's does, and its Rename. After the notification returns,
                // since either rebuilds the tree. Where there is none, as for the Inbox, the
                // core says why.
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                let kind = match key {
                    k if k == VK_DELETE.0 => ActionKind::Delete,
                    k if k == VK_F2.0 => ActionKind::Rename,
                    _ => return Some(0),
                };
                if let Some(entry) = self.selected() {
                    match actions::of_kind(&entry.actions, &[kind]) {
                        Some(action) => app.defer(move |app| app.sidebar.act(app, &entry, &action)),
                        None => actions::say_not_offered(app, &entry.actions, kind),
                    }
                }
                Some(1)
            }
            _ => None,
        }
    }

    pub fn context_menu(&self, app: &App, control: HWND, point: Option<POINT>) -> bool {
        if control != self.tree.hwnd {
            return false;
        }
        let index = point.and_then(|p| self.tree.index_at(p)).or_else(|| self.tree.selected());
        let Some(index) = index else { return true };
        let Some(entry) = self.entries.borrow().get(index).cloned() else { return true };
        let items = actions::menu(&entry.actions);
        if items.is_empty() {
            return true;
        }
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(&entry.actions, command)) {
            self.act(app, &entry, &action);
        }
        true
    }

    /// Adds a project, label or saved filter, as its heading's New does: for the File menu.
    pub fn add(&self, app: &App, group: SidebarGroup) {
        let entry = SidebarEntry { kind: SidebarKind::Group(group), text: String::new(), depth: 0, archived: false, actions: Vec::new() };
        if let Some(action) = heading(group).into_iter().next() {
            self.act(app, &entry, &action);
        }
    }

    /// Runs one of a row's actions, says what it did, and — when it renamed or removed the
    /// place, or made a new one — goes there, or to Tasks.
    fn act(&self, app: &App, entry: &SidebarEntry, action: &Action) {
        let Some((change, answer)) = actions::run(app, app.main, action, || self.new_filter(app)) else { return };
        let query = match &entry.kind {
            SidebarKind::Place(Place::Filter { query, .. }) => query.clone(),
            _ => String::new(),
        };
        let then = match (action.subject, action.kind, answer) {
            (_, ActionKind::Delete, _) => Some(Place::Tasks),
            (Subject::Project, ActionKind::Rename | ActionKind::New | ActionKind::NewInside, Answer::Text { text }) => {
                Some(Place::Project(text))
            }
            (Subject::Label, ActionKind::Rename | ActionKind::New, Answer::Text { text }) => Some(Place::Label(text)),
            (Subject::Label, ActionKind::MergeInto, Answer::Picked { id, .. }) => Some(Place::Label(id)),
            (Subject::Filter, ActionKind::Rename, Answer::Text { text }) => Some(Place::Filter { name: text, query }),
            (Subject::Filter, ActionKind::ChangeQuery, Answer::Text { text }) => {
                Some(Place::Filter { name: action.target.clone(), query: text })
            }
            _ => None,
        };
        if let Some(place) = then.filter(|_| change.changed) {
            app.go(place, false);
        }
        app.say_change(&change);
    }

    /// A new saved filter: the app's own form, a name and then its query.
    fn new_filter(&self, app: &App) {
        let questions = new_filter_questions();
        let (Some(naming), Some(querying)) = (questions.first(), questions.get(1)) else { return };
        let Some(name) = actions::ask(app.main, naming, "").filter(|n| !n.is_empty()) else { return };
        let mut typed = String::new();
        while let Some(query) = actions::ask(app.main, querying, &typed) {
            match app.core.lumenna.add_filter(&name, &query) {
                Ok(change) => {
                    app.store_changed();
                    if change.changed {
                        app.go(Place::Filter { name: name.clone(), query }, false);
                    }
                    app.say_change(&change);
                    return;
                }
                Err(error) => {
                    app.fail(&super::core::sentence(&error));
                    typed = query;
                }
            }
        }
    }
}

fn place_key(place: &Place) -> String {
    SidebarEntry { kind: SidebarKind::Place(place.clone()), text: String::new(), depth: 0, archived: false, actions: Vec::new() }.key()
}