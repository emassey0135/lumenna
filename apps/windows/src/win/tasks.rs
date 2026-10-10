//! Tasks, with the filter above them.
//!
//! A tree view, so a subtask's level and every row's position are the control's to report.
//! Space checks a task off — the tree's own checkbox, so its state is reported as a checkbox
//! — or, in the trash, restores it; Delete runs the row's Delete; Enter opens its details; and
//! the Applications key or Shift+F10 offers every action the core gives the row. After a
//! change the selection, which is the screen reader's focus, lands on the same task if it is
//! still listed and otherwise on whatever now holds its place.

use std::cell::RefCell;
use std::rc::Rc;

use lumenna_surface::{Action, ActionKind, RowView, Syntax};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{
    NM_DBLCLK, NM_TVSTATEIMAGECHANGING, NMHDR, NMTREEVIEWW, NMTVKEYDOWN, NMTVSTATEIMAGECHANGING,
    TVN_ITEMEXPANDEDW, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_EDITW, WC_STATICW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_SPACE};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{EN_CHANGE, ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, WS_TABSTOP};

use super::app::App;
use super::controls::{self, rect};
use super::clock::Locale;
use super::core::sentence;
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::{actions, completion};
use lumenna_surface::places::Place;
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
    rows: RefCell<Vec<RowView>>,
}

impl TaskList {
    pub fn create(app: &App, pane: HWND, place: Place) -> Rc<Self> {
        let trash = place == Place::Trash;
        let label = controls::create(pane, WC_STATICW, "Filte&r", 0, 0, 0);
        let filter = controls::create(pane, WC_EDITW, &place.query(), ES_AUTOHSCROLL as u32 | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, FILTER);
        completion::attach(filter, app.core.lumenna.clone(), Syntax::Filter);
        let readback = controls::create(pane, WC_STATICW, "", SS_NOPREFIX.0, 0, 0);
        let tree = Tree::create(pane, TREE, &place.title(), !trash);
        // The trash is everything deleted, and nothing else: there is no filter to change.
        controls::show(label, !trash);
        controls::show(filter, !trash);
        Rc::new(Self { place, trash, label, filter, readback, tree, rows: RefCell::new(Vec::new()) })
    }

    /// The row selected, if any.
    pub fn selected(&self) -> Option<RowView> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// Lists again, keeping the selection. A filter still being typed may not read yet; then
    /// the old rows stay and the readback says what is wrong with it.
    fn list(&self, app: &App) {
        let key = self.tree.selected().and_then(|index| self.tree.key(index));
        let near = self.tree.selected().or(Some(0));
        let query = if self.trash { self.place.query() } else { controls::text(self.filter) };
        match app.core.lumenna.list_tasks(&query) {
            Ok(listing) => {
                let mut said = Vec::new();
                if !self.trash {
                    said.extend(listing.query.as_ref().map(|q| q.description.clone()));
                }
                said.push(listing.announcement.clone());
                said.extend(listing.notices.iter().cloned());
                controls::set_text(self.readback, &speech::sentence(&said.join(". ")));
                let items = listing
                    .rows
                    .iter()
                    .map(|row| Item {
                        key: row.id.clone(),
                        text: if self.trash { speech::trashed(row, &Locale) } else { speech::row(row, true, &Locale) },
                        depth: row.depth,
                        checked: (!self.trash).then_some(row.checked == Some(true)),
                    })
                    .collect();
                *self.rows.borrow_mut() = listing.rows;
                if self.tree.set(items) {
                    self.tree.select_key_or_near(key.as_deref(), near);
                }
            }
            Err(error) => controls::set_text(self.readback, &sentence(&error)),
        }
    }

    /// Runs one of a row's actions and says what it did. Reloading keeps the selection on the
    /// same task, or near where it was when the task has left the list.
    pub fn act(&self, app: &App, action: &Action) {
        if let Some((change, _)) = actions::run(app, app.main, action, || app.open_detail()) {
            app.say_change(&change);
        }
    }

    /// The selected row's action of one of `kinds`, if it has one: what a key means here.
    fn act_on_selected(&self, app: &App, kinds: &[ActionKind]) {
        if let Some(action) = self.selected().and_then(|row| actions::of_kind(&row.actions, kinds)) {
            self.act(app, &action);
        }
    }

    /// What a new task typed here starts with.
    pub fn quick_add_prefix(&self) -> String {
        self.place.quick_add_prefix()
    }

    /// Selects a task just made, if this list shows it.
    pub fn land_on(&self, id: &str) {
        self.tree.select_key_or_near(Some(id), None);
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

    fn reload(&self, app: &App) {
        self.list(app);
    }

    fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if control == self.filter && u32::from(code) == EN_CHANGE {
            // Typing is silent; the rows follow it, and Enter says what was found.
            self.list(app);
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
                let id = self.selected().filter(|_| !self.trash).map(|row| row.id);
                app.detail.show(app, id.as_deref());
                Some(0)
            }
            NM_TVSTATEIMAGECHANGING if !self.tree.busy() => {
                // Space or a click on the checkbox. The tree flips the box itself; the change is
                // made after the notification returns, since it rebuilds the tree.
                let notice = unsafe { &*(lparam.0 as *const NMTVSTATEIMAGECHANGING) };
                let row = self.tree.index_of_handle(notice.hti).and_then(|i| self.rows.borrow().get(i).cloned());
                let toggle = row.and_then(|row| actions::of_kind(&row.actions, &[ActionKind::MarkDone, ActionKind::MarkNotDone]));
                if let Some(action) = toggle {
                    app.defer(move |app| {
                        if let Some(list) = app.task_list() {
                            list.act(app, &action);
                        }
                    });
                }
                Some(0)
            }
            TVN_KEYDOWN => {
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                let kinds: &'static [ActionKind] = if key == VK_DELETE.0 {
                    &[ActionKind::Delete, ActionKind::DeleteForGood]
                } else if key == VK_SPACE.0 && self.trash {
                    &[ActionKind::Restore]
                } else {
                    return Some(0);
                };
                app.defer(move |app| {
                    if let Some(list) = app.task_list() {
                        list.act_on_selected(app, kinds);
                    }
                });
                // Not part of an incremental search.
                Some(1)
            }
            TVN_ITEMEXPANDEDW => {
                self.tree.expansion_changed(unsafe { &*(lparam.0 as *const NMTREEVIEWW) });
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
