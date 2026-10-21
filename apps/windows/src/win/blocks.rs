//! Every block series, drawn from the app's view of Blocks (`view.list`): Enter changes one,
//! Delete runs its Delete, and the context menu offers what the core gives each row, run as
//! the app's, its form the app's block form.

use std::cell::RefCell;
use std::rc::Rc;

use lumenna_app::ViewModel;
use lumenna_app::lines::Lines;
use lumenna_ffi::ShellEvent;
use lumenna_records::{Action, ActionKind, Question, RowView};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, NMTVKEYDOWN, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_STATICW};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_DELETE;
use windows::Win32::System::SystemServices::SS_NOPREFIX;

use super::app::App;
use super::controls::{self, rect};
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::actions;

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

    /// Draws the app's Blocks: `landed` when the app moved the row in hand itself.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, landed: bool) {
        let Some(list) = &view.list else { return };
        controls::set_text(self.count, &lines.readback);
        let items = list
            .items
            .iter()
            .zip(&lines.list)
            .map(|(item, line)| Item { key: item.row.id.clone(), text: line.text.clone(), depth: item.row.depth, checked: None, expanded: item.row.expanded, swatch: item.row.swatch.clone() })
            .collect();
        *self.rows.borrow_mut() = list.items.iter().map(|item| item.row.clone()).collect();
        let rebuilt = self.tree.set(items);
        if (landed || rebuilt)
            && let Some(index) = view.focus.as_deref().and_then(|key| self.tree.index_of(key))
        {
            self.tree.select_quietly(index);
        }
        if landed && controls::lost(app.main) {
            controls::focus(self.tree.hwnd);
        }
    }

    /// Runs one of the selected row's actions as the app's; Reminders is the window's list.
    fn act(&self, app: &App, action: &Action) {
        match (&action.question, action.kind) {
            (Question::Form, ActionKind::Reminders) => app.crux.send(ShellEvent::Perform(action.clone())),
            _ => app.crux.send(ShellEvent::Perform(action.clone())),
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


    fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW if !self.tree.busy() => {
                if let Some(row) = self.selected() {
                    app.crux.send(ShellEvent::Focus(row.id));
                }
                Some(0)
            }
            TVN_KEYDOWN if unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey == VK_DELETE.0 => {
                app.crux.send(ShellEvent::Run(vec![ActionKind::Delete]));
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
