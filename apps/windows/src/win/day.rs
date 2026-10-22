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
//! summary row's, and the Day menu's. Each runs as the app's, but for the forms still the
//! window's own: the block form, a sitting's task (the details), Plan the Day and Replan.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_app::lines::Lines;
use lumenna_app::{ActingOn, BlockEvent, DayItem, DayKind, DayMove, ViewModel};
use lumenna_ffi::ShellEvent;
use lumenna_records::{ActionKind, Asks, Offered};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{
    NM_DBLCLK, NMHDR, NMTREEVIEWW, NMTVKEYDOWN, TVIS_EXPANDED, TVN_ITEMEXPANDEDW, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_BUTTONW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_SPACE};
use windows::Win32::UI::WindowsAndMessaging::{BN_CLICKED, BS_PUSHBUTTON, WS_TABSTOP};

use super::app::App;
use super::controls::{self, rect};
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::actions;

const PREVIOUS: u16 = 500;
const TODAY: u16 = 501;
const NEXT: u16 = 502;
const ADD: u16 = 503;
const TREE: u16 = 504;

pub struct DayView {
    buttons: [HWND; 4],
    pub tree: Tree,
    /// The rows as the app last gave them, those folded away included, in the tree's order.
    rows: RefCell<Vec<DayItem>>,
    /// The day shown, ISO.
    date: RefCell<Option<String>>,
    /// A row the tree itself just folded or unfolded, which the tree has already said.
    pub folded_by_tree: Cell<bool>,
}

impl DayView {
    pub fn create(_app: &App, pane: HWND) -> Rc<Self> {
        let button = |text: &str, id: u16| controls::create(pane, WC_BUTTONW, text, BS_PUSHBUTTON as u32 | WS_TABSTOP.0, 0, id);
        let buttons = [
            button("Previous Day", PREVIOUS),
            button("Today", TODAY),
            button("Next Day", NEXT),
            button("Add Block...", ADD),
        ];
        let tree = Tree::create(pane, TREE, "The day", false);
        Rc::new(Self {
            buttons,
            tree,
            rows: RefCell::new(Vec::new()),
            date: RefCell::new(None),
            folded_by_tree: Cell::new(false),
        })
    }

    fn selected(&self) -> Option<DayItem> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// The task a selected sitting is for: what the Task menu acts on from the day.
    pub fn selected_task(&self) -> Option<String> {
        self.selected()?.sitting.map(|sitting| sitting.task)
    }

    /// Draws the app's day: `landed` when the app moved the row in hand itself.
    pub fn draw(&self, app: &App, view: &ViewModel, lines: &Lines, landed: bool) {
        let Some(day) = &view.day else { return };
        app.set_title(&lines.day_title);
        let items = day
            .items
            .iter()
            .zip(&lines.day)
            .map(|(item, line)| Item {
                key: item.key.clone(),
                text: line.text.clone(),
                depth: item.depth,
                checked: None,
                expanded: item.expanded,
                // A block's colour beside it; its sittings beneath go without.
                swatch: item.block.as_ref().filter(|_| item.kind == DayKind::Block).and_then(|block| block.swatch.clone()),
            })
            .collect();
        *self.rows.borrow_mut() = day.items.clone();
        *self.date.borrow_mut() = Some(day.date.clone());
        let rebuilt = self.tree.set(items);
        if landed || rebuilt {
            self.show(view.focus.as_deref());
        }
        if landed && controls::lost(app.main) {
            controls::focus(self.tree.hwnd);
        }
    }

    /// Selects a row, without its being taken for the person's move.
    fn show(&self, key: Option<&str>) {
        if let Some(index) = key.and_then(|key| self.tree.index_of(key)) {
            self.tree.select_quietly(index);
        }
    }

    /// Runs one of a row's actions as the app's: a block's form, a new block in free time and
    /// the day's two proposals too, the app asking first which occurrences of a repeating
    /// block. The forms still the window's own are a sitting's task (the details) and a row's
    /// reminders.
    fn act(&self, app: &App, row: &DayItem, action: &Offered) {
        match (action.asks, row.kind, action.kind) {
            (Asks::Form, DayKind::Sitting, ActionKind::EditTask) => app.open_detail(),
            _ => app.crux.send(ShellEvent::ActOn(ActingOn::Row(row.key.clone()), action.clone())),
        }
    }

    /// The day's own action of `kind`, from the Day menu: run by the app, or why the day shown
    /// has none, whatever row is in hand.
    pub fn act_on_day(&self, app: &App, kind: ActionKind) {
        app.crux.send(ShellEvent::RunDay(vec![kind]));
    }

    /// The selected row's action of one of `kinds`, if it has one: what a key means here.
    fn act_on_selected(&self, app: &App, kinds: &[ActionKind]) {
        let Some(row) = self.selected() else { return };
        if let Some(action) = actions::of_kind(&row.offered, kinds) {
            self.act(app, &row, &action);
        }
    }

    /// Enter or a double-click: the row's form, or a cancelled day's restoring.
    fn activate(&self, app: &App) {
        let kinds = [ActionKind::Edit, ActionKind::EditTask, ActionKind::AddBlock, ActionKind::RestoreDay];
        self.act_on_selected(app, &kinds);
    }
}

impl View for DayView {
    fn focus_target(&self) -> HWND {
        self.tree.hwnd
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
        controls::place(self.tree.hwnd, rect(gap, y, width - 2 * gap, (height - y - gap).max(m.line)));
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
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW if !self.tree.busy() => {
                if let Some(row) = self.selected() {
                    app.crux.send(ShellEvent::Focus(row.key));
                }
                Some(0)
            }
            TVN_ITEMEXPANDEDW if !self.tree.busy() => {
                // Folding is the app's: the tree's own Left, Right or click is a block's Expand
                // or Collapse.
                let notice = unsafe { &*(lparam.0 as *const NMTREEVIEWW) };
                let kind = if notice.itemNew.state.0 & TVIS_EXPANDED.0 != 0 { ActionKind::Expand } else { ActionKind::Collapse };
                let row = self.tree.index_of_handle(notice.itemNew.hItem).and_then(|i| self.rows.borrow().get(i).cloned());
                if let Some(row) = row
                    && let Some(action) = actions::of_kind(&row.offered, &[kind])
                {
                    self.folded_by_tree.set(true);
                    actions::fold(app, row.key, action, self.selected().map(|selected| selected.key));
                }
                Some(0)
            }
            TVN_KEYDOWN => {
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                // Space is a sitting's timer, and does nothing elsewhere; Delete removes what
                // the row is, or the core says why not.
                if key == VK_SPACE.0 {
                    let kinds = [ActionKind::StartTimer, ActionKind::PauseTimer, ActionKind::ResumeTimer];
                    if self.selected().is_some_and(|row| actions::of_kind(&row.offered, &kinds).is_some()) {
                        app.crux.send(ShellEvent::Run(kinds.to_vec()));
                        return Some(1);
                    }
                    return Some(0);
                }
                if key == VK_DELETE.0 {
                    app.crux.send(ShellEvent::Run(vec![ActionKind::Delete, ActionKind::Unassign]));
                    // Not part of an incremental search.
                    return Some(1);
                }
                Some(0)
            }
            NM_DBLCLK => {
                app.defer(|app| {
                    if let Some(day) = app.day() {
                        day.activate(app);
                    }
                });
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
        let items = actions::menu(&row.offered);
        if items.is_empty() {
            return true;
        }
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(&row.offered, command)) {
            self.act(app, &row, &action);
        }
        true
    }

    fn properties(&self, app: &App) -> bool {
        let Some(row) = self.selected() else { return false };
        let Some(action) = actions::of_kind(&row.offered, &[ActionKind::Edit, ActionKind::EditTask]) else { return false };
        self.act(app, &row, &action);
        true
    }

    fn enter(&self, app: &App, focus: HWND) -> bool {
        if focus == self.tree.hwnd {
            self.activate(app);
            return true;
        }
        false
    }

    fn windows(&self) -> Vec<HWND> {
        let mut windows = self.buttons.to_vec();
        windows.push(self.tree.hwnd);
        windows
    }
}
