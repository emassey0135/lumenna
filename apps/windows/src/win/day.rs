//! The planner: a day as it is lived, as a tree.
//!
//! The first row is the summary — what a glance at a timeline gives a sighted user. Then
//! blocks in time order with their sittings beneath them, free time as rows of its own, and
//! now as a position rather than a highlight. Opening the day puts the selection on now, not
//! on midnight.
//!
//! On a sitting, Space starts or stops its timer and Delete takes it out of the block; on a
//! block, Enter changes it and Delete deletes it; on free time, Enter adds a block there.
//! Everything else is in the row's context menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_surface::{Action, ActionKind, CancelledBlock, Plan, PlanAssignment, PlanBlock, PlanItem, block_fields, day_block_fields};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NM_DBLCLK, NMHDR, NMTVKEYDOWN, TVN_KEYDOWN, TVN_SELCHANGEDW, WC_BUTTONW};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DELETE, VK_SPACE};
use windows::Win32::UI::WindowsAndMessaging::{BN_CLICKED, BS_PUSHBUTTON, WS_TABSTOP};

use super::app::App;
use super::block_form::{self, Purpose};
use super::clock::Locale;
use super::controls::{self, rect};
use super::core::sentence;
use super::tree::{Item, Tree};
use super::view::{Metrics, View};
use super::{actions, prompts};
use crate::speech::{self, Clock};

const PREVIOUS: u16 = 500;
const TODAY: u16 = 501;
const NEXT: u16 = 502;
const ADD: u16 = 503;
const TREE: u16 = 504;

/// One row of the day.
#[derive(Clone)]
enum Row {
    Summary(String),
    Block(PlanBlock),
    Sitting(PlanAssignment),
    Free { start: String, minutes: u32, text: String, actions: Vec<Action> },
    Now(String),
    Cancelled(CancelledBlock),
}

impl Row {
    /// What identifies it across a reload, for keeping the selection.
    fn key(&self) -> String {
        match self {
            Self::Summary(_) => "summary".to_owned(),
            Self::Block(block) => format!("block:{}", block.id),
            Self::Sitting(sitting) => format!("sitting:{}", sitting.id),
            Self::Free { start, .. } => format!("free:{start}"),
            Self::Now(_) => "now".to_owned(),
            Self::Cancelled(block) => format!("cancelled:{}", block.series),
        }
    }

    fn text(&self) -> String {
        let clock = Locale;
        match self {
            Self::Summary(text) => text.clone(),
            Self::Block(block) => speech::block(block, &clock),
            Self::Sitting(sitting) => speech::sitting(sitting),
            Self::Free { text, .. } => text.clone(),
            Self::Now(text) => text.clone(),
            Self::Cancelled(block) => speech::cancelled(block, &clock),
        }
    }

    /// What the core says can be done to it.
    fn actions(&self) -> &[Action] {
        match self {
            Self::Block(block) => &block.actions,
            Self::Sitting(sitting) => &sitting.actions,
            Self::Free { actions, .. } => actions,
            Self::Cancelled(block) => &block.actions,
            Self::Summary(_) | Self::Now(_) => &[],
        }
    }
}

pub struct DayView {
    /// The day shown, as an ISO date; `None` follows today.
    day: RefCell<Option<String>>,
    plan: RefCell<Option<Plan>>,
    buttons: [HWND; 4],
    pub tree: Tree,
    rows: RefCell<Vec<Row>>,
    /// Whether the day has opened on now yet.
    landed: Cell<bool>,
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
            day: RefCell::new(None),
            plan: RefCell::new(None),
            buttons,
            tree,
            rows: RefCell::new(Vec::new()),
            landed: Cell::new(false),
        })
    }

    /// The ISO date shown.
    pub fn date(&self) -> Option<String> {
        self.plan.borrow().as_ref().map(|plan| plan.date.clone())
    }

    fn selected(&self) -> Option<Row> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// The task a selected sitting is for: what the Task menu acts on from the day.
    pub fn selected_task(&self) -> Option<String> {
        match self.selected()? {
            Row::Sitting(sitting) => Some(sitting.task),
            _ => None,
        }
    }

    fn list(&self, app: &App) {
        let key = self.tree.selected().and_then(|index| self.tree.key(index));
        let near = self.tree.selected();
        let day = self.day.borrow().clone();
        let plan = match app.core.lumenna.plan(day) {
            Ok(plan) => plan,
            Err(error) => {
                let rows = vec![Row::Summary(sentence(&error))];
                self.show(rows, key, near);
                return;
            }
        };
        let mut rows = vec![Row::Summary(speech::summary(&plan.date, &plan.summary, &Locale))];
        for item in &plan.timeline {
            match item {
                PlanItem::Block { row } => {
                    if let Some(block) = plan.blocks.iter().find(|b| b.row == *row) {
                        rows.push(Row::Block(block.clone()));
                        rows.extend(block.assignments.iter().cloned().map(Row::Sitting));
                    }
                }
                PlanItem::Free { start, end, minutes, title, details, actions } => {
                    let actions = actions.clone();
                    let text = speech::free_time(title, details, start, end, &Locale);
                    rows.push(Row::Free { start: start.clone(), minutes: *minutes, text, actions });
                }
                // Its title, then the time: the order every app says it in.
                PlanItem::Now { time, title } => rows.push(Row::Now(format!("{title}, {}", Locale.time(time)))),
            }
        }
        rows.extend(plan.cancelled.iter().cloned().map(Row::Cancelled));
        app.set_title(&Locale.day(&plan.date));
        *self.plan.borrow_mut() = Some(plan);
        self.show(rows, key, near);
    }

    fn show(&self, rows: Vec<Row>, key: Option<String>, near: Option<usize>) {
        let items = rows
            .iter()
            .map(|row| Item {
                key: row.key(),
                text: row.text(),
                depth: u32::from(matches!(row, Row::Sitting(_))),
                checked: None,
            })
            .collect();
        *self.rows.borrow_mut() = rows;
        if self.tree.set(items) {
            self.tree.select_key_or_near(key.as_deref(), near.or(Some(0)));
        }
    }

    /// "Go to now": today, on the now row or the block happening now.
    pub fn go_to_now(&self, app: &App, announce: bool) {
        *self.day.borrow_mut() = None;
        self.list(app);
        let index = self.rows.borrow().iter().position(|row| match row {
            Row::Now(_) => true,
            Row::Block(block) => block.when == "now",
            _ => false,
        });
        self.tree.select(index.unwrap_or(0));
        if announce {
            self.say_summary(app);
        }
    }

    fn say_summary(&self, app: &App) {
        let summary = self.rows.borrow().first().map(Row::text);
        if let Some(summary) = summary {
            app.say(&summary);
        }
    }

    /// Moves by days, and says the new day's summary: a new day is a new screen's worth.
    pub fn step(&self, app: &App, days: i64) {
        let Some(shown) = self.date().and_then(|d| d.parse::<jiff::civil::Date>().ok()) else { return };
        let Ok(next) = shown.checked_add(jiff::Span::new().days(days)) else { return };
        self.go_to(app, next);
    }

    fn go_to(&self, app: &App, date: jiff::civil::Date) {
        let today = jiff::Zoned::now().date();
        *self.day.borrow_mut() = (date != today).then(|| date.to_string());
        self.list(app);
        self.tree.select(0);
        self.say_summary(app);
    }

    pub fn ask_for_day(&self, app: &App) {
        let Some(phrase) = prompts::ask_text(app.main, "Go to Day", "&Day:", "A date, such as friday, or 12 october.", "", "Go") else {
            return;
        };
        match app.core.lumenna.plan(Some(phrase)) {
            Ok(plan) => {
                if let Ok(date) = plan.date.parse() {
                    self.go_to(app, date);
                }
            }
            Err(error) => prompts::fail(app.main, &sentence(&error)),
        }
    }

    pub fn add_block(&self, app: &App, start: Option<&str>, minutes: Option<u32>) {
        let date = self.date().unwrap_or_else(|| "today".to_owned());
        let fields = block_form::fresh(start.unwrap_or("09:00"), minutes.unwrap_or(60).min(720));
        if let Some(change) = block_form::run(app.main, &app.core.lumenna, Purpose::Add { date }, fields, None) {
            app.store_changed();
            if let Some(series) = change.affected.blocks.first() {
                let key = self.rows.borrow().iter().find_map(|row| match row {
                    Row::Block(block) if &block.series == series => Some(row.key()),
                    _ => None,
                });
                self.tree.select_key_or_near(key.as_deref(), None);
            }
            app.say_change(&change);
        }
    }

    /// Changes a block — asking "this day, or every day?" of a repeating one, never guessing.
    fn edit(&self, app: &App, block: &PlanBlock) {
        let Some(date) = self.date() else { return };
        // One day alone: its time, length, title, kind and flags, as this day has them.
        let mut fields = day_block_fields(block.clone());
        let mut rule = None;
        let purpose = if block.repeats {
            let day = Locale.day(&date);
            let choice = prompts::choose(
                app.main,
                &format!("Change {}", block.title),
                "Which occurrences?",
                &[&format!("{day} Only"), "Every Occurrence"],
                false,
            );
            match choice {
                Some(0) => Purpose::Occurrence { series: block.series.clone(), date },
                Some(_) => Purpose::Series { id: block.series.clone() },
                None => return,
            }
        } else {
            Purpose::Series { id: block.series.clone() }
        };
        if let Purpose::Series { id } = &purpose {
            // The series as it is, not as this day shows it.
            match app.core.lumenna.show_block(id) {
                Ok(shown) => {
                    rule = shown.rrule.clone().filter(|_| shown.repeats);
                    fields = block_fields(shown);
                }
                Err(error) => return prompts::fail(app.main, &sentence(&error)),
            }
        }
        let key = format!("block:{}", block.id);
        if let Some(change) = block_form::run(app.main, &app.core.lumenna, purpose, fields, rule) {
            app.store_changed();
            self.tree.select_key_or_near(Some(&key), self.tree.selected());
            app.say_change(&change);
        }
    }

    /// Runs one of a row's actions and says what it did. The forms are the app's own: a
    /// block's Edit, a sitting's task, a free span's new block. Reloading keeps the selection
    /// on the same row, or near where it was when the row has gone.
    fn act(&self, app: &App, row: &Row, action: &Action) {
        let form = || match (row, action.kind) {
            (Row::Block(block), ActionKind::Edit) => self.edit(app, block),
            (Row::Sitting(_), ActionKind::EditTask) => app.open_detail(),
            (Row::Free { start, minutes, .. }, ActionKind::AddBlock) => self.add_block(app, Some(start), Some(*minutes)),
            _ => {}
        };
        if let Some((change, _)) = actions::run(app, app.main, action, form) {
            app.say_change(&change);
        }
    }

    /// The selected row's action of one of `kinds`, if it has one: what a key means here.
    fn act_on_selected(&self, app: &App, kinds: &[ActionKind]) {
        let Some(row) = self.selected() else { return };
        if let Some(action) = actions::of_kind(row.actions(), kinds) {
            self.act(app, &row, &action);
        }
    }

    /// Enter or a double-click: the row's form, or a cancelled day's restoring.
    fn activate(&self, app: &App) {
        let kinds = [ActionKind::Edit, ActionKind::EditTask, ActionKind::AddBlock, ActionKind::RestoreDay];
        self.act_on_selected(app, &kinds);
    }

    /// The same, after the notification being handled.
    fn act_later(app: &App, kinds: &'static [ActionKind]) {
        app.defer(move |app| {
            if let Some(day) = app.day() {
                day.act_on_selected(app, kinds);
            }
        });
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

    fn reload(&self, app: &App) {
        self.list(app);
        // Opening the day lands on now. Once, so coming back to it later does not move
        // the selection from where the person left it.
        if !self.landed.replace(true) {
            self.go_to_now(app, false);
        }
    }

    fn minute(&self, app: &App) {
        if self.day.borrow().is_none() {
            self.list(app);
        }
    }

    fn command(&self, app: &App, control: HWND, _id: u16, code: u16) -> bool {
        if u32::from(code) != BN_CLICKED {
            return false;
        }
        match self.buttons.iter().position(|b| *b == control) {
            Some(0) => self.step(app, -1),
            Some(1) => self.go_to_now(app, true),
            Some(2) => self.step(app, 1),
            Some(3) => self.add_block(app, None, None),
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
                app.detail.show(app, self.selected_task().as_deref());
                Some(0)
            }
            TVN_KEYDOWN => {
                let key = unsafe { &*(lparam.0 as *const NMTVKEYDOWN) }.wVKey;
                // Space is a sitting's timer; Delete removes what the row is.
                let kinds: &'static [ActionKind] = if key == VK_SPACE.0 {
                    &[ActionKind::StartTimer, ActionKind::PauseTimer, ActionKind::ResumeTimer]
                } else if key == VK_DELETE.0 {
                    &[ActionKind::Delete, ActionKind::Unassign]
                } else {
                    return Some(0);
                };
                let Some(row) = self.selected() else { return Some(0) };
                if actions::of_kind(row.actions(), kinds).is_none() {
                    // Delete on free time or a cancelled day: the core says why not.
                    if key == VK_DELETE.0 {
                        actions::say_not_offered(app, row.actions(), ActionKind::Delete);
                        return Some(1);
                    }
                    return Some(0);
                }
                Self::act_later(app, kinds);
                // Not part of an incremental search.
                Some(1)
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
        let items = actions::menu(row.actions());
        if items.is_empty() {
            return true;
        }
        let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(action) = app.popup(&items, at).and_then(|command| actions::chosen(row.actions(), command)) {
            self.act(app, &row, &action);
        }
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
