//! What every list in the middle pane does alike — the tasks, the Blocks list, the day: a tree
//! of the app's rows, each with its actions in short, drawn as the app lands them, and what the
//! person does in it sent as the app's.
//!
//! Selecting a row sends `Focus`; Delete and Space send the core's keys (`keys::runs`), Space
//! staying the tree's own on a row it does nothing to, and a checkbox's flip being the tree's,
//! which then sends it; folding is the row's Expand or Collapse; and the context menu offers the
//! row's actions. A row's selection moves only when the app lands it (`view.landing`), or when
//! the rows were rebuilt under it, never for a sync or a minute.

use std::cell::{Cell, RefCell};

use lumenna_app::keys;
use lumenna_ffi::ShellEvent;
use lumenna_records::{ActionKind, Offered};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{
    NM_TVSTATEIMAGECHANGING, NMHDR, NMTREEVIEWW, NMTVKEYDOWN, NMTVSTATEIMAGECHANGING, TVIS_EXPANDED, TVN_ITEMEXPANDEDW,
    TVN_KEYDOWN, TVN_SELCHANGEDW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_SPACE};

use super::actions;
use super::app::App;
use super::controls;
use super::tree::{Item, Tree};

/// One row to draw: as the tree shows it, the app's record of it, and its actions in short.
pub struct Shown<R> {
    pub item: Item,
    pub row: R,
    pub offered: Vec<Offered>,
}

/// A tree of the app's rows of type `R`.
pub struct Rows<R> {
    pub tree: Tree,
    /// Whether its rows have the tree's checkboxes, which Space flips itself.
    checkboxes: bool,
    /// The rows as the app last gave them, those folded away included, in the tree's order.
    rows: RefCell<Vec<R>>,
    /// Each row's actions, as the app offers them in short, in the same order.
    offers: RefCell<Vec<Vec<Offered>>>,
    /// A row the tree itself just folded or unfolded, which the tree has already said.
    folded_by_tree: Cell<bool>,
}

impl<R: Clone> Rows<R> {
    /// Creates one in `pane`, named `name` for screen readers.
    pub fn create(pane: HWND, id: u16, name: &str, checkboxes: bool) -> Self {
        Self {
            tree: Tree::create(pane, id, name, checkboxes),
            checkboxes,
            rows: RefCell::new(Vec::new()),
            offers: RefCell::new(Vec::new()),
            folded_by_tree: Cell::new(false),
        }
    }

    /// The row selected, if any.
    pub fn selected(&self) -> Option<R> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// The selected row's key.
    pub fn selected_key(&self) -> Option<String> {
        self.tree.key(self.tree.selected()?)
    }

    /// The selected row's actions.
    pub fn selected_actions(&self) -> Option<Vec<Offered>> {
        let index = self.tree.selected()?;
        self.offers.borrow().get(index).cloned()
    }

    /// Enter or a double-click on the selected row: the core's Open (`keys::runs("open")`) as the
    /// row offers it — a task's details, a block's form, a new block in free time, a cancelled
    /// day restored — or, where it offers none, the app's refusal in its words.
    pub fn open(&self, app: &App) {
        let kinds = keys::runs("open").unwrap_or_default();
        let action = self.selected_actions().and_then(|offered| actions::of_kind(&offered, kinds));
        match (self.selected_key(), action) {
            (Some(key), Some(action)) => actions::act_on_row(app, &key, &action),
            _ => app.crux.send(ShellEvent::Key("open".into())),
        }
    }

    /// Whether the tree folded a row itself since this was last asked: the app's "Expanded" or
    /// "Collapsed" is then not said, since the tree reports the state.
    pub fn took_fold(&self) -> bool {
        self.folded_by_tree.replace(false)
    }

    /// Draws the app's rows: the row in hand (`focus`) selected when the app moved it itself
    /// (`landed`) or the rows were rebuilt, and focus into the tree on landing only when it is
    /// lost.
    pub fn draw(&self, app: &App, shown: Vec<Shown<R>>, focus: Option<&str>, landed: bool) {
        let mut items = Vec::with_capacity(shown.len());
        let mut rows = Vec::with_capacity(shown.len());
        let mut offers = Vec::with_capacity(shown.len());
        for Shown { item, row, offered } in shown {
            items.push(item);
            rows.push(row);
            offers.push(offered);
        }
        *self.rows.borrow_mut() = rows;
        *self.offers.borrow_mut() = offers;
        let rebuilt = self.tree.set(items);
        if (landed || rebuilt)
            && let Some(index) = focus.and_then(|key| self.tree.index_of(key))
        {
            self.tree.select_quietly(index);
        }
        if landed && controls::lost(app.main) {
            controls::focus(self.tree.hwnd);
        }
    }

    /// The tree's notifications every list answers alike, and what to return for each; `None`
    /// for any other.
    pub fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW if !self.tree.busy() => {
                if let Some(key) = self.selected_key() {
                    app.crux.send(ShellEvent::Focus(key));
                }
                Some(0)
            }
            NM_TVSTATEIMAGECHANGING if !self.tree.busy() => {
                // Space or a click on the checkbox, which the tree flips itself; the app's
                // redraw then says whether it took, and puts it back if not.
                let notice = unsafe { &*(lparam.0 as *const NMTVSTATEIMAGECHANGING) };
                if let Some(key) = self.tree.index_of_handle(notice.hti).and_then(|index| self.tree.key(index)) {
                    if self.selected_key().as_ref() != Some(&key) {
                        app.crux.send(ShellEvent::Focus(key));
                    }
                    app.crux.send(ShellEvent::Key("toggle".into()));
                }
                Some(0)
            }
            TVN_KEYDOWN => {
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                // What each runs is the core's (`keys::runs`): Delete removes what the row is, or
                // the core says why not. Space on a checkbox is the tree's, which flips it; on
                // another row it stays the tree's where the row has none of its actions.
                if key == VK_DELETE.0 {
                    app.crux.send(ShellEvent::Key("delete".into()));
                    // Not part of an incremental search.
                    return Some(1);
                }
                if key == VK_SPACE.0 && !self.checkboxes {
                    let kinds = keys::runs("toggle").unwrap_or_default();
                    if self.selected_actions().is_some_and(|offered| actions::of_kind(&offered, kinds).is_some()) {
                        app.crux.send(ShellEvent::Key("toggle".into()));
                        return Some(1);
                    }
                }
                Some(0)
            }
            TVN_ITEMEXPANDEDW if !self.tree.busy() => {
                // Folding is the app's: the tree's own Left, Right or click is the row's Expand
                // or Collapse.
                let notice = unsafe { &*(lparam.0 as *const NMTREEVIEWW) };
                let kind = if notice.itemNew.state.0 & TVIS_EXPANDED.0 != 0 { ActionKind::Expand } else { ActionKind::Collapse };
                let Some(index) = self.tree.index_of_handle(notice.itemNew.hItem) else { return Some(0) };
                let action = self.offers.borrow().get(index).and_then(|offers| actions::of_kind(offers, &[kind]));
                if let (Some(key), Some(action)) = (self.tree.key(index), action) {
                    self.folded_by_tree.set(true);
                    actions::fold(app, key, action, self.selected_key());
                }
                Some(0)
            }
            _ => None,
        }
    }

    /// A context menu on the tree, at a point on screen with the mouse or from the keyboard:
    /// the row's actions, the one chosen run with `act` (its key, its row, the action). Returns
    /// whether it was this tree's.
    pub fn context_menu(&self, app: &App, control: HWND, point: Option<POINT>, act: impl FnOnce(&str, &R, &Offered)) -> bool {
        if control != self.tree.hwnd {
            return false;
        }
        if let Some(index) = point.and_then(|p| self.tree.index_at(p)) {
            self.tree.select(index);
        }
        let Some(index) = self.tree.selected() else { return true };
        let (Some(key), Some(row), Some(offered)) = (self.tree.key(index), self.selected(), self.selected_actions()) else { return true };
        let items = actions::menu(&offered);
        if items.is_empty() {
            return true;
        }
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(&offered, command)) {
            act(&key, &row, &action);
        }
        true
    }
}
