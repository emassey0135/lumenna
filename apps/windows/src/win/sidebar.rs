//! The places (§16.1: project tree, label list, saved filters): Today, Tasks, the projects as
//! they nest, labels, saved filters, Blocks, the trash.
//!
//! A tree view like every list here, so a subproject's level is the control's to report.
//! Moving through it shows each place in the middle pane, as Explorer's folder tree does.
//! What a project, label or filter can have done to it is in its context menu.

use std::cell::RefCell;

use lumenna_surface::{Change, Direction, Lumenna, Result, Weight};
use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::{NMHDR, NMTREEVIEWW, TVN_ITEMEXPANDEDW, TVN_SELCHANGEDW};

use super::app::App;
use super::controls::{self, rect};
use super::prompts;
use super::tree::{Item, Tree};
use super::view::Metrics;
use crate::places::{self, Entry, Group, Kind, Place};

const TREE: u16 = 200;

const NEW: u16 = 1;
const RENAME: u16 = 2;
const NEW_INSIDE: u16 = 3;
const MOVE_UNDER: u16 = 4;
const UP: u16 = 5;
const DOWN: u16 = 6;
const WEIGHT: u16 = 7;
const ARCHIVE: u16 = 8;
const DELETE: u16 = 9;
const MERGE: u16 = 10;
const COLOUR: u16 = 11;
const QUERY: u16 = 12;

pub struct Sidebar {
    pub tree: Tree,
    entries: RefCell<Vec<Entry>>,
    /// The place shown, by key.
    current: RefCell<String>,
}

impl Sidebar {
    pub fn create(pane: HWND) -> Self {
        Self { tree: Tree::create(pane, TREE, "Places", false), entries: RefCell::new(Vec::new()), current: RefCell::new(String::new()) }
    }

    pub fn layout(&self, width: i32, height: i32, m: Metrics) {
        let gap = m.gap();
        controls::place(self.tree.hwnd, rect(gap, gap, width - gap, height - 2 * gap));
    }

    /// Lists the places again, keeping the one shown selected without showing it again —
    /// which would throw away whatever the middle pane was doing.
    pub fn reload(&self, app: &App) {
        let entries = places::sidebar(&app.core.lumenna);
        let items = entries
            .iter()
            .map(|entry| Item { key: entry.key(), text: entry.text.clone(), depth: entry.depth, checked: None })
            .collect();
        *self.entries.borrow_mut() = entries;
        if self.tree.set(items) {
            let current = self.current.borrow().clone();
            if let Some(index) = self.tree.index_of(&current) {
                self.tree.select_quietly(index);
            }
        }
    }

    /// Selects a place, as going there from the menu does.
    pub fn select(&self, place: &Place) {
        let key = place_key(place);
        *self.current.borrow_mut() = key.clone();
        if let Some(index) = self.tree.index_of(&key) {
            self.tree.select_quietly(index);
        }
    }

    fn selected(&self) -> Option<Entry> {
        let index = self.tree.selected()?;
        self.entries.borrow().get(index).cloned()
    }

    pub fn notify(&self, app: &App, header: &NMHDR, lparam: LPARAM) -> Option<isize> {
        if header.hwndFrom != self.tree.hwnd {
            return None;
        }
        match header.code {
            TVN_SELCHANGEDW if !self.tree.busy() => {
                if let Some(Entry { kind: Kind::Place(place), .. }) = self.selected() {
                    let key = place_key(&place);
                    if *self.current.borrow() != key {
                        *self.current.borrow_mut() = key;
                        app.show_place(place);
                    }
                }
                Some(0)
            }
            TVN_ITEMEXPANDEDW => {
                self.tree.expansion_changed(unsafe { &*(lparam.0 as *const NMTREEVIEWW) });
                Some(0)
            }
            _ => None,
        }
    }

    pub fn context_menu(&self, app: &App, control: HWND, point: Option<POINT>) -> bool {
        if control != self.tree.hwnd {
            return false;
        }
        let index = point.and_then(|p| self.tree.index_at(p)).or_else(|| self.tree.selected());
        let Some(index) = index else { return true };
        let Some(entry) = self.entries.borrow().get(index).cloned() else { return true };
        let items: Vec<(u16, &str)> = match &entry.kind {
            Kind::Group(Group::Projects) => vec![(NEW, "New &Project...")],
            Kind::Group(Group::Labels) => vec![(NEW, "New &Label...")],
            Kind::Group(Group::Filters) => vec![(NEW, "New Saved &Filter...")],
            Kind::Place(Place::Project(_)) => vec![
                (RENAME, "&Rename..."),
                (NEW_INSIDE, "&New Project Inside..."),
                (MOVE_UNDER, "Move &Under..."),
                (UP, "Move U&p"),
                (DOWN, "Move &Down"),
                (WEIGHT, "&Weight..."),
                (ARCHIVE, if entry.archived { "Un&archive" } else { "&Archive" }),
                (0, ""),
                (DELETE, "D&elete..."),
            ],
            Kind::Place(Place::Label(_)) => vec![
                (RENAME, "&Rename..."),
                (MERGE, "&Merge Into..."),
                (COLOUR, "&Colour..."),
                (UP, "Move U&p"),
                (DOWN, "Move &Down"),
                (0, ""),
                (DELETE, "D&elete..."),
            ],
            Kind::Place(Place::Filter { .. }) => vec![
                (RENAME, "&Rename..."),
                (QUERY, "Change &Query..."),
                (UP, "Move U&p"),
                (DOWN, "Move &Down"),
                (0, ""),
                (DELETE, "D&elete..."),
            ],
            Kind::Place(_) => return true,
        };
        let at = point.unwrap_or_else(|| self.tree.menu_point(index));
        if let Some(command) = app.popup(&items, at) {
            self.act(app, command, &entry);
        }
        true
    }

    fn act(&self, app: &App, command: u16, entry: &Entry) {
        let owner = app.main;
        let lumenna = &app.core.lumenna;
        match (&entry.kind, command) {
            (Kind::Group(Group::Projects), NEW) => self.new_project(app, None),
            (Kind::Group(Group::Labels), NEW) => self.new_label(app),
            (Kind::Group(Group::Filters), NEW) => self.new_filter(app),

            (Kind::Place(Place::Project(name)), command) => match command {
                RENAME => {
                    if let Some(to) = prompts::ask_text(owner, &format!("Rename {name}"), "&Name:", "", name) {
                        self.change(app, Some(Place::Project(to.clone())), |l| l.rename_project(name, &to));
                    }
                }
                NEW_INSIDE => self.new_project(app, Some(name.clone())),
                MOVE_UNDER => {
                    let mut choices = vec!["The top level".to_owned()];
                    choices.extend(project_names(lumenna).into_iter().filter(|p| p != name));
                    if let Some(index) = prompts::pick(owner, &format!("Move {name}"), "&Under:", &choices) {
                        let parent = (index > 0).then(|| choices[index].clone());
                        self.change(app, None, |l| l.move_project(name, parent));
                    }
                }
                UP => self.change(app, None, |l| l.reorder_project(name, Direction::Up)),
                DOWN => self.change(app, None, |l| l.reorder_project(name, Direction::Down)),
                WEIGHT => {
                    let message = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.";
                    if let Some(text) = prompts::ask_text(owner, &format!("Weight of {name}"), "&Weight:", message, "1.0") {
                        let weight = text.parse::<f32>().map_or(Weight::Inherit, |value| Weight::Value { value });
                        self.change(app, None, |l| l.weigh_project(name, weight));
                    }
                }
                ARCHIVE => self.change(app, None, |l| l.archive_project(name)),
                DELETE => {
                    let choice = prompts::choose(
                        owner,
                        &format!("Delete {name}?"),
                        "Its tasks can go to the trash with it, or move to the Inbox.",
                        &["Delete and Trash Its Tasks", "Delete and Keep Its Tasks"],
                        true,
                    );
                    if let Some(choice) = choice {
                        self.change(app, Some(Place::Tasks), |l| l.delete_project(name, choice == 1));
                    }
                }
                _ => {}
            },

            (Kind::Place(Place::Label(name)), command) => match command {
                RENAME => {
                    if let Some(to) = prompts::ask_text(owner, &format!("Rename {name}"), "&Name:", "", name) {
                        self.change(app, Some(Place::Label(to.clone())), |l| l.rename_label(name, &to));
                    }
                }
                MERGE => {
                    // For when a typo made a near-duplicate: this one's tasks move to the other.
                    let others: Vec<String> = label_names(lumenna).into_iter().filter(|l| l != name).collect();
                    if let Some(index) = prompts::pick(owner, &format!("Merge {name}"), "&Into:", &others) {
                        let into = others[index].clone();
                        self.change(app, Some(Place::Label(into.clone())), |l| l.merge_labels(name, &into));
                    }
                }
                COLOUR => {
                    let message = "A colour name, such as red or teal, or none. The name always shows too.";
                    if let Some(colour) = prompts::ask_text(owner, &format!("Colour for {name}"), "&Colour:", message, "") {
                        let colour = (!colour.eq_ignore_ascii_case("none")).then_some(colour);
                        self.change(app, None, |l| l.recolour_label(name, colour));
                    }
                }
                UP => self.change(app, None, |l| l.reorder_label(name, Direction::Up)),
                DOWN => self.change(app, None, |l| l.reorder_label(name, Direction::Down)),
                DELETE
                    if prompts::confirm(owner, &format!("Delete {name}?"), "Tasks wearing it stay; they just stop showing it.", "Delete") => {
                        self.change(app, Some(Place::Tasks), |l| l.delete_label(name));
                    }
                _ => {}
            },

            (Kind::Place(Place::Filter { name, query }), command) => match command {
                RENAME => {
                    if let Some(to) = prompts::ask_text(owner, &format!("Rename {name}"), "&Name:", "", name) {
                        let then = Place::Filter { name: to.clone(), query: query.clone() };
                        self.change(app, Some(then), |l| l.edit_filter(name, Some(to.clone()), None));
                    }
                }
                QUERY => {
                    if let Some(new) = prompts::ask_text(owner, &format!("Query for {name}"), "&Query:", "", query) {
                        let then = Place::Filter { name: name.clone(), query: new.clone() };
                        self.change(app, Some(then), |l| l.edit_filter(name, None, Some(new.clone())));
                    }
                }
                UP => self.change(app, None, |l| l.reorder_filter(name, Direction::Up)),
                DOWN => self.change(app, None, |l| l.reorder_filter(name, Direction::Down)),
                DELETE
                    if prompts::confirm(owner, &format!("Delete {name}?"), "The tasks it shows are not touched.", "Delete") => {
                        self.change(app, Some(Place::Tasks), |l| l.delete_filter(name));
                    }
                _ => {}
            },
            _ => {}
        }
    }

    /// Runs a change, says what it did, and — when it renamed or removed the place, or made
    /// a new one — goes to `then`.
    fn change(&self, app: &App, then: Option<Place>, operation: impl FnOnce(&Lumenna) -> Result<Change>) {
        if let Some(change) = app.perform(operation) {
            if let Some(place) = then.filter(|_| change.changed) {
                app.go(place, false);
            }
            app.say_change(&change);
        }
    }

    pub fn new_project(&self, app: &App, parent: Option<String>) {
        let title = parent.as_ref().map_or_else(|| "New Project".to_owned(), |p| format!("New Project in {p}"));
        if let Some(name) = prompts::ask_text(app.main, &title, "&Name:", "", "") {
            self.change(app, Some(Place::Project(name.clone())), |l| l.add_project(&name, parent));
        }
    }

    pub fn new_label(&self, app: &App) {
        if let Some(name) = prompts::ask_text(app.main, "New Label", "&Name:", "", "") {
            self.change(app, Some(Place::Label(name.clone())), |l| l.add_label(&name));
        }
    }

    pub fn new_filter(&self, app: &App) {
        let Some(name) = prompts::ask_text(app.main, "New Saved Filter", "&Name:", "", "") else { return };
        let message = "Such as #Work & overdue, or p1 | today.";
        if let Some(query) = prompts::ask_text(app.main, &format!("Query for {name}"), "&Query:", message, "") {
            let then = Place::Filter { name: name.clone(), query: query.clone() };
            self.change(app, Some(then), |l| l.add_filter(&name, &query));
        }
    }
}

fn place_key(place: &Place) -> String {
    Entry { kind: Kind::Place(place.clone()), text: String::new(), depth: 0, archived: false }.key()
}

fn project_names(lumenna: &Lumenna) -> Vec<String> {
    lumenna.list_projects().map(|r| r.rows.into_iter().map(|p| p.title).collect()).unwrap_or_default()
}

fn label_names(lumenna: &Lumenna) -> Vec<String> {
    lumenna.list_labels().map(|r| r.rows.into_iter().map(|l| l.title).collect()).unwrap_or_default()
}
