//! The planner: a day as it is lived, as a tree, drawn from the app's view (`view.day`): which
//! day, its rows, the row in hand and what is folded are the app's.
//!
//! The first row is the summary — what a glance at a timeline gives a sighted user. Then
//! blocks in time order with their sittings beneath them, free time as rows of its own, and
//! now as a position rather than a highlight. Today opens on now, not on midnight.
//!
//! On a sitting, Space starts or stops its timer and Delete takes it out of the block; on a
//! block, Enter changes it and Delete deletes it; on free time, Enter adds a block there.
//! Everything else is in the row's context menu. What can be done to the day itself is the
//! summary row's, and the Day menu's. Each runs as the app's (`rows.rs`), but for a sitting's
//! task, whose form is the details.

use std::rc::Rc;

use lumenna_app::lines::Lines;
use lumenna_app::{BlockEvent, DayItem, DayKind, DayMove, ViewModel};
use lumenna_ffi::ShellEvent;
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, WC_BUTTONW};
use windows::Win32::UI::WindowsAndMessaging::{BN_CLICKED, BS_PUSHBUTTON, WS_TABSTOP};

use super::app::App;
use super::controls::{self, rect};
use super::rows::{Rows, Shown};
use super::tree::Item;
use super::view::{Metrics, View};
use super::actions;
use super::menu;

const PREVIOUS: u16 = 500;
const TODAY: u16 = 501;
const NEXT: u16 = 502;
const ADD: u16 = 503;
const TREE: u16 = 504;

pub struct DayView {
    buttons: [HWND; 4],
    pub rows: Rows<DayItem>,
}

impl DayView {
    pub fn create(_app: &App, pane: HWND) -> Rc<Self> {
        // Each the command's own, as the menus name it.
        let button = |command: &str, id: u16| {
            controls::create(pane, WC_BUTTONW, &menu::button(command), BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, id)
        };
        let buttons = [
            button("previous-day", PREVIOUS),
            button("go-today", TODAY),
            button("next-day", NEXT),
            button("new-block", ADD),
        ];
        Rc::new(Self { buttons, rows: Rows::create(pane, TREE, "The day", false) })
    }

    /// Draws the app's day: `landed` when the app moved the row in hand itself.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, landed: bool) {
        let Some(day) = &view.day else { return };
        app.set_title(&lines.day_title);
        let shown = day
            .items
            .iter()
            .zip(&lines.day)
            .map(|(item, line)| Shown {
                item: Item {
                    key: item.key.clone(),
                    text: line.text.clone(),
                    depth: item.depth,
                    checked: None,
                    expanded: item.expanded,
                    // A block's colour beside it; its sittings beneath go without.
                    swatch: item.block.as_ref().filter(|_| item.kind == DayKind::Block).and_then(|block| block.swatch.clone()),
                },
                row: item.clone(),
                offered: item.offered.clone(),
            })
            .collect();
        self.rows.draw(app, shown, view.focus.as_deref(), landed);
    }

}

impl View for DayView {
    fn focus_target(&self) -> HWND {
        self.rows.tree.hwnd
    }

    fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        let mut x = gap;
        let widths = [m.px(100), m.px(64), m.px(80), m.px(96)];
        for (button, width) in self.buttons.iter().zip(widths) {
            controls::place(*button, rect(x, gap, width, m.button));
            x += width + m.px(6);
        }
        let y = gap + m.button + gap;
        controls::place(self.rows.tree.hwnd, rect(gap, y, width - 2 * gap, (height - y - gap).max(m.line)));
    }

    fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if u32::from(code) != BN_CLICKED {
            return false;
        }
        match self.buttons.iter().position(|b| *b == control) {
            Some(0) => app.crux.send(ShellEvent::Day(DayMove::Previous)),
            Some(1) => app.crux.send(ShellEvent::Day(DayMove::Now)),
            Some(2) => app.crux.send(ShellEvent::Day(DayMove::Next)),
            Some(3) => app.crux.send(ShellEvent::Block(BlockEvent::New { start: None, minutes: None })),
            _ => return false,
        }
        true
    }

    fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom == self.rows.tree.hwnd && header.code == NM_DBLCLK {
            app.defer(|app| {
                if let Some(day) = app.day() {
                    day.rows.open(app);
                }
            });
            return Some(1);
        }
        self.rows.notify(app, header, lparam)
    }

    fn context_menu(&self, app: &App, control: HWND, point: Option<POINT>) -> bool {
        self.rows.context_menu(app, control, point, |key, _, action| actions::act_on_row(app, key, action))
    }

    fn enter(&self, app: &App, focus: HWND) -> bool {
        if focus == self.rows.tree.hwnd {
            self.rows.open(app);
            return true;
        }
        false
    }

    fn took_fold(&self) -> bool {
        self.rows.took_fold()
    }

    fn windows(&self) -> Vec<HWND> {
        let mut windows = self.buttons.to_vec();
        windows.push(self.rows.tree.hwnd);
        windows
    }
}
