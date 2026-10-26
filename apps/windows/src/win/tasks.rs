//! Tasks, with the filter above them, drawn from the app's view: its rows, its readback, the
//! row in hand and what it folds.
//!
//! A tree view, so a subtask's level and every row's position are the control's to report.
//! Space checks a task off — the tree's own checkbox, so its state is reported as a checkbox
//! — or, in the trash, restores it; Delete runs the row's Delete; Enter opens its details; and
//! the Applications key or Shift+F10 offers every action the core gives the row. Each runs as
//! the app's (`rows.rs`): the row in hand, then the action, its question the app's open one.

use std::cell::Cell;
use std::rc::Rc;

use lumenna_app::ViewModel;
use lumenna_app::lines::Lines;
use lumenna_ffi::ShellEvent;
use lumenna_records::{RowView, Syntax};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, WC_EDITW, WC_STATICW};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{EN_CHANGE, ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, WS_TABSTOP};

use super::app::App;
use super::controls::{self, rect};
use super::clock::Locale;
use super::rows::{Rows, Shown};
use super::tree::Item;
use super::view::{Metrics, View};
use super::{actions, completion};
use lumenna_records::places::Place;
use crate::speech;

const FILTER: u16 = 400;
const TREE: u16 = 401;

pub struct TaskList {
    pub place: Place,
    trash: bool,
    /// Whether the place lists under a filter: not the trash, nor History, which list what they hold.
    filtered: bool,
    label: HWND,
    filter: HWND,
    readback: HWND,
    pub rows: Rows<RowView>,
    /// Whether the filter has been filled from the app since the place opened: going back to a
    /// place puts back the filter it was left with.
    filled: Cell<bool>,
    /// The filter is being filled from the app, not typed.
    filling: Cell<bool>,
}

impl TaskList {
    pub fn create(_app: &App, pane: HWND, place: Place) -> Rc<Self> {
        let trash = place == Place::Trash;
        let filtered = !matches!(place, Place::Trash | Place::History);
        let label = controls::create(pane, WC_STATICW, "Filte&r:", 0, 0, 0);
        let filter = controls::create(pane, WC_EDITW, "", ES_AUTOHSCROLL as u32 | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, FILTER);
        completion::attach(filter, Syntax::Filter);
        let readback = controls::create(pane, WC_STATICW, "", SS_NOPREFIX.0, 0, 0);
        let rows = Rows::create(pane, TREE, &place.title(), !trash);
        // The trash is everything deleted, and History everything done lately, and nothing else:
        // there is no filter to change.
        controls::show(label, filtered);
        controls::show(filter, filtered);
        Rc::new(Self { place, trash, filtered, label, filter, readback, rows, filled: Cell::new(false), filling: Cell::new(false) })
    }

    /// Draws the app's list: `landed` when the app moved the row in hand itself.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, landed: bool) {
        let Some(list) = &view.list else { return };
        if !self.filled.replace(true) && self.filtered {
            self.filling.set(true);
            controls::set_text(self.filter, &view.query);
            self.filling.set(false);
        }
        controls::set_text(self.readback, &lines.readback);
        let shown = list
            .items
            .iter()
            .zip(&lines.list)
            .map(|(item, line)| Shown {
                item: Item {
                    key: item.row.id.clone(),
                    // The tree's checkbox says a task is done, so the line does not.
                    text: if self.trash { line.text.clone() } else { speech::row(&item.row, true, &Locale) },
                    depth: item.row.depth,
                    checked: (!self.trash).then_some(item.row.checked == Some(true)),
                    expanded: item.row.expanded,
                    swatch: item.row.swatch.clone(),
                },
                row: item.row.clone(),
                offered: actions::of_row(&list.offered, &item.actions),
            })
            .collect();
        self.rows.draw(app, shown, view.focus.as_deref(), landed);
    }

    /// What a new task typed here starts with.
    pub fn quick_add_prefix(&self) -> String {
        self.place.quick_add_prefix()
    }

    /// Whether it has a filter to change: not the trash, nor History.
    pub fn has_filter(&self) -> bool {
        self.filtered
    }

    pub fn focus_filter(&self) {
        if self.filtered {
            controls::focus(self.filter);
            controls::send(self.filter, windows::Win32::UI::Controls::EM_SETSEL, 0, -1);
        }
    }
}

impl View for TaskList {
    fn focus_target(&self) -> HWND {
        self.rows.tree.hwnd
    }

    fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        let inner = width - 2 * gap;
        let mut y = gap;
        if self.filtered {
            controls::place(self.label, rect(gap, y, inner, m.line));
            y += m.line + m.px(2);
            controls::place(self.filter, rect(gap, y, inner, m.field));
            y += m.field + gap;
        }
        controls::place(self.readback, rect(gap, y, inner, m.line * 2));
        y += m.line * 2 + gap;
        controls::place(self.rows.tree.hwnd, rect(gap, y, inner, (height - y - gap).max(m.line)));
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
        if header.hwndFrom == self.rows.tree.hwnd && header.code == NM_DBLCLK {
            if !self.trash && self.rows.selected().is_some() {
                app.open_detail();
            }
            return Some(1);
        }
        self.rows.notify(app, header, lparam)
    }

    fn context_menu(&self, app: &App, control: HWND, point: Option<POINT>) -> bool {
        self.rows.context_menu(app, control, point, |key, _, action| actions::act_on_row(app, key, action))
    }

    fn enter(&self, app: &App, focus: HWND) -> bool {
        if focus == self.filter {
            // Finishing the filter says what it found, then goes to it.
            app.say(&controls::text(self.readback));
            controls::focus(self.rows.tree.hwnd);
            return true;
        }
        if focus == self.rows.tree.hwnd && !self.trash && self.rows.selected().is_some() {
            app.open_detail();
            return true;
        }
        false
    }

    fn escape(&self, _app: &App, focus: HWND) -> bool {
        if focus == self.filter {
            controls::focus(self.rows.tree.hwnd);
            return true;
        }
        false
    }

    fn took_fold(&self) -> bool {
        self.rows.took_fold()
    }

    fn windows(&self) -> Vec<HWND> {
        vec![self.label, self.filter, self.readback, self.rows.tree.hwnd]
    }
}
