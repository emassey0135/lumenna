//! Projects, labels and saved filters are reached by name, so each needs one.

use lumenna_host::{BlockEdit, BlockScope, Lumenna, NewBlock, TaskEdit};

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

#[test]
fn a_label_completes_over_what_was_typed_of_it_in_a_filter() {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = lumenna_host::Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    lumenna.add_label("calls").unwrap();
    let found = lumenna.complete_text("@ca", 3, lumenna_host::Syntax::Filter).unwrap();
    assert_eq!((found.start, found.end), (0, 3), "{found:?}");
}

#[test]
fn a_blank_title_or_name_is_refused_for_a_task_a_block_or_a_device() {
    let (_directory, lumenna) = open();
    let task = lumenna.add_task("Essay").unwrap().affected.tasks[0].clone();
    let blank = TaskEdit { title: Some(" ".to_owned()), ..TaskEdit::default() };
    assert_eq!(lumenna.edit_task(&task, blank).unwrap_err().message(), "a task needs a title");
    let new = NewBlock { title: String::new(), at: "9am".to_owned(), minutes: 60, kind: "work".to_owned(), ..NewBlock::default() };
    assert_eq!(lumenna.add_block(new).unwrap_err().message(), "a block needs a name");
    let block = NewBlock { title: "Deep work".to_owned(), at: "9am".to_owned(), minutes: 60, kind: "work".to_owned(), ..NewBlock::default() };
    let series = lumenna.add_block(block).unwrap().affected.blocks[0].clone();
    let renamed = BlockEdit { title: Some(String::new()), ..BlockEdit::default() };
    assert_eq!(lumenna.edit_block(&series, renamed, BlockScope::Series).unwrap_err().message(), "a block needs a name");
    assert_eq!(lumenna.rename_device("anything", "  ").unwrap_err().message(), "a device needs a name");
}

#[test]
fn a_priority_outside_one_to_four_is_refused_in_the_automation_s_words() {
    let (_directory, lumenna) = open();
    let task = lumenna.add_task("Essay").unwrap().affected.tasks[0].clone();
    for priority in [0, 5, 200] {
        let edit = TaskEdit { priority: Some(priority), ..TaskEdit::default() };
        let refused = lumenna.edit_task(&task, edit).unwrap_err();
        assert_eq!(refused.message(), format!("Priority takes 1 to 4, where 1 is highest, not {priority}."));
    }
    let edit = TaskEdit { priority: Some(2), ..TaskEdit::default() };
    lumenna.edit_task(&task, edit).unwrap();
    assert_eq!(lumenna.show_task(&task).unwrap().task.priority, 2);
}
