//! Every block series, drawn from the app's view of Blocks (`view.list`), with Add Block… above
//! them: Enter changes one, Delete runs its Delete, and the context menu offers what the core
//! gives each row, run as the app's (`rows.rs`), its form the app's block form.

use std::rc::Rc;

use lumenna_app::{BlockEvent, ViewModel};
use lumenna_app::lines::Lines;
use lumenna_ffi::ShellEvent;
use lumenna_records::RowView;
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, WC_BUTTONW, WC_STATICW};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::WindowsAndMessaging::{BN_CLICKED, BS_PUSHBUTTON, WS_TABSTOP};

use super::app::App;
use super::controls::{self, rect};
use super::rows::{Rows, Shown};
use super::tree::Item;
use super::view::{Metrics, View};
use super::actions;
use super::menu;

const TREE: u16 = 600;
const ADD: u16 = 601;

pub struct BlockList {
    add: HWND,
    count: HWND,
    pub rows: Rows<RowView>,
}

impl BlockList {
    pub fn create(_app: &App, pane: HWND) -> Rc<Self> {
        // The count first: an empty static made after the button would be named after it.
        let count = controls::create(pane, WC_STATICW, "", SS_NOPREFIX.0, 0, 0);
        // The day's own button, as the day has it: a new block, which the app lands on here.
        let add = controls::create(pane, WC_BUTTONW, &menu::button("new-block"), BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, ADD);
        let rows = Rows::create(pane, TREE, "Blocks", false);
        Rc::new(Self { add, count, rows })
    }

    /// Draws the app's Blocks: `landed` when the app moved the row in hand itself.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, landed: bool) {
        let Some(list) = &view.list else { return };
        controls::set_text(self.count, &lines.readback);
        let shown = list
            .items
            .iter()
            .zip(&lines.list)
            .map(|(item, line)| Shown {
                item: Item {
                    key: item.row.id.clone(),
                    text: line.text.clone(),
                    depth: item.row.depth,
                    checked: None,
                    expanded: item.row.expanded,
                    swatch: item.row.swatch.clone(),
                },
                row: item.row.clone(),
                offered: actions::of_row(&list.offered, &item.actions),
            })
            .collect();
        self.rows.draw(app, shown, view.focus.as_deref(), landed);
    }

}

impl View for BlockList {
    fn focus_target(&self) -> HWND {
        self.rows.tree.hwnd
    }

    fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        controls::place(self.count, rect(gap, gap, width - 2 * gap, m.line));
        let mut y = gap + m.line + gap;
        controls::place(self.add, rect(gap, y, m.px(96), m.button));
        y += m.button + gap;
        controls::place(self.rows.tree.hwnd, rect(gap, y, width - 2 * gap, (height - y - gap).max(m.line)));
    }

    fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if control == self.add && u32::from(code) == BN_CLICKED {
            app.crux.send(ShellEvent::Block(BlockEvent::New { start: None, minutes: None }));
            return true;
        }
        false
    }

    fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom == self.rows.tree.hwnd && header.code == NM_DBLCLK {
            app.defer(|app| {
                if let Some(blocks) = app.blocks() {
                    blocks.rows.open(app);
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
        vec![self.count, self.add, self.rows.tree.hwnd]
    }
}
