//! The planner: a day as it is lived, as a tree. The app's day (`ViewModel::day`), drawn.
//!
//! The first row is the summary — what a glance at a timeline gives a sighted user. Then
//! blocks in time order with their sittings beneath them, free time as rows of its own, and
//! now as a position rather than a highlight. Which day is shown, and that today opens on now,
//! are the app's.
//!
//! A row's actions are the core's, and the keys mean what they mean on every row: on a
//! sitting, Space starts, pauses or resumes its timer and Delete takes it out of the block; on a
//! block, Enter changes it and Delete deletes it; on free time, Enter adds a block there. Left
//! and Right fold a block's sittings. Everything else is in the row's context menu.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::{DayItem, DayKind};
use lumenna_ffi::ShellEvent;
use lumenna_records::{Action, ActionKind};

use crate::actions;
use crate::tree::{Item, Landing, Tree};
use crate::window::{App, spawn};

pub struct DayView {
    pub widget: gtk::Box,
    pub tree: Rc<Tree>,
    /// The rows as last drawn.
    rows: RefCell<Vec<DayItem>>,
    /// The day shown, ISO, as last drawn.
    date: RefCell<Option<String>>,
    /// When focus goes to the row in hand.
    landing: Landing,
    /// A block added here, by series, to land on once the day shows it.
    landing_on: RefCell<Option<String>>,
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
            tree,
            rows: RefCell::new(Vec::new()),
            date: RefCell::new(None),
            landing: Landing::default(),
            landing_on: RefCell::new(None),
        });
        day.connect();
        day
    }

    fn connect(self: &Rc<Self>) {
        // The person's moves are the app's to know, so the row in hand follows them.
        let weak = Rc::downgrade(self);
        self.tree.connect_selected(move |index| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(key) = index.and_then(|index| day.tree.key(index)) else { return };
            if app.model.view().focus.as_deref() != Some(key.as_str()) {
                app.model.send(ShellEvent::Focus(key));
            }
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_activate(move |index| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = day.rows.borrow().get(index).cloned() else { return };
            crate::asking::run_kind(&app, &row.actions, actions::ENTER, &row.key);
        });
        self.tree.connect_key(move |key, modifiers, _| {
            let Some(app) = crate::window::app() else { return glib::Propagation::Proceed };
            if modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK) {
                return glib::Propagation::Proceed;
            }
            let kinds = match key {
                gdk::Key::space | gdk::Key::KP_Space => actions::SPACE,
                gdk::Key::Delete | gdk::Key::KP_Delete => actions::DELETE,
                _ => return glib::Propagation::Proceed,
            };
            app.model.send(ShellEvent::Run(kinds.to_vec()));
            glib::Propagation::Stop
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_menu(move |index, point| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = day.rows.borrow().get(index).cloned() else { return };
            // Folding is Left and Right's, as in every tree; the menu holds what the row is for.
            let offered: Vec<Action> =
                row.actions.iter().filter(|a| !matches!(a.kind, ActionKind::Expand | ActionKind::Collapse)).cloned().collect();
            if offered.is_empty() {
                return;
            }
            let key = row.key.clone();
            let (menu, group) = actions::menu_running(
                &offered,
                Rc::new(move |action| {
                    if let Some(app) = crate::window::app() {
                        crate::asking::run(&app, action, &key);
                    }
                }),
            );
            app.popup(&menu, day.tree.view.upcast_ref(), point, Some(&group));
        });
        let weak = Rc::downgrade(self);
        self.tree.connect_fold(move |index, expand| {
            let (Some(day), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(row) = day.rows.borrow().get(index).cloned() else { return };
            let kind = if expand { ActionKind::Expand } else { ActionKind::Collapse };
            if let Some(action) = row.actions.iter().find(|action| action.kind == kind) {
                crate::window::QUIET_FOLD.set(true);
                app.model.run(action.clone(), &row.key);
            }
        });
    }

    /// Draws the day the app last left, if the app shows the day.
    pub fn draw(&self, app: &App) {
        let view = app.model.view();
        let Some(day) = view.day.as_ref().filter(|_| view.ready) else { return };
        let lines = app.model.lines();
        let items = day
            .items
            .iter()
            .enumerate()
            .map(|(at, item)| Item {
                key: item.key.clone(),
                text: lines.day.get(at).map_or_else(|| item.key.clone(), |line| line.text.clone()),
                depth: item.depth,
                expanded: item.expanded,
                ..Default::default()
            })
            .collect();
        *self.rows.borrow_mut() = day.items.clone();
        *self.date.borrow_mut() = Some(day.date.clone());
        app.set_title(&lines.day_title);
        let rebuilt = self.tree.set(items);
        let landing_on = self.landing_on.borrow().clone();
        let block = landing_on.and_then(|series| {
            day.items.iter().find(|item| item.kind == DayKind::Block && item.block.as_ref().is_some_and(|b| b.series == series))
        });
        match block.and_then(|item| self.tree.index_of(&item.key)) {
            Some(index) => {
                self.landing_on.take();
                self.tree.select(index);
                self.tree.focus();
            }
            None => self.landing.land(&self.tree, app.window.upcast_ref(), view.focus.as_deref(), view.landing, rebuilt),
        }
        app.detail.follow(app, self.selected_task().as_deref());
    }

    fn selected(&self) -> Option<DayItem> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).cloned()
    }

    /// What can be done to the selected row.
    pub fn selected_actions(&self) -> Vec<Action> {
        self.selected().map(|row| row.actions).unwrap_or_default()
    }

    /// The task a selected sitting is for: what the Task menu acts on from the day.
    pub fn selected_task(&self) -> Option<String> {
        self.selected()?.sitting.map(|sitting| sitting.task)
    }

    /// Moves focus into the day, once it is drawn.
    pub fn focus(&self) {
        self.landing.want(&self.tree);
    }

    /// A new block on the day shown, from the menu or the Add Block button.
    pub fn add_block(&self, app: &Rc<App>) {
        let date = self.date.borrow().clone().unwrap_or_else(|| "today".to_owned());
        let app = Rc::clone(app);
        spawn(async move { actions::add_block(&app, &date, None, 60).await });
    }

    /// Lands on a block of `series` shown on the day, as after adding one.
    pub fn land_on_block(&self, series: &str) {
        *self.landing_on.borrow_mut() = Some(series.to_owned());
        if let Some(app) = crate::window::app() {
            app.model.tick();
            self.draw(&app);
        }
    }
}
