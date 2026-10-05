//! Tasks, with the filter above them (§16.1: task list + filter entry).
//!
//! A tree view, so a subtask's level and every row's position are the control's to report.
//! Space checks a task off — the tree's own checkbox, so its state is reported as a checkbox
//! — Delete trashes it, Enter opens its details, and the Applications key or Shift+F10 opens
//! everything else. After a change the selection, which is the screen reader's focus, lands on
//! the same task if it is still listed and otherwise on whatever now holds its place (§13).

use std::cell::RefCell;
use std::rc::Rc;

use lumenna_surface::{MoveTarget, RowView, Syntax};
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
use super::core::sentence;
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::{completion, menu, prompts};
use crate::places::Place;
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
                        text: if self.trash { speech::trashed(row) } else { speech::row(row, true) },
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

    /// Runs a change, puts the selection on `keep` or near where it was, and says what
    /// happened.
    fn perform(&self, app: &App, keep: Option<&str>, operation: impl FnOnce(&lumenna_surface::Lumenna) -> lumenna_surface::Result<lumenna_surface::Change>) {
        let near = self.tree.selected();
        if let Some(change) = app.perform(operation) {
            self.tree.select_key_or_near(keep, near);
            app.say_change(&change);
        }
    }

    pub fn toggle_done(&self, app: &App, id: &str, done: bool) {
        self.perform(app, Some(id), |lumenna| {
            if done { lumenna.uncomplete_task(id) } else { lumenna.complete_task(id) }
        });
    }

    pub fn mark_selected_done(&self, app: &App) {
        if let Some(row) = self.selected().filter(|_| !self.trash) {
            self.toggle_done(app, &row.id, row.checked == Some(true));
        }
    }

    pub fn trash_selected(&self, app: &App) {
        if let Some(row) = self.selected().filter(|_| !self.trash) {
            self.perform(app, None, |lumenna| lumenna.trash_task(&row.id));
        }
    }

    pub fn restore_selected(&self, app: &App) {
        if let Some(row) = self.selected().filter(|_| self.trash) {
            self.perform(app, None, |lumenna| lumenna.restore_task(&row.id));
        }
    }

    /// Erasing rebuilds the document without the task and cannot be undone (§9), so it asks.
    pub fn erase_selected(&self, app: &App) {
        let Some(row) = self.selected().filter(|_| self.trash) else { return };
        if prompts::confirm(
            app.main,
            &format!("Erase {}?", row.title),
            "It and its history are deleted for good. This cannot be undone.",
            "Erase",
        ) {
            self.perform(app, None, |lumenna| lumenna.erase_task(&row.id));
        }
    }

    pub fn move_selected_to_project(&self, app: &App) {
        let Some(row) = self.selected().filter(|_| !self.trash) else { return };
        let projects: Vec<String> =
            app.core.lumenna.list_projects().map(|r| r.rows.into_iter().map(|p| p.title).collect()).unwrap_or_default();
        if let Some(index) = prompts::pick(app.main, &format!("Move {}", row.title), "&Project:", &projects) {
            let name = projects[index].clone();
            self.perform(app, Some(&row.id), |lumenna| lumenna.move_task(&row.id, MoveTarget::Project { name }));
        }
    }

    pub fn make_selected_subtask(&self, app: &App) {
        let Some(row) = self.selected().filter(|_| !self.trash) else { return };
        let others: Vec<RowView> = app
            .core
            .lumenna
            .list_tasks("")
            .map(|r| r.rows.into_iter().filter(|t| t.id != row.id).collect())
            .unwrap_or_default();
        let titles: Vec<String> = others.iter().map(|t| speech::row(t, false)).collect();
        if let Some(index) = prompts::pick(app.main, &format!("Make {} a Subtask", row.title), "Of &task:", &titles) {
            let parent = others[index].id.clone();
            self.perform(app, Some(&row.id), |lumenna| lumenna.move_task(&row.id, MoveTarget::Parent { id: parent }));
        }
    }

    pub fn move_selected_to_top(&self, app: &App) {
        if let Some(row) = self.selected().filter(|_| !self.trash) {
            self.perform(app, Some(&row.id), |lumenna| lumenna.move_task(&row.id, MoveTarget::Top));
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
                if let Some(row) = row {
                    app.defer(move |app| {
                        if let Some(list) = app.task_list() {
                            list.toggle_done(app, &row.id, row.checked == Some(true));
                        }
                    });
                }
                Some(0)
            }
            TVN_KEYDOWN => {
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                if key == VK_DELETE.0 {
                    let trash = self.trash;
                    app.defer(move |app| {
                        if let Some(list) = app.task_list() {
                            if trash { list.erase_selected(app) } else { list.trash_selected(app) }
                        }
                    });
                    return Some(1);
                }
                if key == VK_SPACE.0 && self.trash {
                    app.defer(|app| {
                        if let Some(list) = app.task_list() {
                            list.restore_selected(app);
                        }
                    });
                    // Not part of an incremental search.
                    return Some(1);
                }
                Some(0)
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
        let items: Vec<(u16, &str)> = if self.trash {
            vec![(menu::RESTORE_TASK, "&Restore"), (menu::ERASE_TASK, "&Erase for Good...")]
        } else {
            let done = if row.checked == Some(true) { "&Mark Not Done" } else { "&Mark Done" };
            let mut items = vec![
                (menu::MARK_DONE, done),
                (menu::OPEN_TASK, "&Edit Details"),
                (0, ""),
                (menu::MOVE_TO_PROJECT, "Move to &Project..."),
                (menu::MAKE_SUBTASK, "Make Su&btask Of..."),
            ];
            if row.depth > 0 || row.state.iter().any(|s| s == "subtask") {
                items.push((menu::MOVE_TO_TOP, "Move to &Top Level"));
            }
            items.extend([(0, ""), (menu::TRASH_TASK, "Move to T&rash")]);
            items
        };
        // The same commands as the menu bar's, acting on the same selection.
        if let Some(command) = app.popup(&items, at) {
            app.menu_command(command);
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
