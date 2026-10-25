//! The places: Today, Tasks, the projects as they nest, labels, saved filters, Blocks, the
//! trash, drawn from the app's view (`view.sidebar`), with its row in hand (`sidebar_held`) and
//! its folding.
//!
//! A tree view like every list here, so a subproject's level is the control's to report.
//! Moving through it shows each place in the middle pane, as Explorer's folder tree does.
//! What a project, label, filter or heading can have done to it is the core's, offered in its
//! context menu and run as the app's, the row held first so the change lands back here; Delete
//! runs a row's Delete and F2 its Rename.

use std::cell::{Cell, RefCell};

use lumenna_app::{ActingOn, Side, ViewModel};
use lumenna_ffi::ShellEvent;
use lumenna_records::{ActionKind, Offered};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{
    NMHDR, NMTREEVIEWW, NMTVKEYDOWN, TVIS_EXPANDED, TVN_ITEMEXPANDEDW, TVN_KEYDOWN, TVN_SELCHANGEDW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_F2};

use super::app::App;
use super::controls::{self, rect};
use super::tree::{Item, Tree};
use super::view::Metrics;
use super::actions;
use lumenna_records::places::{Place, SidebarEntry, SidebarGroup, SidebarKind};

const TREE: u16 = 200;
pub struct Sidebar {
    pub tree: Tree,
    entries: RefCell<Vec<SidebarEntry>>,
    /// The place shown when the sidebar was last drawn, by key.
    shown: RefCell<String>,
    /// The landing count of the sidebar's row in hand when last drawn.
    landing: Cell<u32>,
    /// A row the tree itself just folded or unfolded, which the tree has already said.
    pub folded_by_tree: Cell<bool>,
}

impl Sidebar {
    pub fn create(pane: HWND) -> Self {
        Self {
            tree: Tree::create(pane, TREE, "Places", false),
            entries: RefCell::new(Vec::new()),
            shown: RefCell::new(String::new()),
            landing: Cell::new(0),
            folded_by_tree: Cell::new(false),
        }
    }

    pub fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        controls::place(self.tree.hwnd, rect(gap, gap, width - gap, height - 2 * gap));
    }

    /// Draws the app's sidebar. The selection moves to the row the app holds when it moved it
    /// (a place renamed, made or deleted), to the place shown when another was opened, and
    /// otherwise stays where the person left it.
    pub fn draw(&self, app: &App, view: &ViewModel) {
        if !view.ready {
            return;
        }
        let items = view
            .sidebar
            .iter()
            .map(|entry| Item {
                key: entry.key(),
                text: entry.text.clone(),
                depth: entry.depth,
                checked: None,
                expanded: fold(entry).map(|kind| kind == ActionKind::Collapse),
                swatch: entry.swatch.clone(),
            })
            .collect();
        *self.entries.borrow_mut() = view.sidebar.clone();
        let rebuilt = self.tree.set(items);
        let place = view.place.as_ref().map(place_key).unwrap_or_default();
        let opened = *self.shown.borrow() != place;
        *self.shown.borrow_mut() = place.clone();
        let held = &view.sidebar_held;
        let landed = held.landing != self.landing.replace(held.landing);
        let target = if landed {
            held.focus.clone()
        } else if opened {
            Some(place)
        } else if rebuilt {
            held.focus.clone().or(Some(place))
        } else {
            None
        };
        if let Some(index) = target.and_then(|key| self.tree.index_of(&key)) {
            self.tree.select_quietly(index);
        }
        if landed && controls::lost(app.main) {
            controls::focus(self.tree.hwnd);
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
                if let Some(entry) = self.selected() {
                    app.crux.send(ShellEvent::Hold(Side::Sidebar, entry.key()));
                    if let SidebarKind::Place(place) = entry.kind
                        && *self.shown.borrow() != place_key(&place)
                    {
                        app.show_place(place);
                    }
                }
                Some(0)
            }
            TVN_ITEMEXPANDEDW if !self.tree.busy() => {
                // Folding is the app's: the tree's own Left, Right or click is its Expand or
                // Collapse.
                let notice = unsafe { &*(lparam.0 as *const NMTREEVIEWW) };
                let kind = if notice.itemNew.state.0 & TVIS_EXPANDED.0 != 0 { ActionKind::Expand } else { ActionKind::Collapse };
                let entry = self.tree.index_of_handle(notice.itemNew.hItem).and_then(|i| self.entries.borrow().get(i).cloned());
                if let Some(entry) = entry
                    && let Some(action) = actions::of_kind(&entry.offered, &[kind])
                {
                    self.folded_by_tree.set(true);
                    // A click can fold an entry other than the one selected, which the app
                    // holds to fold it: the selected one is held again after.
                    let selected = self.selected().map(|selected| selected.key()).filter(|selected| *selected != entry.key());
                    self.act(app, &entry, action);
                    if let Some(selected) = selected {
                        app.crux.send(ShellEvent::Hold(Side::Sidebar, selected));
                    }
                }
                Some(0)
            }
            TVN_KEYDOWN => {
                // Delete and F2 do what they do in Explorer: the row's own Delete, asking first
                // as the context menu's does, and its Rename. Where there is none, as for the
                // Inbox, the app says why.
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                let kind = match key {
                    // What Delete runs is the core's (`keys::runs`).
                    k if k == VK_DELETE.0 => {
                        if let Some(entry) = self.selected().filter(|entry| !entry.offered.is_empty()) {
                            app.crux.send(ShellEvent::Hold(Side::Sidebar, entry.key()));
                            app.crux.send(ShellEvent::Key("delete".into()));
                        }
                        return Some(1);
                    }
                    k if k == VK_F2.0 => ActionKind::Rename,
                    _ => return Some(0),
                };
                if let Some(entry) = self.selected() {
                    match actions::of_kind(&entry.offered, &[kind]) {
                        Some(action) => self.act(app, &entry, action),
                        // A heading has no actions, and stays quiet.
                        None if !entry.offered.is_empty() => {
                            app.crux.send(ShellEvent::Hold(Side::Sidebar, entry.key()));
                            app.crux.send(ShellEvent::Run(vec![kind]));
                        }
                        None => {}
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
        let items = actions::menu(&entry.offered);
        if items.is_empty() {
            return true;
        }
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(&entry.offered, command)) {
            self.act(app, &entry, action);
        }
        true
    }

    /// Adds a project, label or saved filter, as its heading's New does: for the File menu.
    pub fn add(&self, app: &App, group: SidebarGroup) {
        let key = SidebarEntry::key_of(&SidebarKind::Group(group));
        let entry = self.entries.borrow().iter().find(|entry| entry.key() == key).cloned();
        // A heading's own action comes first, before its folding.
        if let Some(entry) = entry
            && let Some(action) = entry.offered.first().cloned()
        {
            self.act(app, &entry, action);
        }
    }

    /// Runs one of a row's actions as the app's, the row held first: what it renames, makes or
    /// removes, the app goes to, and the window with it.
    fn act(&self, app: &App, entry: &SidebarEntry, action: Offered) {
        app.crux.send(ShellEvent::ActOn(ActingOn::Held(Side::Sidebar, entry.key()), action));
    }
}

/// The Expand or Collapse a row offers, when something is under it.
fn fold(entry: &SidebarEntry) -> Option<ActionKind> {
    entry.offered.iter().map(|action| action.kind).find(|kind| matches!(kind, ActionKind::Expand | ActionKind::Collapse))
}

pub fn place_key(place: &Place) -> String {
    SidebarEntry::key_of(&SidebarKind::Place(place.clone()))
}
