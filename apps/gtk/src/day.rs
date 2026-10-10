//! The planner: a day as it is lived, as a tree.
//!
//! The first row is the summary — what a glance at a timeline gives a sighted user. Then
//! blocks in time order with their sittings beneath them, free time as rows of its own, and
//! now as a position rather than a highlight. Opening the day puts focus on now, not on
//! midnight.
//!
//! A row's actions are the core's (`actions`), and the keys mean what they mean on every row:
//! on a sitting, Space starts, pauses or resumes its timer and Delete takes it out of the
//! block; on a block, Enter changes it and Delete deletes it; on free time, Enter adds a block
//! there. Everything else is in the row's context menu.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_desktop::speech::{self, Clock};
use lumenna_surface::actions::Action;
use lumenna_surface::{CancelledBlock, Plan, PlanAssignment, PlanBlock, PlanItem};

use crate::core::sentence;
use crate::tree::{Item, Tree};
use crate::window::{App, spawn};
use crate::actions;

/// One row of the day.
#[derive(Clone)]
enum Row {
    Summary(String),
    Block(PlanBlock),
    Sitting(PlanAssignment),
    Free { start: String, end: String, title: String, details: Vec<String>, actions: Vec<Action> },
    Now { title: String, time: String },
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
            Self::Now { .. } => "now".to_owned(),
            Self::Cancelled(block) => format!("cancelled:{}", block.series),
        }
    }

    fn text(&self, clock: &dyn Clock) -> String {
        match self {
            Self::Summary(text) => text.clone(),
            Self::Block(block) => speech::block(block, clock),
            Self::Sitting(sitting) => speech::sitting(sitting),
            Self::Free { start, end, title, details, .. } => speech::free_time(title, details, start, end, clock),
            Self::Now { title, time } => format!("{title}, {}", clock.time(time)),
            Self::Cancelled(block) => speech::cancelled(block, clock),
        }
    }

    /// What can be done to it: the core's, on the record it shows.
    fn actions(&self) -> &[Action] {
        match self {
            Self::Block(block) => &block.actions,
            Self::Sitting(sitting) => &sitting.actions,
            Self::Free { actions, .. } => actions,
            Self::Cancelled(block) => &block.actions,
            Self::Summary(_) | Self::Now { .. } => &[],
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
                app.detail.follow(&app, day.selected_task().as_deref());
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_activate(move |index| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let row = day.rows.borrow().get(index).cloned();
            if let Some(action) = row.as_ref().and_then(|row| actions::find(row.actions(), actions::ENTER)) {
                actions::run(&app, action.clone(), None);
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
            let kinds = match key {
                gdk::Key::space | gdk::Key::KP_Space => actions::SPACE,
                gdk::Key::Delete | gdk::Key::KP_Delete => actions::DELETE,
                _ => return glib::Propagation::Proceed,
            };
            match actions::find(row.actions(), kinds) {
                Some(action) => {
                    actions::run(&app, action.clone(), None);
                    glib::Propagation::Stop
                }
                None => glib::Propagation::Proceed,
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_menu(move |index, point| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = day.rows.borrow().get(index).cloned() else { return };
            if row.actions().is_empty() {
                return;
            }
            let (menu, group) = actions::menu(row.actions(), None);
            app.popup(&menu, day.tree.view.upcast_ref(), point, Some(&group));
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

    /// What can be done to the selected row.
    pub fn selected_actions(&self) -> Vec<Action> {
        self.selected().map(|row| row.actions().to_vec()).unwrap_or_default()
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
                PlanItem::Free { start, end, title, details, actions, .. } => {
                    rows.push(Row::Free {
                        start: start.clone(),
                        end: end.clone(),
                        title: title.clone(),
                        details: details.clone(),
                        actions: actions.clone(),
                    });
                }
                PlanItem::Now { time, title } => rows.push(Row::Now { title: title.clone(), time: time.clone() }),
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

    /// Reads the store again; the first time, lands on now. Once, so coming back to it
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

    /// Go to now: today, on the now row or the block happening now.
    pub fn go_to_now(&self, app: &App, announce: bool) {
        *self.day.borrow_mut() = None;
        self.list(app);
        let index = self.rows.borrow().iter().position(|row| match row {
            Row::Now { .. } => true,
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
            let window: gtk::Window = app.window.clone().upcast();
            let Some(phrase) = actions::ask_text(&window, &lumenna_surface::go_to_day_question(), None).await else {
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

    /// A new block on the day shown, from the menu or the Add Block button.
    pub fn add_block(&self, app: &Rc<App>) {
        let date = self.date().unwrap_or_else(|| "today".to_owned());
        let app = Rc::clone(app);
        spawn(async move { actions::add_block(&app, &date, None, 60).await });
    }

    /// Lands on a block of `series` shown on the day, as after adding one.
    pub fn land_on_block(&self, series: &str) {
        let key = self.rows.borrow().iter().find_map(|row| match row {
            Row::Block(block) if block.series == series => Some(row.key()),
            _ => None,
        });
        self.tree.focus();
        self.tree.select_key_or_near(key.as_deref(), None);
    }
}
