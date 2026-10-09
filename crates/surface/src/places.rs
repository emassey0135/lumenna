//! Somewhere to go, and the sidebar that lists them: the same places, in the same order and
//! the same words, in every app that has a sidebar — Windows, GTK, the web, the iPad.
//!
//! Named for what they are beside the rest of the surface (`SidebarEntry`): a bare `Group`
//! would shadow SwiftUI's in the Swift bindings.

use serde::{Deserialize, Serialize};

use crate::types::RowView;
use crate::{Lumenna, label_reference, project_reference};

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

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Every place a sidebar lists ([`sidebar`]).
    #[must_use]
    pub fn places(&self) -> Places {
        let entries = sidebar(self);
        Places { announcement: crate::words::count_line(entries.len(), "place"), notices: Vec::new(), entries }
    }
}

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
pub fn sidebar(lumenna: &Lumenna) -> Vec<SidebarEntry> {
    let place = |place: Place, detail: &str, depth: u32| SidebarEntry {
        text: line(&place.title(), detail),
        kind: SidebarKind::Place(place),
        depth,
        archived: false,
    };
    let group = |group: SidebarGroup, title: &str| SidebarEntry {
        kind: SidebarKind::Group(group),
        text: title.to_owned(),
        depth: 0,
        archived: false,
    };
    let detail = |row: &RowView| {
        row.value.iter().cloned().chain(row.state.iter().cloned()).collect::<Vec<_>>().join(", ")
    };

    let mut entries = vec![place(Place::Today, "", 0), place(Place::Tasks, "", 0)];

    entries.push(group(SidebarGroup::Projects, "Projects"));
    for row in lumenna.list_projects().map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Project(row.title.clone()), &detail(&row), row.depth + 1);
        entry.archived = row.state.iter().any(|s| s == "archived");
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Labels, "Labels"));
    for row in lumenna.list_labels().map(|r| r.rows).unwrap_or_default() {
        entries.push(place(Place::Label(row.title.clone()), &detail(&row), 1));
    }

    entries.push(group(SidebarGroup::Filters, "Saved Filters"));
    for filter in lumenna.list_filters().map(|f| f.filters).unwrap_or_default() {
        let query = filter.query.clone();
        entries.push(place(Place::Filter { name: filter.name, query: filter.query }, &query, 1));
    }

    entries.push(place(Place::Blocks, "", 0));
    let trashed = lumenna.list_tasks("deleted").map(|r| r.announcement).unwrap_or_default();
    entries.push(place(Place::Trash, &trashed, 0));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> (tempfile::TempDir, std::sync::Arc<Lumenna>) {
        let directory = tempfile::tempdir().unwrap();
        let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
        (directory, lumenna)
    }

    #[test]
    fn a_project_lists_its_own_tasks_and_new_ones_land_in_it() {
        let place = Place::Project("Home Office".to_owned());
        assert_eq!(place.query(), "#\"Home Office\"");
        assert_eq!(place.quick_add_prefix(), "#\"Home Office\" ");
        assert_eq!(Place::Trash.query(), "deleted");
    }

    #[test]
    fn the_sidebar_holds_every_place_in_order() {
        let (_directory, lumenna) = open();
        lumenna.add_project("Work", None).unwrap();
        lumenna.add_project("Reports", Some("Work".to_owned())).unwrap();
        lumenna.add_label("calls").unwrap();
        lumenna.add_filter("Urgent", "p1").unwrap();
        let entries = sidebar(&lumenna);
        let keys: Vec<(String, u32)> = entries.iter().map(|e| (e.text.clone(), e.depth)).collect();
        let position = |text: &str| keys.iter().position(|(t, _)| t.starts_with(text)).unwrap();
        assert_eq!(keys[0].0, "Today");
        assert_eq!(keys[1].0, "Tasks");
        assert!(position("Projects") < position("Work"));
        assert_eq!(keys[position("Reports")].1, keys[position("Work")].1 + 1, "subprojects nest");
        assert!(position("Labels") < position("calls"));
        assert!(position("Saved Filters") < position("Urgent, p1"));
        assert!(keys.last().unwrap().0.starts_with("Trash"));
    }

    #[test]
    fn every_entry_has_its_own_key() {
        let (_directory, lumenna) = open();
        lumenna.add_project("Work", None).unwrap();
        lumenna.add_label("Work").unwrap();
        let entries = sidebar(&lumenna);
        let mut keys: Vec<String> = entries.iter().map(SidebarEntry::key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), entries.len(), "a project and a label of one name are two places");
    }
}
