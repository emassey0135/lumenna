//! Projects, labels and saved filters are reached by name, so each needs one.

use lumenna_surface::Lumenna;

fn open() -> (tempfile::TempDir, std::sync::Arc<Lumenna>) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, lumenna)
}

#[test]
fn a_blank_name_is_refused_for_a_new_project_label_or_saved_filter() {
    let (_directory, lumenna) = open();
    assert_eq!(lumenna.add_project("", None).unwrap_err().message(), "a project needs a name");
    assert_eq!(lumenna.add_project("  ", None).unwrap_err().message(), "a project needs a name");
    assert_eq!(lumenna.add_label(" ").unwrap_err().message(), "a label needs a name");
    assert_eq!(lumenna.add_filter("", "p1").unwrap_err().message(), "a saved filter needs a name");
    assert_eq!(lumenna.list_projects().unwrap().count, 1, "only the Inbox");
}

#[test]
fn a_blank_name_is_refused_when_renaming() {
    let (_directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    lumenna.add_label("calls").unwrap();
    lumenna.add_filter("Urgent", "p1").unwrap();
    assert!(lumenna.rename_project("Work", "").is_err());
    assert!(lumenna.rename_label("calls", " ").is_err());
    assert!(lumenna.edit_filter("Urgent", Some(String::new()), None).is_err());
    assert!(lumenna.edit_filter("Urgent", None, Some("p2".to_owned())).is_ok(), "a new query alone needs no name");
}

#[test]
fn a_subproject_is_listed_under_its_parent_whatever_order_it_was_made_in() {
    let (_directory, lumenna) = open();
    for name in ["Work", "Test", "Home"] {
        lumenna.add_project(name, None).unwrap();
    }
    lumenna.move_project("Test", Some("Work".to_owned())).unwrap();
    let listed: Vec<(String, u32)> =
        lumenna.list_projects().unwrap().rows.into_iter().map(|r| (r.title, r.depth)).collect();
    let expected = [("Inbox", 0), ("Work", 0), ("Test", 1), ("Home", 0)];
    assert_eq!(listed, expected.map(|(t, d)| (t.to_owned(), d)));
}
