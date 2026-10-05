//! The places (§16.1: project tree, label list, saved filters): Today, Tasks, the projects as
//! they nest, labels, saved filters, Blocks, the trash.
//!
//! A tree like every list here, so a subproject's level is the tree's to report. Moving
//! through it shows each place in the middle pane, as Files' sidebar does; Enter goes into it.
//! What a project, label or filter can have done to it is in its context menu; Delete deletes
//! one, asking first.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_desktop::places::{self, Entry, Group, Kind, Place};
use lumenna_surface::{Change, Direction, Lumenna, Result, parse_weight};

use crate::prompts;
use crate::tree::{Item, Tree};
use crate::window::{App, spawn};

/// What can be done to a sidebar row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Command {
    New,
    Rename,
    NewInside,
    MoveUnder,
    Up,
    Down,
    Weight,
    Archive,
    Delete,
    Merge,
    Colour,
    Query,
}

impl Command {
    fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Rename => "rename",
            Self::NewInside => "new-inside",
            Self::MoveUnder => "move-under",
            Self::Up => "up",
            Self::Down => "down",
            Self::Weight => "weight",
            Self::Archive => "archive",
            Self::Delete => "delete",
            Self::Merge => "merge",
            Self::Colour => "colour",
            Self::Query => "query",
        }
    }
}

pub struct Sidebar {
    pub tree: Rc<Tree>,
    entries: RefCell<Vec<Entry>>,
    /// The place shown, by key.
    current: RefCell<String>,
}

impl Sidebar {
    pub fn new() -> Rc<Self> {
        let sidebar = Rc::new(Self { tree: Tree::new("Places"), entries: RefCell::new(Vec::new()), current: RefCell::new(String::new()) });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_selected(move |_| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            if let Some(Entry { kind: Kind::Place(place), .. }) = sidebar.selected() {
                let key = place_key(&place);
                if *sidebar.current.borrow() != key {
                    *sidebar.current.borrow_mut() = key;
                    app.show_place(place);
                }
            }
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_activate(move |_| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            if matches!(sidebar.selected(), Some(Entry { kind: Kind::Place(_), .. })) {
                app.focus_content();
            }
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_key(move |key, modifiers, index| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else {
                return glib::Propagation::Proceed;
            };
            // Delete here does what it does in every other list: the row's own Delete, asking
            // first as the context menu's does.
            if matches!(key, gdk::Key::Delete | gdk::Key::KP_Delete) && modifiers.is_empty() {
                if let Some(entry) = sidebar.entries.borrow().get(index).cloned().filter(deletable) {
                    sidebar.act(&app, Command::Delete, entry);
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        let weak = Rc::downgrade(&sidebar);
        sidebar.tree.connect_menu(move |index, point| {
            let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) else { return };
            let Some(entry) = sidebar.entries.borrow().get(index).cloned() else { return };
            let items: Vec<(Command, &str)> = match &entry.kind {
                Kind::Group(Group::Projects) => vec![(Command::New, "New _Project…")],
                Kind::Group(Group::Labels) => vec![(Command::New, "New _Label…")],
                Kind::Group(Group::Filters) => vec![(Command::New, "New Saved _Filter…")],
                Kind::Place(Place::Project(_)) => vec![
                    (Command::Rename, "_Rename…"),
                    (Command::NewInside, "_New Project Inside…"),
                    (Command::MoveUnder, "Move _Under…"),
                    (Command::Up, "Move U_p"),
                    (Command::Down, "Move _Down"),
                    (Command::Weight, "_Weight…"),
                    (Command::Archive, if entry.archived { "Un_archive" } else { "_Archive" }),
                    (Command::Delete, "D_elete…"),
                ],
                Kind::Place(Place::Label(_)) => vec![
                    (Command::Rename, "_Rename…"),
                    (Command::Merge, "_Merge Into…"),
                    (Command::Colour, "_Colour…"),
                    (Command::Up, "Move U_p"),
                    (Command::Down, "Move _Down"),
                    (Command::Delete, "D_elete…"),
                ],
                Kind::Place(Place::Filter { .. }) => vec![
                    (Command::Rename, "_Rename…"),
                    (Command::Query, "Change _Query…"),
                    (Command::Up, "Move U_p"),
                    (Command::Down, "Move _Down"),
                    (Command::Delete, "D_elete…"),
                ],
                Kind::Place(_) => return,
            };
            let menu = gio::Menu::new();
            let actions = gio::SimpleActionGroup::new();
            for (command, label) in items {
                let item = gio::MenuItem::new(Some(label), Some(&format!("row.{}", command.name())));
                if command == Command::Delete {
                    item.set_attribute_value("accel", Some(&"Delete".to_variant()));
                }
                menu.append_item(&item);
                let action = gio::SimpleAction::new(command.name(), None);
                let weak = Rc::downgrade(&sidebar);
                let entry = entry.clone();
                action.connect_activate(move |_, _| {
                    if let (Some(sidebar), Some(app)) = (weak.upgrade(), crate::window::app()) {
                        sidebar.act(&app, command, entry.clone());
                    }
                });
                actions.add_action(&action);
            }
            app.popup(&menu, sidebar.tree.view.upcast_ref(), point, Some(&actions));
        });
        sidebar
    }

    /// Lists the places again, keeping the one shown selected without showing it again —
    /// which would throw away whatever the middle pane was doing.
    pub fn reload(&self, app: &App) {
        let entries = places::sidebar(&app.core.lumenna);
        let items = entries.iter().map(|entry| Item { key: entry.key(), text: entry.text.clone(), depth: entry.depth }).collect();
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
}

impl Sidebar {
    fn act(self: &Rc<Self>, app: &Rc<App>, command: Command, entry: Entry) {
        let (sidebar, app) = (Rc::clone(self), Rc::clone(app));
        spawn(async move { sidebar.run(&app, command, entry).await });
    }

    async fn run(&self, app: &Rc<App>, command: Command, entry: Entry) {
        let window = &app.window;
        let lumenna = &app.core.lumenna;
        match (&entry.kind, command) {
            (Kind::Group(Group::Projects), Command::New) => new_project(app, None).await,
            (Kind::Group(Group::Labels), Command::New) => new_label(app).await,
            (Kind::Group(Group::Filters), Command::New) => new_filter(app).await,

            (Kind::Place(Place::Project(name)), command) => match command {
                Command::Rename => {
                    if let Some(to) = prompts::ask(window, &format!("Rename {name}"), "_Name:", "", name).await {
                        change(app, Some(Place::Project(to.clone())), |l| l.rename_project(name, &to));
                    }
                }
                Command::NewInside => new_project(app, Some(name.clone())).await,
                Command::MoveUnder => {
                    let mut choices = vec!["The top level".to_owned()];
                    choices.extend(project_names(lumenna).into_iter().filter(|p| p != name));
                    if let Some(index) = prompts::pick(window, &format!("Move {name}"), "_Under:", &choices).await {
                        let parent = (index > 0).then(|| choices[index].clone());
                        change(app, None, |l| l.move_project(name, parent));
                    }
                }
                Command::Up => change(app, None, |l| l.reorder_project(name, Direction::Up)),
                Command::Down => change(app, None, |l| l.reorder_project(name, Direction::Down)),
                Command::Weight => {
                    let message = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.";
                    let mut typed = "1.0".to_owned();
                    // Read as the surface reads a weight; one that does not read is said, and
                    // asked for again with what was typed, rather than taken as inherit.
                    while let Some(text) = prompts::ask(window, &format!("Weight of {name}"), "_Weight:", message, &typed).await {
                        match parse_weight(text.clone()) {
                            Ok(weight) => {
                                change(app, None, |l| l.weigh_project(name, weight));
                                break;
                            }
                            Err(error) => {
                                prompts::tell(window, &crate::core::sentence(&error)).await;
                                typed = text;
                            }
                        }
                    }
                }
                Command::Archive => change(app, None, |l| l.archive_project(name)),
                Command::Delete => {
                    let choice = prompts::choose(
                        window,
                        &format!("Delete {name}?"),
                        "Its tasks can go to the trash with it, or move to the Inbox.",
                        &["Delete and Trash Its Tasks", "Delete and Keep Its Tasks"],
                    )
                    .await;
                    if let Some(choice) = choice {
                        change(app, Some(Place::Tasks), |l| l.delete_project(name, choice == 1));
                    }
                }
                _ => {}
            },

            (Kind::Place(Place::Label(name)), command) => match command {
                Command::Rename => {
                    if let Some(to) = prompts::ask(window, &format!("Rename {name}"), "_Name:", "", name).await {
                        change(app, Some(Place::Label(to.clone())), |l| l.rename_label(name, &to));
                    }
                }
                Command::Merge => {
                    // For when a typo made a near-duplicate: this one's tasks move to the other.
                    let others: Vec<String> = label_names(lumenna).into_iter().filter(|l| l != name).collect();
                    if let Some(index) = prompts::pick(window, &format!("Merge {name}"), "_Into:", &others).await {
                        let into = others[index].clone();
                        change(app, Some(Place::Label(into.clone())), |l| l.merge_labels(name, &into));
                    }
                }
                Command::Colour => {
                    let message = "A colour name, such as red or teal, or none. The name always shows too.";
                    if let Some(colour) = prompts::ask(window, &format!("Colour for {name}"), "_Colour:", message, "").await {
                        let colour = (!colour.trim().eq_ignore_ascii_case("none")).then(|| colour.trim().to_owned());
                        change(app, None, |l| l.recolour_label(name, colour));
                    }
                }
                Command::Up => change(app, None, |l| l.reorder_label(name, Direction::Up)),
                Command::Down => change(app, None, |l| l.reorder_label(name, Direction::Down)),
                Command::Delete => {
                    let detail = "Tasks wearing it stay; they just stop showing it.";
                    if prompts::confirm(window, &format!("Delete {name}?"), detail, "Delete").await {
                        change(app, Some(Place::Tasks), |l| l.delete_label(name));
                    }
                }
                _ => {}
            },

            (Kind::Place(Place::Filter { name, query }), command) => match command {
                Command::Rename => {
                    if let Some(to) = prompts::ask(window, &format!("Rename {name}"), "_Name:", "", name).await {
                        let then = Place::Filter { name: to.clone(), query: query.clone() };
                        change(app, Some(then), |l| l.edit_filter(name, Some(to.clone()), None));
                    }
                }
                Command::Query => {
                    if let Some(new) = prompts::ask(window, &format!("Query for {name}"), "_Query:", "", query).await {
                        let then = Place::Filter { name: name.clone(), query: new.clone() };
                        change(app, Some(then), |l| l.edit_filter(name, None, Some(new.clone())));
                    }
                }
                Command::Up => change(app, None, |l| l.reorder_filter(name, Direction::Up)),
                Command::Down => change(app, None, |l| l.reorder_filter(name, Direction::Down)),
                Command::Delete => {
                    let detail = "The tasks it shows are not touched.";
                    if prompts::confirm(window, &format!("Delete {name}?"), detail, "Delete").await {
                        change(app, Some(Place::Tasks), |l| l.delete_filter(name));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Runs a change, says what it did, and — when it renamed or removed the place, or made a new
/// one — goes to `then`.
fn change(app: &App, then: Option<Place>, operation: impl FnOnce(&Lumenna) -> Result<Change>) {
    if let Some(change) = app.perform(operation) {
        if let Some(place) = then.filter(|_| change.changed) {
            app.go(place, false);
        }
        app.say_change(&change);
    }
}

pub async fn new_project(app: &App, parent: Option<String>) {
    let title = parent.as_ref().map_or_else(|| "New Project".to_owned(), |p| format!("New Project in {p}"));
    if let Some(name) = prompts::ask(&app.window, &title, "_Name:", "", "").await {
        change(app, Some(Place::Project(name.clone())), |l| l.add_project(&name, parent));
    }
}

pub async fn new_label(app: &App) {
    if let Some(name) = prompts::ask(&app.window, "New Label", "_Name:", "", "").await {
        change(app, Some(Place::Label(name.clone())), |l| l.add_label(&name));
    }
}

pub async fn new_filter(app: &App) {
    let Some(name) = prompts::ask(&app.window, "New Saved Filter", "_Name:", "", "").await else { return };
    let message = "Such as #Work & overdue, or p1 | today.";
    if let Some(query) = prompts::ask(&app.window, &format!("Query for {name}"), "_Query:", message, "").await {
        let then = Place::Filter { name: name.clone(), query: query.clone() };
        change(app, Some(then), |l| l.add_filter(&name, &query));
    }
}

fn project_names(lumenna: &Lumenna) -> Vec<String> {
    lumenna.list_projects().map(|r| r.rows.into_iter().map(|p| p.title).collect()).unwrap_or_default()
}

fn label_names(lumenna: &Lumenna) -> Vec<String> {
    lumenna.list_labels().map(|r| r.rows.into_iter().map(|l| l.title).collect()).unwrap_or_default()
}

/// Whether a row has a Delete: projects, labels and saved filters do; the fixed places and
/// the headings do not.
fn deletable(entry: &Entry) -> bool {
    matches!(entry.kind, Kind::Place(Place::Project(_) | Place::Label(_) | Place::Filter { .. }))
}

/// The key a place's row has in the sidebar.
fn place_key(place: &Place) -> String {
    Entry { kind: Kind::Place(place.clone()), text: String::new(), depth: 0, archived: false }.key()
}
