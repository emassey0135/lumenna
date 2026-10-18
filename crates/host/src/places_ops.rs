//! The sidebar, as the store has it now.

use crate::places::{Place, Places, SidebarEntry, SidebarGroup, SidebarKind, line};
use crate::types::RowView;
use crate::Lumenna;

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Every place a sidebar lists ([`sidebar`]).
    #[must_use]
    pub fn places(&self) -> Places {
        let entries = sidebar(self);
        Places { announcement: crate::words::count_line(entries.len(), "place"), notices: Vec::new(), entries }
    }
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
    for row in lumenna.list_projects().map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Project(row.title.clone()), &detail(&row), row.depth + 1);
        entry.archived = row.state.iter().any(|s| s == "archived");
        entry.actions = row.actions;
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Labels, "Labels"));
    for row in lumenna.list_labels().map(|r| r.rows).unwrap_or_default() {
        let mut entry = place(Place::Label(row.title.clone()), &detail(&row), 1);
        entry.actions = row.actions;
        entries.push(entry);
    }

    entries.push(group(SidebarGroup::Filters, "Saved Filters"));
    for filter in lumenna.list_filters().map(|f| f.filters).unwrap_or_default() {
        let query = filter.query.clone();
        let mut entry = place(Place::Filter { name: filter.name, query: filter.query }, &query, 1);
        entry.actions = filter.actions;
        entries.push(entry);
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
