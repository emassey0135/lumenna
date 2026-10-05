//! The planner (§16.1; §13's worked example): a day as it is lived, as a tree.
//!
//! The first row is the summary — what a glance at a timeline gives a sighted user. Then
//! blocks in time order with their sittings beneath them, free time as rows of its own, and
//! now as a position rather than a highlight. Opening the day puts focus on now, not on
//! midnight.
//!
//! On a sitting, Space starts, pauses or resumes its timer and Delete takes it out of the
//! block; on a block, Enter changes it and Delete deletes it; on free time, Enter adds a block
//! there. Everything else is in the row's context menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_desktop::speech::{self, Clock};
use lumenna_surface::{CancelledBlock, Change, Lumenna, Plan, PlanAssignment, PlanBlock, PlanItem, Result, RowView};

use crate::block_form::{self, Purpose};
use crate::core::sentence;
use crate::tree::{Item, Tree};
use crate::window::{App, spawn};
use crate::{prompts, task_actions};

/// One row of the day.
#[derive(Clone)]
enum Row {
    Summary(String),
    Block(PlanBlock),
    Sitting(PlanAssignment),
    Free { start: String, end: String, minutes: u32 },
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

    fn text(&self, clock: &dyn Clock) -> String {
        match self {
            Self::Summary(text) => text.clone(),
            Self::Block(block) => speech::block(block, clock),
            Self::Sitting(sitting) => speech::sitting(sitting),
            Self::Free { start, end, minutes } => speech::free(start, end, *minutes, clock),
            Self::Now(time) => speech::now(time, clock),
            Self::Cancelled(block) => speech::cancelled(block, clock),
        }
    }
}

/// What can be done to a row of the day.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    Assign,
    Edit,
    CancelDay,
    RestoreDay,
    Delete,
    /// Starts the timer, pauses it or resumes it: what Space does.
    Timer,
    Stop,
    Planned,
    Log,
    Unassign,
    AddHere,
    Open,
}

impl Command {
    fn name(self) -> &'static str {
        match self {
            Self::Assign => "assign",
            Self::Edit => "edit",
            Self::CancelDay => "cancel-day",
            Self::RestoreDay => "restore-day",
            Self::Delete => "delete",
            Self::Timer => "timer",
            Self::Stop => "stop",
            Self::Planned => "planned",
            Self::Log => "log",
            Self::Unassign => "unassign",
            Self::AddHere => "add-here",
            Self::Open => "open",
        }
    }
}

pub struct DayView {
    pub widget: gtk::Box,
    /// The day shown, as an ISO date; `None` follows today.
    day: RefCell<Option<String>>,
    plan: RefCell<Option<Plan>>,
    pub tree: Rc<Tree>,
    rows: RefCell<Vec<Row>>,
    /// Whether the day has opened on now yet.
    landed: Cell<bool>,
}

impl DayView {
    pub fn new() -> Rc<Self> {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        let buttons = gtk::Box::builder().spacing(6).build();
        let button = |label: &str, action: &str| {
            let button = gtk::Button::builder().label(label).action_name(action).build();
            buttons.append(&button);
        };
        button("Previous Day", "win.previous-day");
        button("Today", "win.go-to-now");
        button("Next Day", "win.next-day");
        button("Add Block…", "win.new-block");
        let tree = Tree::new("The day");
        widget.append(&buttons);
        widget.append(&tree.widget);
        let day = Rc::new(Self {
            widget,
            day: RefCell::new(None),
            plan: RefCell::new(None),
            tree,
            rows: RefCell::new(Vec::new()),
            landed: Cell::new(false),
        });
        day.connect();
        day
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.tree.connect_selected(move |_| {
            if let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) {
                app.detail.show(&app, day.selected_task().as_deref());
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_activate(move |_| {
            if let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) {
                day.activate(&app);
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_key(move |key, modifiers, index| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else {
                return glib::Propagation::Proceed;
            };
            if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let Some(row) = day.rows.borrow().get(index).cloned() else { return glib::Propagation::Proceed };
            let command = match (key, &row) {
                (gdk::Key::space | gdk::Key::KP_Space, Row::Sitting(_)) => Command::Timer,
                (gdk::Key::Delete | gdk::Key::KP_Delete, Row::Sitting(_)) => Command::Unassign,
                (gdk::Key::Delete | gdk::Key::KP_Delete, Row::Block(_)) => Command::Delete,
                _ => return glib::Propagation::Proceed,
            };
            day.act(&app, command, row);
            glib::Propagation::Stop
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_menu(move |index, point| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = day.rows.borrow().get(index).cloned() else { return };
            let items: Vec<(Command, &str)> = match &row {
                Row::Block(block) => {
                    let mut items = Vec::new();
                    if block.accepts_tasks {
                        items.push((Command::Assign, "_Assign a Task…"));
                    }
                    items.push((Command::Edit, "_Change…"));
                    if block.repeats {
                        items.push((Command::CancelDay, "C_ancel This Day"));
                    }
                    if block.changed_for_this_day {
                        items.push((Command::RestoreDay, "_Restore This Day"));
                    }
                    items.push((Command::Delete, "_Delete Block…"));
                    items
                }
                Row::Sitting(sitting) => {
                    let mut items = vec![(Command::Timer, timer_label(sitting))];
                    if sitting.running || sitting.status == "paused" {
                        items.push((Command::Stop, "S_top Timer"));
                    }
                    items.extend([
                    (Command::Open, "_Edit Task Details"),
                    (Command::Planned, "_Planned Length…"),
                    (Command::Log, "_Log Minutes…"),
                    (Command::Unassign, "_Unassign"),
                    ]);
                    items
                }
                Row::Free { .. } => vec![(Command::AddHere, "_Add Block Here…")],
                Row::Cancelled(_) => vec![(Command::RestoreDay, "_Restore This Day")],
                Row::Summary(_) | Row::Now(_) => return,
            };
            let menu = gio::Menu::new();
            let actions = gio::SimpleActionGroup::new();
            for (command, label) in items {
                menu.append(Some(label), Some(&format!("row.{}", command.name())));
                let action = gio::SimpleAction::new(command.name(), None);
                let weak = Rc::downgrade(&day);
                let row = row.clone();
                action.connect_activate(move |_, _| {
                    if let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) {
                        day.act(&app, command, row.clone());
                    }
                });
                actions.add_action(&action);
            }
            app.popup(&menu, day.tree.view.upcast_ref(), point, Some(&actions));
        });
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
                self.show(app, vec![Row::Summary(sentence(&error))], key, near);
                return;
            }
        };
        let mut rows = vec![Row::Summary(speech::summary(&plan.date, &plan.summary, &app.clock))];
        for item in &plan.timeline {
            match item {
                PlanItem::Block { row } => {
                    if let Some(block) = plan.blocks.iter().find(|b| b.row == *row) {
                        rows.push(Row::Block(block.clone()));
                        rows.extend(block.assignments.iter().cloned().map(Row::Sitting));
                    }
                }
                PlanItem::Free { start, end, minutes } => {
                    rows.push(Row::Free { start: start.clone(), end: end.clone(), minutes: *minutes });
                }
                PlanItem::Now { time } => rows.push(Row::Now(time.clone())),
            }
        }
        rows.extend(plan.cancelled.iter().cloned().map(Row::Cancelled));
        app.set_title(&app.clock.day(&plan.date));
        *self.plan.borrow_mut() = Some(plan);
        self.show(app, rows, key, near);
    }

    fn show(&self, app: &App, rows: Vec<Row>, key: Option<String>, near: Option<usize>) {
        let items = rows
            .iter()
            .map(|row| Item { key: row.key(), text: row.text(&app.clock), depth: u32::from(matches!(row, Row::Sitting(_))) })
            .collect();
        *self.rows.borrow_mut() = rows;
        if self.tree.set(items) {
            self.tree.select_key_or_near(key.as_deref(), near.or(Some(0)));
        }
    }

    /// Reads the store again; the first time, lands on now (§13). Once, so coming back to it
    /// later does not move the selection from where the person left it.
    pub fn reload(&self, app: &App) {
        self.list(app);
        if !self.landed.replace(true) {
            self.go_to_now(app, false);
        }
    }

    /// A minute has passed: today's day moves on with the clock.
    pub fn minute(&self, app: &App) {
        if self.day.borrow().is_none() {
            self.list(app);
        }
    }

    /// §13's "go to now": today, on the now row or the block happening now.
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
        let summary = self.rows.borrow().first().map(|row| row.text(&app.clock));
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

    pub fn ask_for_day(self: &Rc<Self>, app: &Rc<App>) {
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let Some(phrase) = prompts::ask(&app.window, "Go to Day", "_Day:", "A date, such as friday, or 12 october.", "").await else {
                return;
            };
            match app.core.lumenna.plan(Some(phrase)) {
                Ok(plan) => {
                    if let Ok(date) = plan.date.parse() {
                        day.go_to(&app, date);
                    }
                }
                Err(error) => app.fail(&sentence(&error)),
            }
        });
    }

    pub fn add_block(self: &Rc<Self>, app: &Rc<App>, start: Option<&str>, minutes: Option<u32>) {
        let date = self.date().unwrap_or_else(|| "today".to_owned());
        let fields = block_form::new_fields(start.unwrap_or("09:00"), minutes.unwrap_or(60).min(720));
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let lumenna = app.core.lumenna.clone();
            let window = app.window.clone().upcast::<gtk::Window>();
            let Some(change) = block_form::run(&window, lumenna, Purpose::Add { date }, fields, None).await else { return };
            app.store_changed();
            if let Some(series) = change.affected.blocks.first() {
                let key = day.rows.borrow().iter().find_map(|row| match row {
                    Row::Block(block) if &block.series == series => Some(row.key()),
                    _ => None,
                });
                day.tree.focus();
                day.tree.select_key_or_near(key.as_deref(), None);
            }
            app.say_change(&change);
        });
    }

    /// Changes a block — asking "this day, or every day?" of a repeating one, never guessing
    /// (§13, §4.3).
    fn edit(self: &Rc<Self>, app: &Rc<App>, block: PlanBlock) {
        let Some(date) = self.date() else { return };
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            // One day starts from that day; every occurrence, from the series.
            let mut fields = lumenna_surface::day_block_fields(block.clone());
            let mut rule = None;
            let purpose = if block.repeats {
                let on = format!("{} Only", app.clock.day(&date));
                let heading = format!("Change {}", block.title);
                match prompts::choose(&app.window, &heading, "Which occurrences?", &[&on, "Every Occurrence"]).await {
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
                        fields = lumenna_surface::block_fields(shown);
                    }
                    Err(error) => return app.fail(&sentence(&error)),
                }
            }
            let key = format!("block:{}", block.id);
            let window = app.window.clone().upcast::<gtk::Window>();
            if let Some(change) = block_form::run(&window, app.core.lumenna.clone(), purpose, fields, rule).await {
                app.store_changed();
                day.tree.select_key_or_near(Some(&key), day.tree.selected());
                app.say_change(&change);
            }
        });
    }

    fn delete(self: &Rc<Self>, app: &Rc<App>, block: PlanBlock) {
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let message = if block.repeats {
                "Every occurrence goes, not only this day. To skip one day, cancel it instead."
            } else {
                "It goes, with what is assigned to it."
            };
            if prompts::confirm(&app.window, &format!("Delete {}?", block.title), message, "Delete").await {
                day.change(&app, None, |lumenna| lumenna.delete_block(&block.series));
            }
        });
    }

    /// Runs a change, keeps the selection on `keep` or near where it was, and says it.
    fn change(&self, app: &App, keep: Option<&str>, operation: impl FnOnce(&Lumenna) -> Result<Change>) {
        let near = self.tree.selected();
        if let Some(change) = app.perform(operation) {
            self.tree.select_key_or_near(keep, near);
            app.say_change(&change);
        }
    }

    /// Space on a sitting: starts its timer, pauses it while it runs, resumes it while paused
    /// (§3.7). Stopping, which ends the sitting, is in its menu.
    fn toggle_timer(&self, app: &App, sitting: &PlanAssignment) {
        let key = format!("sitting:{}", sitting.id);
        if sitting.running {
            self.timer(app, &key, app.core.lumenna.pause_timer(&sitting.id));
        } else {
            // Starting a paused sitting resumes it.
            self.change(app, Some(&key), |lumenna| lumenna.start_timer(&sitting.id));
        }
    }

    fn stop_timer(&self, app: &App, sitting: &PlanAssignment) {
        let key = format!("sitting:{}", sitting.id);
        self.timer(app, &key, app.core.lumenna.stop_timer(&sitting.id, None));
    }

    /// Says what pausing or stopping a timer did, from the sitting it was on.
    fn timer(&self, app: &App, key: &str, result: lumenna_surface::Result<lumenna_surface::Timer>) {
        match result {
            Ok(timer) => {
                app.store_changed();
                self.tree.select_key_or_near(Some(key), None);
                app.say(&speech::announcement(&timer.announcement, &timer.notices));
            }
            Err(error) => app.fail(&sentence(&error)),
        }
    }

    fn plan_length(self: &Rc<Self>, app: &Rc<App>, sitting: PlanAssignment) {
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let current = sitting.planned_mins.map(|m| m.to_string()).unwrap_or_default();
            let title = format!("Planned Length of {}", sitting.title);
            let Some(minutes) = task_actions::ask_minutes(&app, &title, &current, true).await else { return };
            let key = format!("sitting:{}", sitting.id);
            day.change(&app, Some(&key), |lumenna| lumenna.plan_minutes(&sitting.id, minutes));
        });
    }

    /// Records a sitting's whole time by hand — without a timer, or to replace a capped one.
    fn log_minutes(self: &Rc<Self>, app: &Rc<App>, sitting: PlanAssignment) {
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let title = format!("Minutes on {}", sitting.title);
            let Some(Some(minutes)) = task_actions::ask_minutes(&app, &title, "", false).await else { return };
            match app.core.lumenna.stop_timer(&sitting.id, Some(minutes)) {
                Ok(timer) => {
                    app.store_changed();
                    day.tree.select_key_or_near(Some(&format!("sitting:{}", sitting.id)), None);
                    app.say(&speech::announcement(&timer.announcement, &timer.notices));
                }
                Err(error) => app.fail(&sentence(&error)),
            }
        });
    }

    /// Fills a block from the task side's opposite: from the block, pick a task (§13).
    fn assign(self: &Rc<Self>, app: &Rc<App>, block: PlanBlock) {
        let Some(date) = self.date() else { return };
        let (day, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move {
            let tasks: Vec<RowView> = app.core.lumenna.list_tasks("").map(|r| r.rows).unwrap_or_default();
            let titles: Vec<String> = tasks.iter().map(|t| speech::row(t, false)).collect();
            let heading = format!("Assign to {}", block.title);
            let Some(index) = prompts::pick(&app.window, &heading, "_Task:", &titles).await else { return };
            let task = &tasks[index];
            let title = format!("How Long Is {} Meant to Take?", task.title);
            let Some(minutes) = task_actions::ask_minutes(&app, &title, "", true).await else { return };
            let key = format!("block:{}", block.id);
            day.change(&app, Some(&key), |lumenna| lumenna.assign(&task.id, &block.series, Some(date), minutes));
        });
    }

    fn act(self: &Rc<Self>, app: &Rc<App>, command: Command, row: Row) {
        let date = self.date().unwrap_or_default();
        match (command, row) {
            (Command::Assign, Row::Block(block)) => self.assign(app, block),
            (Command::Edit, Row::Block(block)) => self.edit(app, block),
            (Command::CancelDay, row @ Row::Block(_)) => {
                let Row::Block(block) = &row else { return };
                self.change(app, Some(&row.key()), |lumenna| lumenna.cancel_occurrence(&block.series, &date));
            }
            (Command::RestoreDay, row @ Row::Block(_)) => {
                let Row::Block(block) = &row else { return };
                self.change(app, Some(&row.key()), |lumenna| lumenna.restore_occurrence(&block.series, &date));
            }
            (Command::RestoreDay, Row::Cancelled(block)) => {
                self.change(app, None, |lumenna| lumenna.restore_occurrence(&block.series, &date));
            }
            (Command::Delete, Row::Block(block)) => self.delete(app, block),
            (Command::Timer, Row::Sitting(sitting)) => self.toggle_timer(app, &sitting),
            (Command::Stop, Row::Sitting(sitting)) => self.stop_timer(app, &sitting),
            (Command::Planned, Row::Sitting(sitting)) => self.plan_length(app, sitting),
            (Command::Log, Row::Sitting(sitting)) => self.log_minutes(app, sitting),
            (Command::Unassign, Row::Sitting(sitting)) => self.change(app, None, |lumenna| lumenna.unassign(&sitting.id)),
            (Command::Open, Row::Sitting(_)) => app.open_detail(),
            (Command::AddHere, Row::Free { start, minutes, .. }) => self.add_block(app, Some(&start), Some(minutes)),
            _ => {}
        }
    }

    fn activate(self: &Rc<Self>, app: &Rc<App>) {
        let Some(row) = self.selected() else { return };
        let command = match &row {
            Row::Block(_) => Command::Edit,
            Row::Sitting(_) => Command::Open,
            Row::Free { .. } => Command::AddHere,
            Row::Cancelled(_) => Command::RestoreDay,
            Row::Summary(_) | Row::Now(_) => return,
        };
        self.act(app, command, row);
    }
}

/// What Space does to a sitting's timer, as its menu says it.
fn timer_label(sitting: &PlanAssignment) -> &'static str {
    if sitting.running {
        "_Pause Timer"
    } else if sitting.status == "paused" {
        "_Resume Timer"
    } else {
        "_Start Timer"
    }
}
