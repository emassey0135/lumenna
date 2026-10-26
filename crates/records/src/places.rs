//! Somewhere to go, and the sidebar that lists them: the same places, in the same order and
//! the same words, in every app that has a sidebar — Windows, GTK, the web, the iPad.
//!
//! Named for what they are beside the rest of the surface (`SidebarEntry`): a bare `Group`
//! would shadow SwiftUI's in the Swift bindings.

use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::types::RowView;

use crate::{label_reference, project_reference};

/// Somewhere to go.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Place {
    /// The day planner.
    Today,
    /// Every open task.
    Tasks,
    /// One project's tasks.
    Project(String),
    /// The tasks wearing one label.
    Label(String),
    /// A saved filter's tasks.
    Filter {
        /// Its name.
        name: String,
        /// The query it runs.
        query: String,
    },
    /// Every block series.
    Blocks,
    /// Trashed tasks.
    Trash,
    /// What was done lately: tasks completed and worked on, newest first.
    History,
}

impl Place {
    /// What the window and the list are called while it is shown.
    pub fn title(&self) -> String {
        match self {
            Self::Today => "Today".to_owned(),
            Self::Tasks => "Tasks".to_owned(),
            Self::Project(name) | Self::Label(name) | Self::Filter { name, .. } => name.clone(),
            Self::Blocks => "Blocks".to_owned(),
            Self::Trash => "Trash".to_owned(),
            Self::History => "History".to_owned(),
        }
    }

    /// The query a task list starts from.
    pub fn query(&self) -> String {
        match self {
            Self::Project(name) => project_reference(name.clone()),
            Self::Label(name) => label_reference(name.clone()),
            Self::Filter { query, .. } => query.clone(),
            Self::Trash => "deleted".to_owned(),
            Self::Today | Self::Tasks | Self::Blocks | Self::History => String::new(),
        }
    }

    /// What a new task typed here starts with, so it lands where it was added.
    pub fn quick_add_prefix(&self) -> String {
        match self {
            Self::Project(name) => format!("{} ", project_reference(name.clone())),
            Self::Label(name) => format!("{} ", label_reference(name.clone())),
            _ => String::new(),
        }
    }
}

/// What a sidebar row is.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SidebarKind {
    /// Somewhere to go.
    Place(Place),
    /// "Projects", "Labels", "Saved Filters": a heading over places, with its own actions.
    Group(SidebarGroup),
}

/// The sidebar's headings.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SidebarGroup {
    /// The project tree.
    Projects,
    /// Every label.
    Labels,
    /// The saved filters.
    Filters,
}

impl SidebarGroup {
    /// How many of the group's places there are, in words: "no projects", "1 label".
    #[must_use]
    pub fn count(self, count: usize) -> String {
        let noun = match self {
            Self::Projects => "project",
            Self::Labels => "label",
            Self::Filters => "filter",
        };
        crate::words::count_line(count, noun)
    }

    /// What the group's list says with nothing in it, as its listing says it.
    #[must_use]
    pub fn empty(self) -> String {
        match self {
            Self::Projects => "No projects.".to_owned(),
            Self::Labels => "No labels.".to_owned(),
            Self::Filters => "No saved filters. A filter's query is kept here under a name.".to_owned(),
        }
    }
}

/// One sidebar row.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarEntry {
    /// What it is.
    pub kind: SidebarKind,
    /// What identifies it across a reload, so the row in hand stays put: [`SidebarEntry::key_of`].
    pub key: String,
    /// The line as read: the name, then what is in it.
    pub text: String,
    /// Its name alone: "Work", "Projects".
    pub title: String,
    /// What is in it, for a second line or a description: "3 open tasks". Empty for a heading.
    pub detail: String,
    /// How deep it sits: a subproject is one deeper than its parent.
    pub depth: u32,
    /// For a project: whether it is archived, which decides "Archive" or "Unarchive".
    pub archived: bool,
    /// A project's, label's or saved filter's colour, to draw beside its name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swatch: Option<crate::types::Swatch>,
    /// What can be done to it: a project's, label's or saved filter's own, a heading's New.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<crate::actions::Action>,
    /// What can be done to it, in short, as a view offers it (see [`crate::actions::Offered`]);
    /// empty outside a view, where `actions` are whole.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<crate::actions::Offered>,
    /// Folded away under a collapsed row, in a view: a tree control that folds by itself takes
    /// every row, a flat list shows only the rows not hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// For a row with rows under it, in a view: whether they are shown. None for a row with
    /// none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// In a view, "level 2" where the level differs from the row shown before, as a list's rows
    /// say it (`words::level`); depth said on every row says too much.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
}

impl SidebarEntry {
    /// A row: its line is its title, then what is in it.
    #[must_use]
    pub fn new(kind: SidebarKind, title: &str, detail: &str, depth: u32, actions: Vec<crate::actions::Action>) -> Self {
        Self {
            key: Self::key_of(&kind),
            text: line(title, detail),
            title: title.to_owned(),
            detail: detail.to_owned(),
            kind,
            depth,
            archived: false,
            swatch: None,
            actions,
            offered: Vec::new(),
            hidden: false,
            expanded: None,
            level: None,
        }
    }

    /// What identifies it across a reload, so the selection stays put.
    pub fn key(&self) -> String {
        self.key.clone()
    }

    /// What identifies a row of this kind, whether or not one is listed.
    #[must_use]
    pub fn key_of(kind: &SidebarKind) -> String {
        match kind {
            SidebarKind::Group(group) => format!("group:{group:?}"),
            SidebarKind::Place(place) => format!("place:{place:?}"),
        }
    }
}

/// A place in the sidebar, with what is in it: "Work, 3 tasks".
#[must_use]
pub fn line(title: &str, detail: &str) -> String {
    if detail.is_empty() { title.to_owned() } else { format!("{title}, {detail}") }
}

/// The sidebar, for an app that cannot call [`sidebar`] itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Places {
    /// How many places there are, in a sentence.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// Every row, in order: groups at depth 0 with their places beneath.
    pub entries: Vec<SidebarEntry>,
}

crate::announced!(Places);


/// What a place is called while it is shown.
#[must_use]
pub fn place_title(place: &Place) -> String {
    place.title()
}

/// The query a task list for a place starts from.
#[must_use]
pub fn place_query(place: &Place) -> String {
    place.query()
}

/// What a new task typed in a place starts with.
#[must_use]
pub fn place_quick_add_prefix(place: &Place) -> String {
    place.quick_add_prefix()
}

/// Every place, as the sidebar lists them: Today, Tasks, the project tree, labels, saved
/// filters, blocks, the trash.
///
/// A listing that fails leaves its group empty rather than the whole sidebar: the places
/// that do not depend on it still work.
pub fn sidebar(snapshot: &Snapshot, now: &Zoned) -> Vec<SidebarEntry> {
    let place = |place: Place, detail: &str, depth: u32| {
        let title = place.title();
        SidebarEntry::new(SidebarKind::Place(place), &title, detail, depth, Vec::new())
    };
    let group = |group: SidebarGroup, title: &str| {
        SidebarEntry::new(SidebarKind::Group(group), title, "", 0, crate::actions::heading(group))
    };
    let detail = |row: &RowView| {
        row.value.iter().cloned().chain(row.state.iter().cloned()).collect::<Vec<_>>().join(", ")
    };

    let mut entries = vec![place(Place::Today, "", 0), place(Place::Tasks, "", 0)];

    entries.push(group(SidebarGroup::Projects, "Projects"));
    for row in crate::lists::projects(snapshot).map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Project(row.title.clone()), &detail(&row), row.depth + 1);
        entry.archived = row.state.iter().any(|s| s == "archived");
        entry.swatch = row.swatch;
        entry.actions = row.actions;
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Labels, "Labels"));
    for row in crate::lists::labels(snapshot).map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Label(row.title.clone()), &detail(&row), 1);
        entry.swatch = row.swatch;
        entry.actions = row.actions;
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Filters, "Saved Filters"));
    for filter in crate::lists::filters(snapshot).map(|f| f.filters).unwrap_or_default() {
        let detail = filter.colour.as_ref().map_or_else(|| filter.query.clone(), |colour| format!("{}, {colour}", filter.query));
        let mut entry = place(Place::Filter { name: filter.name, query: filter.query }, &detail, 1);
        entry.swatch = filter.swatch;
        entry.actions = filter.actions;
        entries.push(entry);
    }

    // How many block series, as the trash says how many tasks: a list of places that does not
    // show them beneath (the iPhone's Browse) says what is in each.
    let blocks = crate::plan::blocks(snapshot, now.date()).map(|r| r.announcement).unwrap_or_default();
    entries.push(place(Place::Blocks, &blocks, 0));
    let done = crate::lists::history(snapshot, now).map(|r| r.announcement).unwrap_or_default();
    entries.push(place(Place::History, &done, 0));
    let trashed = crate::lists::tasks(snapshot, "deleted", now).map(|r| r.announcement).unwrap_or_default();
    entries.push(place(Place::Trash, &trashed, 0));
    entries
}
