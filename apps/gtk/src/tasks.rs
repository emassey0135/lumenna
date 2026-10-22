//! Tasks, with the filter above them: the app's list (`ViewModel::list`), drawn.
//!
//! A tree, so a subtask's level and every row's position are the tree's to report. A row's
//! actions are the app's, offered as the core gives them: Space checks a task off (in the
//! trash, restores it), Delete trashes it (in the trash, deletes it from there), Enter opens its
//! details, and the Menu key or Shift+F10 offers the rest; Left and Right fold through the
//! row's Collapse and Expand. Which row is in hand after a change is the app's too: focus
//! moves to it only when the app says it moved (`ViewModel::landing`), never for a redraw.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use lumenna_app::{Item as Row, Offered};
use lumenna_ffi::ShellEvent;
use lumenna_records::places::Place;
use lumenna_records::{ActionKind, RowView};

use crate::actions;
use crate::tree::{Item, Landing, Tree};
use crate::window::App;

pub struct TaskList {
    pub place: Place,
    trash: bool,
    /// Whether it has a filter to change: not the trash, History or the block series.
    filtered: bool,
    pub widget: gtk::Box,
    filter: gtk::Entry,
    readback: gtk::Label,
    pub tree: Rc<Tree>,
    /// The rows as last drawn.
    rows: RefCell<Vec<Row>>,
    /// What the rows offer, each once, in short: each row names its own by place here.
    offered: RefCell<Vec<Offered>>,
    /// When focus goes to the row in hand.
    landing: Landing,
    /// Whether the filter's text is being put back from the app, not typed.
    setting: Cell<bool>,
}

impl TaskList {
    pub fn new(place: Place) -> Rc<Self> {
        let trash = place == Place::Trash;
        // The block series have no filter either, and a button to add one.
        let blocks = place == Place::Blocks;
        let filtered = !matches!(place, Place::Trash | Place::History | Place::Blocks);
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        let filter = gtk::Entry::builder().text(place.query()).build();
        let label = gtk::Label::builder().label("Filte_r").use_underline(true).xalign(0.0).build();
        label.set_mnemonic_widget(Some(&filter));
        filter.update_property(&[gtk::accessible::Property::Description(
            "Such as p1 & due before: friday, or #Work. Down arrow offers what could come next. Enter goes to what it finds.",
        )]);
        crate::completion::attach(&filter, crate::completion::Source::Filter);
        let readback = gtk::Label::builder().xalign(0.0).wrap(true).build();
        let tree = Tree::new(&place.title());
        // The trash is everything deleted and History everything done lately, nothing else:
        // there is no filter to change.
        if filtered {
            widget.append(&label);
            widget.append(&filter);
        }
        widget.append(&readback);
        if blocks {
            let add = gtk::Button::builder().label("Add Block…").action_name("win.new-block").halign(gtk::Align::Start).build();
            widget.append(&add);
        }
        widget.append(&tree.widget);
        let list = Rc::new(Self {
            place,
            trash,
            filtered,
            widget,
            filter,
            readback,
            tree,
            rows: RefCell::new(Vec::new()),
            offered: RefCell::new(Vec::new()),
            landing: Landing::default(),
            setting: Cell::new(false),
        });
        list.connect();
        list
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.filter.connect_changed(move |filter| {
            // Typing is silent; the rows follow it, and Enter says what was found.
            if let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app())
                && !list.setting.get()
            {
                app.model.send(ShellEvent::Filter(filter.text().to_string()));
            }
        });
        let weak = Rc::downgrade(self);
        self.filter.connect_activate(move |_| {
            // Finishing the filter says what it found, then goes to it.
            if let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) {
                app.say(&list.readback.label());
                list.tree.focus();
            }
        });
        let escape = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape
                && let Some(list) = weak.upgrade() {
                    list.tree.focus();
                    return glib::Propagation::Stop;
                }
            glib::Propagation::Proceed
        });
        self.filter.add_controller(escape);

        // The person's moves are the app's to know, so the row in hand follows them.
        let weak = Rc::downgrade(self);
        self.tree.connect_selected(move |index| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(key) = index.and_then(|index| list.tree.key(index)) else { return };
            if app.model.view().focus.as_deref() != Some(key.as_str()) {
                app.model.send(ShellEvent::Focus(key));
            }
        });
        self.tree.connect_activate(move |_| {
            if let Some(app) = crate::window::app() {
                app.model.send(ShellEvent::Run(actions::ENTER.to_vec()));
            }
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
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some((row, offered)) = list.offered(index) else { return };
            let (menu, group) = actions::menu_offering(
                &offered,
                Rc::new(move |offered| {
                    if let Some(app) = crate::window::app() {
                        crate::asking::act(&app, offered, &row);
                    }
                }),
            );
            app.popup(&menu, list.tree.view.upcast_ref(), point, Some(&group));
        });
        // Folding is the app's: Left and Right run the row's Collapse or Expand.
        let weak = Rc::downgrade(self);
        self.tree.connect_fold(move |index, expand| {
            let (Some(list), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some((row, offered)) = list.offered(index) else { return };
            let kind = if expand { ActionKind::Expand } else { ActionKind::Collapse };
            if let Some(action) = offered.into_iter().find(|action| action.kind == kind) {
                // The tree reports the row's new state itself, so the app's word for it would
                // say it twice.
                crate::window::QUIET_FOLD.set(true);
                app.model.act(action, &row.id);
            }
        });
    }

    /// The row at `index` and what it offers, in short, as the list gives them.
    fn offered(&self, index: usize) -> Option<(RowView, Vec<Offered>)> {
        let row = self.rows.borrow().get(index)?.clone();
        let offered = self.offered.borrow();
        let actions = row.actions.iter().filter_map(|at| offered.get(*at as usize).cloned()).collect();
        Some((row.row, actions))
    }

    /// The row selected, if any.
    pub fn selected(&self) -> Option<RowView> {
        let index = self.tree.selected()?;
        self.rows.borrow().get(index).map(|row| row.row.clone())
    }

    /// Draws what the app last left, if it is this place's list. Focus goes to the row in hand
    /// when the app moved it — if focus is in the list, or nowhere — and stays put for a redraw.
    pub fn draw(&self, app: &App) {
        let view = app.model.view();
        if !view.ready || view.place.as_ref() != Some(&self.place) {
            return;
        }
        let Some(list) = &view.list else { return };
        let lines = app.model.lines();
        self.readback.set_label(&lines.readback);
        if self.filtered && self.filter.text() != view.query && !has_focus(&self.filter) {
            self.setting.set(true);
            self.filter.set_text(&view.query);
            self.setting.set(false);
        }
        let items = list
            .items
            .iter()
            .enumerate()
            .map(|(at, row)| Item {
                key: row.row.id.clone(),
                // Orca does not read a tree item's checked state, so "completed" is said in
                // words; the level is the tree's to report.
                text: lines.list.get(at).map_or_else(|| row.row.title.clone(), |line| line.text.clone()),
                depth: row.row.depth,
                expanded: row.row.expanded,
                swatch: row.row.swatch.clone(),
                ..Default::default()
            })
            .collect();
        *self.rows.borrow_mut() = list.items.clone();
        *self.offered.borrow_mut() = list.offered.clone();
        let rebuilt = self.tree.set(items);
        self.landing.land(&self.tree, app.window.upcast_ref(), view.focus.as_deref(), view.landing, rebuilt);
    }

    /// Whether this is the trash, where only restoring and erasing apply.
    pub fn is_trash(&self) -> bool {
        self.trash
    }

    /// Whether it has a filter to change.
    pub fn has_filter(&self) -> bool {
        self.filtered
    }

    /// Moves focus into the list once it is drawn, or now if it is.
    pub fn focus(&self) {
        self.landing.want(&self.tree);
    }

    /// Puts focus in the filter, all of it selected so typing replaces it.
    pub fn focus_filter(&self) {
        if self.filtered {
            self.filter.grab_focus();
        }
    }
}

/// Whether focus is in `widget`, or in what it holds: an entry's text.
fn has_focus(widget: &impl IsA<gtk::Widget>) -> bool {
    widget.root().and_then(|root| root.focus()).is_some_and(|focus| &focus == widget.as_ref() || focus.is_ancestor(widget))
}
