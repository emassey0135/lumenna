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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
        }
    }

    /// The query a task list starts from.
    pub fn query(&self) -> String {
        match self {
            Self::Project(name) => project_reference(name.clone()),
            Self::Label(name) => label_reference(name.clone()),
            Self::Filter { query, .. } => query.clone(),
            Self::Trash => "deleted".to_owned(),
            Self::Today | Self::Tasks | Self::Blocks => String::new(),
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum SidebarKind {
    /// Somewhere to go.
    Place(Place),
    /// "Projects", "Labels", "Saved Filters": a heading over places, with its own actions.
    Group(SidebarGroup),
}

/// The sidebar's headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum SidebarGroup {
    /// The project tree.
    Projects,
    /// Every label.
    Labels,
    /// The saved filters.
    Filters,
}

/// One sidebar row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SidebarEntry {
    /// What it is.
    pub kind: SidebarKind,
    /// The line as read: the name, then what is in it.
    pub text: String,
    /// How deep it sits: a subproject is one deeper than its parent.
    pub depth: u32,
    /// For a project: whether it is archived, which decides "Archive" or "Unarchive".
    pub archived: bool,
    /// What can be done to it: a project's, label's or saved filter's own, a heading's New.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<crate::actions::Action>,
}

impl SidebarEntry {
    /// What identifies it across a reload, so the selection stays put.
    pub fn key(&self) -> String {
        match &self.kind {
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
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
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
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn place_title(place: &Place) -> String {
    place.title()
}

/// The query a task list for a place starts from.
#[cfg_attr(feature = "uniffi", uniffi::export)]
#[must_use]
pub fn place_query(place: &Place) -> String {
    place.query()
}

/// What a new task typed in a place starts with.
#[cfg_attr(feature = "uniffi", uniffi::export)]
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
    let place = |place: Place, detail: &str, depth: u32| SidebarEntry {
        text: line(&place.title(), detail),
        kind: SidebarKind::Place(place),
        depth,
        archived: false,
        actions: Vec::new(),
    };
    let group = |group: SidebarGroup, title: &str| SidebarEntry {
        kind: SidebarKind::Group(group),
        text: title.to_owned(),
        depth: 0,
        archived: false,
        actions: crate::actions::heading(group),
    };
    let detail = |row: &RowView| {
        row.value.iter().cloned().chain(row.state.iter().cloned()).collect::<Vec<_>>().join(", ")
    };

    let mut entries = vec![place(Place::Today, "", 0), place(Place::Tasks, "", 0)];

    entries.push(group(SidebarGroup::Projects, "Projects"));
    for row in crate::lists::projects(snapshot).map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Project(row.title.clone()), &detail(&row), row.depth + 1);
        entry.archived = row.state.iter().any(|s| s == "archived");
        entry.actions = row.actions;
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Labels, "Labels"));
    for row in crate::lists::labels(snapshot).map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Label(row.title.clone()), &detail(&row), 1);
        entry.actions = row.actions;
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Filters, "Saved Filters"));
    for filter in crate::lists::filters(snapshot).map(|f| f.filters).unwrap_or_default() {
        let query = filter.query.clone();
        let mut entry = place(Place::Filter { name: filter.name, query: filter.query }, &query, 1);
        entry.actions = filter.actions;
        entries.push(entry);
    }

    entries.push(place(Place::Blocks, "", 0));
    let trashed = crate::lists::tasks(snapshot, "deleted", now).map(|r| r.announcement).unwrap_or_default();
    entries.push(place(Place::Trash, &trashed, 0));
    entries
}
