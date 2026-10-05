//! Every block series (§16.1: block editor): Enter changes one, Delete deletes it.

use std::cell::RefCell;
use std::rc::Rc;

use lumenna_surface::{RowView, block_fields};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, NMTVKEYDOWN, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_STATICW};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::System::SystemServices::SS_NOPREFIX;

use super::app::App;
use super::block_form::{self, Purpose};
use super::controls::{self, rect};
use super::core::sentence;
use super::prompts;
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use crate::speech;

const TREE: u16 = 600;
const EDIT: u16 = 1;
const DELETE: u16 = 2;

pub struct BlockList {
    count: HWND,
    pub tree: Tree,
    rows: RefCell<Vec<RowView>>,
}

impl BlockList {
    pub fn create(_app: &App, pane: HWND) -> Rc<Self> {
        let count = controls::create(pane, WC_STATICW, "", SS_NOPREFIX.0, 0, 0);
        let tree = Tree::create(pane, TREE, "Blocks", false);
        Rc::new(Self { count, tree, rows: RefCell::new(Vec::new()) })
    }

    fn selected(&self) -> Option<RowView> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    fn edit(&self, app: &App, row: &RowView) {
        let shown = match app.core.lumenna.show_block(&row.id) {
            Ok(shown) => shown,
            Err(error) => return prompts::fail(app.main, &sentence(&error)),
        };
        let rule = shown.rrule.clone().filter(|_| shown.repeats);
        let purpose = Purpose::Series { id: shown.id.clone() };
        if let Some(change) = block_form::run(app.main, &app.core.lumenna, purpose, block_fields(shown), rule) {
            app.store_changed();
            self.tree.select_key_or_near(Some(&row.id), self.tree.selected());
            app.say_change(&change);
        }
    }

    fn delete(&self, app: &App, row: &RowView) {
        if prompts::confirm(
            app.main,
            &format!("Delete {}?", row.title),
            "Every occurrence goes, with what is assigned to it.",
            "Delete",
        ) {
            let near = self.tree.selected();
            if let Some(change) = app.perform(|lumenna| lumenna.delete_block(&row.id)) {
                self.tree.select_key_or_near(None, near);
                app.say_change(&change);
            }
        }
    }

    fn act(&self, app: &App, command: u16) {
        let Some(row) = self.selected() else { return };
        match command {
            EDIT => self.edit(app, &row),
            DELETE => self.delete(app, &row),
            _ => {}
        }
    }

    fn act_later(app: &App, command: u16) {
        app.defer(move |app| {
            if let Some(blocks) = app.blocks() {
                blocks.act(app, command);
            }
        });
    }
}

impl View for BlockList {
    fn focus_target(&self) -> HWND {
        self.tree.hwnd
    }

    fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        controls::place(self.count, rect(gap, gap, width - 2 * gap, m.line));
        let y = gap + m.line + gap;
        controls::place(self.tree.hwnd, rect(gap, y, width - 2 * gap, (height - y - gap).max(m.line)));
    }

    fn reload(&self, app: &App) {
        let key = self.tree.selected().and_then(|index| self.tree.key(index));
        let near = self.tree.selected().or(Some(0));
        match app.core.lumenna.list_blocks() {
            Ok(listing) => {
                controls::set_text(self.count, &speech::sentence(&listing.announcement));
                let items = listing
                    .rows
                    .iter()
                    .map(|row| Item { key: row.id.clone(), text: speech::row(row, false), depth: 0, checked: None })
                    .collect();
                *self.rows.borrow_mut() = listing.rows;
                if self.tree.set(items) {
                    self.tree.select_key_or_near(key.as_deref(), near);
                }
            }
            Err(error) => controls::set_text(self.count, &sentence(&error)),
        }
    }

    fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW => Some(0),
            TVN_KEYDOWN if unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey == VK_DELETE.0 => {
                Self::act_later(app, DELETE);
                Some(1)
            }
            NM_DBLCLK => {
                Self::act_later(app, EDIT);
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
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(command) = app.popup(&[(EDIT, "&Change..."), (0, ""), (DELETE, "&Delete...")], at) {
            self.act(app, command);
        }
        true
    }

    fn enter(&self, app: &App, focus: HWND) -> bool {
        if focus == self.tree.hwnd {
            self.act(app, EDIT);
            return true;
        }
        false
    }

    fn windows(&self) -> Vec<HWND> {
        vec![self.count, self.tree.hwnd]
    }
}
