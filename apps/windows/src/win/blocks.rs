//! Every block series: Enter changes one, Delete runs its Delete, and the context menu offers
//! what the core gives each row.

use std::cell::RefCell;
use std::rc::Rc;

use lumenna_surface::{Action, ActionKind, RowView, block_fields, unsayable_repeat_note};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, NMTVKEYDOWN, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_STATICW};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::System::SystemServices::SS_NOPREFIX;

use super::app::App;
use super::block_form::{self, Purpose};
use super::clock::Locale;
use super::controls::{self, rect};
use super::core::sentence;
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::{actions, prompts};
use crate::speech;

const TREE: u16 = 600;

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
        let note = unsayable_repeat_note(shown.clone());
        let purpose = Purpose::Series { id: shown.id.clone() };
        if let Some(change) = block_form::run(app.main, &app.core.lumenna, purpose, block_fields(shown), note) {
            app.store_changed();
            self.tree.select_key_or_near(Some(&row.id), self.tree.selected());
            app.say_change(&change);
        }
    }

    /// Runs one of the selected row's actions; Edit Block is the block form.
    fn act(&self, app: &App, action: &Action) {
        let Some(row) = self.selected() else { return };
        if let Some((change, _)) = actions::run(app, app.main, action, || self.edit(app, &row)) {
            app.say_change(&change);
        }
    }

    /// The selected row's action of one of `kinds`, after the notification being handled.
    fn act_later(app: &App, kinds: &'static [ActionKind]) {
        app.defer(move |app| {
            if let Some(blocks) = app.blocks()
                && let Some(action) = blocks.selected().and_then(|row| actions::of_kind(&row.actions, kinds))
            {
                blocks.act(app, &action);
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
                let said = if listing.rows.is_empty() && !listing.empty.is_empty() { &listing.empty } else { &listing.announcement };
                controls::set_text(self.count, &speech::sentence(said));
                let items = listing
                    .rows
                    .iter()
                    .map(|row| Item { key: row.id.clone(), text: speech::row(row, false, &Locale), depth: 0, checked: None })
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
                Self::act_later(app, &[ActionKind::Delete]);
                Some(1)
            }
            NM_DBLCLK => {
                Self::act_later(app, &[ActionKind::Edit]);
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
        let (Some(index), Some(row)) = (self.tree.selected(), self.selected()) else { return true };
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        let items = actions::menu(&row.actions);
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(&row.actions, command)) {
            self.act(app, &action);
        }
        true
    }

    fn properties(&self, app: &App) -> bool {
        let Some(action) = self.selected().and_then(|row| actions::of_kind(&row.actions, &[ActionKind::Edit])) else { return false };
        self.act(app, &action);
        true
    }

    fn enter(&self, app: &App, focus: HWND) -> bool {
        if focus == self.tree.hwnd {
            if let Some(action) = self.selected().and_then(|row| actions::of_kind(&row.actions, &[ActionKind::Edit])) {
                self.act(app, &action);
            }
            return true;
        }
        false
    }

    fn windows(&self) -> Vec<HWND> {
        vec![self.count, self.tree.hwnd]
    }
}
