//! Somewhere to go in the main window, and the sidebar that lists them.

use lumenna_surface::{Lumenna, RowView, label_reference, project_reference};

use crate::speech;

/// Somewhere to go.
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Somewhere to go.
    Place(Place),
    /// "Projects", "Labels", "Saved Filters": a heading over places, with its own actions.
    Group(Group),
}

/// The sidebar's headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// The project tree.
    Projects,
    /// Every label.
    Labels,
    /// The saved filters.
    Filters,
}

/// One sidebar row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// What it is.
    pub kind: Kind,
    /// The line as read: the name, then what is in it.
    pub text: String,
    /// How deep it sits: a subproject is one deeper than its parent.
    pub depth: u32,
    /// For a project: whether it is archived, which decides "Archive" or "Unarchive".
    pub archived: bool,
}

impl Entry {
    /// What identifies it across a reload, so the selection stays put.
    pub fn key(&self) -> String {
        match &self.kind {
            Kind::Group(group) => format!("group:{group:?}"),
            Kind::Place(place) => format!("place:{place:?}"),
        }
    }
}

/// Every place, as the sidebar lists them: Today, Tasks, the project tree, labels, saved
/// filters, blocks, the trash.
///
/// A listing that fails leaves its group empty rather than the whole sidebar: the places
/// that do not depend on it still work.
pub fn sidebar(lumenna: &Lumenna) -> Vec<Entry> {
    let place = |place: Place, detail: &str, depth: u32| Entry {
        text: speech::place(&place.title(), detail),
        kind: Kind::Place(place),
        depth,
        archived: false,
    };
    let group = |group: Group, title: &str| Entry {
        kind: Kind::Group(group),
        text: title.to_owned(),
        depth: 0,
        archived: false,
    };
    let detail = |row: &RowView| {
        row.value.iter().cloned().chain(row.state.iter().cloned()).collect::<Vec<_>>().join(", ")
    };

    let mut entries = vec![place(Place::Today, "", 0), place(Place::Tasks, "", 0)];

    entries.push(group(Group::Projects, "Projects"));
    for row in lumenna.list_projects().map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Project(row.title.clone()), &detail(&row), row.depth + 1);
        entry.archived = row.state.iter().any(|s| s == "archived");
        entries.push(entry);
    }

    entries.push(group(Group::Labels, "Labels"));
    for row in lumenna.list_labels().map(|r| r.rows).unwrap_or_default() {
        entries.push(place(Place::Label(row.title.clone()), &detail(&row), 1));
    }

    entries.push(group(Group::Filters, "Saved Filters"));
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
        let mut keys: Vec<String> = entries.iter().map(Entry::key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), entries.len(), "a project and a label of one name are two places");
    }
}
