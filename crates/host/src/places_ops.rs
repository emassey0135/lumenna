//! The sidebar, as the store has it now.

use crate::places::{Places, SidebarEntry};
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

/// Every place, as the sidebar lists them, as the store has it now (`places::sidebar`).
pub fn sidebar(lumenna: &Lumenna) -> Vec<SidebarEntry> {
    let now = jiff::Zoned::now();
    lumenna.with(|store| Ok(crate::places::sidebar(&crate::repaired(store), &now))).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::places::Place;

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
