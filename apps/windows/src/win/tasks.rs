//! Tasks, with the filter above them, drawn from the app's view: its rows, its readback, the
//! row in hand and what it folds.
//!
//! A tree view, so a subtask's level and every row's position are the control's to report.
//! Space checks a task off — the tree's own checkbox, so its state is reported as a checkbox
//! — or, in the trash, restores it; Delete runs the row's Delete; Enter opens its details; and
//! the Applications key or Shift+F10 offers every action the core gives the row. Each runs as
//! the app's: the row in hand, then the action, its question the app's open one.
//!
//! The selection moves only when the app lands somewhere (`view.landing`), or when the rows
//! were rebuilt under it; a redraw for a sync or a minute updates the rows in place and leaves
//! it alone.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::ViewModel;
use lumenna_app::lines::Lines;
use lumenna_ffi::ShellEvent;
use lumenna_records::{Action, ActionKind, Question, RowView, Syntax};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{
    NM_DBLCLK, NM_TVSTATEIMAGECHANGING, NMHDR, NMTREEVIEWW, NMTVKEYDOWN, NMTVSTATEIMAGECHANGING, TVIS_EXPANDED,
    TVN_ITEMEXPANDEDW, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_EDITW, WC_STATICW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_SPACE};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{EN_CHANGE, ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, WS_TABSTOP};

use super::app::App;
use super::controls::{self, rect};
use super::clock::Locale;
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::{actions, completion};
use lumenna_records::places::Place;
use crate::speech;

const FILTER: u16 = 400;
const TREE: u16 = 401;

pub struct TaskList {
    pub place: Place,
    trash: bool,
    label: HWND,
    filter: HWND,
    readback: HWND,
    pub tree: Tree,
    /// The rows as the app last listed them, those folded away included, in the tree's order.
    rows: RefCell<Vec<RowView>>,
    /// Whether the filter has been filled from the app since the place opened: going back to a
    /// place puts back the filter it was left with.
    filled: Cell<bool>,
    /// The filter is being filled from the app, not typed.
    filling: Cell<bool>,
    /// A task to land on once the app lists it: one quick add has just made.
    pending: RefCell<Option<String>>,
    /// A row the tree itself just folded or unfolded, which the tree has already said.
    pub folded_by_tree: Cell<bool>,
}

impl TaskList {
    pub fn create(_app: &App, pane: HWND, place: Place) -> Rc<Self> {
        let trash = place == Place::Trash;
        let label = controls::create(pane, WC_STATICW, "Filte&r:", 0, 0, 0);
        let filter = controls::create(pane, WC_EDITW, "", ES_AUTOHSCROLL as u32 | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, FILTER);
        completion::attach(filter, Syntax::Filter);
        let readback = controls::create(pane, WC_STATICW, "", SS_NOPREFIX.0, 0, 0);
        let tree = Tree::create(pane, TREE, &place.title(), !trash);
        // The trash is everything deleted, and nothing else: there is no filter to change.
        controls::show(label, !trash);
        controls::show(filter, !trash);
        Rc::new(Self {
            place,
            trash,
            label,
            filter,
            readback,
            tree,
            rows: RefCell::new(Vec::new()),
            filled: Cell::new(false),
            filling: Cell::new(false),
            pending: RefCell::new(None),
            folded_by_tree: Cell::new(false),
        })
    }

    /// The row selected, if any.
    pub fn selected(&self) -> Option<RowView> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// Draws the app's list: `landed` when the app moved the row in hand itself.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, landed: bool) {
        let Some(list) = &view.list else { return };
        if !self.filled.replace(true) && !self.trash {
            self.filling.set(true);
            controls::set_text(self.filter, &view.query);
            self.filling.set(false);
        }
        controls::set_text(self.readback, &lines.readback);
        let items = list
            .items
            .iter()
            .zip(&lines.list)
            .map(|(item, line)| Item {
                key: item.row.id.clone(),
                // The tree's checkbox says a task is done, so the line does not.
                text: if self.trash { line.text.clone() } else { speech::row(&item.row, true, &Locale) },
                depth: item.row.depth,
                checked: (!self.trash).then_some(item.row.checked == Some(true)),
                expanded: item.row.expanded,
            })
            .collect();
        *self.rows.borrow_mut() = list.items.iter().map(|item| item.row.clone()).collect();
        let rebuilt = self.tree.set(items);
        let pending = self.pending.borrow().clone();
        if let Some(id) = pending.filter(|id| self.tree.index_of(id).is_some()) {
            // Made here, and now listed: the person works on it next.
            self.pending.replace(None);
            app.crux.send(ShellEvent::Focus(id.clone()));
            self.show(Some(&id));
            return;
        }
        if landed || rebuilt {
            self.show(view.focus.as_deref());
        }
        if landed && controls::lost(app.main) {
            controls::focus(self.tree.hwnd);
        }
    }

    /// Selects a row, without its being taken for the person's move, and shows its details.
    fn show(&self, key: Option<&str>) {
        if let Some(index) = key.and_then(|key| self.tree.index_of(key)) {
            self.tree.select_quietly(index);
        }
    }

    /// Runs one of a row's actions, as the app's. A task's own form is the details pane, and
    /// its reminders a dialog of the window's.
    pub fn act(&self, app: &App, action: &Action) {
        match (&action.question, action.kind) {
            (Question::Form, ActionKind::Reminders) => app.crux.send(ShellEvent::Perform(action.clone())),
            (Question::Form, _) => app.open_detail(),
            _ => app.crux.send(ShellEvent::Perform(action.clone())),
        }
    }

    /// What a new task typed here starts with.
    pub fn quick_add_prefix(&self) -> String {
        self.place.quick_add_prefix()
    }

    /// Lands on a task just made, once the app lists it.
    pub fn land_on(&self, id: &str) {
        self.pending.replace(Some(id.to_owned()));
    }

    pub fn focus_filter(&self) {
        if !self.trash {
            controls::focus(self.filter);
            controls::send(self.filter, windows::Win32::UI::Controls::EM_SETSEL, 0, -1);
        }
    }
}

impl View for TaskList {
    fn focus_target(&self) -> HWND {
        self.tree.hwnd
    }

    fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        let inner = width - 2 * gap;
        let mut y = gap;
        if !self.trash {
            controls::place(self.label, rect(gap, y, inner, m.line));
            y += m.line + m.px(2);
            controls::place(self.filter, rect(gap, y, inner, m.field));
            y += m.field + gap;
        }
        controls::place(self.readback, rect(gap, y, inner, m.line * 2));
        y += m.line * 2 + gap;
        controls::place(self.tree.hwnd, rect(gap, y, inner, (height - y - gap).max(m.line)));
    }


    fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if control == self.filter && u32::from(code) == EN_CHANGE {
            // Typing is silent; the rows follow it, and Enter says what was found.
            if !self.filling.get() {
                app.crux.send(ShellEvent::Filter(controls::text(self.filter)));
            }
            return true;
        }
        false
    }

    fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW if !self.tree.busy() => {
                if let Some(row) = self.selected() {
                    app.crux.send(ShellEvent::Focus(row.id.clone()));
                }
                Some(0)
            }
            NM_TVSTATEIMAGECHANGING if !self.tree.busy() => {
                // Space or a click on the checkbox, which the tree flips itself; the app's
                // redraw then says whether it took.
                let notice = unsafe { &*(lparam.0 as *const NMTVSTATEIMAGECHANGING) };
                let row = self.tree.index_of_handle(notice.hti).and_then(|i| self.rows.borrow().get(i).cloned());
                if let Some(row) = row {
                    if self.selected().is_none_or(|selected| selected.id != row.id) {
                        app.crux.send(ShellEvent::Focus(row.id));
                    }
                    app.crux.send(ShellEvent::Run(vec![ActionKind::MarkDone, ActionKind::MarkNotDone]));
                }
                Some(0)
            }
            TVN_KEYDOWN => {
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                let kinds = if key == VK_DELETE.0 {
                    vec![ActionKind::Delete, ActionKind::DeleteForGood]
                } else if key == VK_SPACE.0 && self.trash {
                    vec![ActionKind::Restore]
                } else {
                    return Some(0);
                };
                app.crux.send(ShellEvent::Run(kinds));
                // Not part of an incremental search.
                Some(1)
            }
            TVN_ITEMEXPANDEDW if !self.tree.busy() => {
                // Folding is the app's: the tree's own Left, Right or click is its Expand or
                // Collapse.
                let notice = unsafe { &*(lparam.0 as *const NMTREEVIEWW) };
                let expanded = notice.itemNew.state.0 & TVIS_EXPANDED.0 != 0;
                let kind = if expanded { ActionKind::Expand } else { ActionKind::Collapse };
                let row = self.tree.index_of_handle(notice.itemNew.hItem).and_then(|i| self.rows.borrow().get(i).cloned());
                if let Some(action) = row.and_then(|row| actions::of_kind(&row.actions, &[kind])) {
                    self.folded_by_tree.set(true);
                    app.crux.send(ShellEvent::Perform(action));
                }
                Some(0)
            }
            NM_DBLCLK => {
                if !self.trash && self.selected().is_some() {
                    app.open_detail();
                }
                Some(1)
            }
            _ => None,
        }
    }

    fn context_menu(&self, app: &App, control: HWND, point: Option<POINT>) -> bool {
        if control != self.tree.hwnd {
            return false;
        }
        if let Some(index) = point.and_then(|p| self.tree.index_at(p)) {
            self.tree.select(index);
        }
        let Some(index) = self.tree.selected() else { return true };
        let Some(row) = self.selected() else { return true };
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        let items = actions::menu(&row.actions);
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(&row.actions, command)) {
            self.act(app, &action);
        }
        true
    }

    fn enter(&self, app: &App, focus: HWND) -> bool {
        if focus == self.filter {
            // Finishing the filter says what it found, then goes to it.
            app.say(&controls::text(self.readback));
            controls::focus(self.tree.hwnd);
            return true;
        }
        if focus == self.tree.hwnd && !self.trash && self.selected().is_some() {
            app.open_detail();
            return true;
        }
        false
    }

    fn properties(&self, app: &App) -> bool {
        if self.trash || self.selected().is_none() {
            return false;
        }
        app.open_detail();
        true
    }

    fn escape(&self, _app: &App, focus: HWND) -> bool {
        if focus == self.filter {
            controls::focus(self.tree.hwnd);
            return true;
        }
        false
    }

    fn windows(&self) -> Vec<HWND> {
        vec![self.label, self.filter, self.readback, self.tree.hwnd]
    }
}
