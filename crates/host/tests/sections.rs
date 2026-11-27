//! Sections: a project's listing headed by them, their actions, and a task put under one.

use lumenna_host::{Action, ActionKind, Answer, Lumenna, Question, Subject};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

fn titles(actions: &[Action]) -> Vec<&str> {
    actions.iter().map(|a| a.title.as_str()).collect()
}

fn find(actions: &[Action], title: &str) -> Action {
    actions.iter().find(|a| a.title == title).unwrap_or_else(|| panic!("no {title} in {:?}", titles(actions))).clone()
}

/// Work with two sections, Errands and Calls, a task under each and one under neither.
fn work() -> (tempfile::TempDir, Lumenna) {
    let (directory, lumenna) = open();
    lumenna.add_project("Work", None).unwrap();
    lumenna.add_section("Work", "Errands").unwrap();
    lumenna.add_section("Work", "Calls").unwrap();
    lumenna.add_task("Post the parcel #Work /Errands").unwrap();
    lumenna.add_task("Ring the bank #Work /Calls").unwrap();
    lumenna.add_task("Tidy the desk #Work").unwrap();
    (directory, lumenna)
}

#[test]
fn a_projects_listing_puts_its_loose_tasks_first_then_each_section_with_its_tasks_beneath() {
    let (_directory, lumenna) = work();
    let rows = lumenna.list_tasks("#Work").unwrap();
    let lines: Vec<(&str, &str, u32)> = rows.rows.iter().map(|r| (r.role.as_str(), r.title.as_str(), r.depth)).collect();
    assert_eq!(
        lines,
        [
            ("task", "Tidy the desk", 0),
            ("section", "Errands", 0),
            ("task", "Post the parcel", 1),
            ("section", "Calls", 0),
            ("task", "Ring the bank", 1),
        ]
    );
    assert_eq!(rows.announcement, "3 tasks", "the headings are not counted as tasks");
    let errands = &rows.rows[1];
    assert_eq!((errands.index, errands.count), (2, 3), "a heading is a sibling of the loose tasks");
    assert_eq!(errands.expanded, Some(true));
    assert_eq!(errands.row, 1, "numbered among the sections");
    assert_eq!(rows.rows[2].row, 2, "numbered among the tasks");
    assert_eq!(
        titles(&errands.actions),
        ["Rename", "New Task", "Move Down", "Delete"],
        "the first section moves only down"
    );
    // Any other listing is tasks alone.
    assert!(lumenna.list_tasks("").unwrap().rows.iter().all(|r| r.role == "task"));
}

#[test]
fn a_sections_actions_add_rename_move_and_delete_it_keeping_or_trashing_its_tasks() {
    let (_directory, lumenna) = work();
    let heading = |name: &str| {
        lumenna.list_tasks("#Work").unwrap().rows.into_iter().find(|r| r.role == "section" && r.title == name).unwrap()
    };
    let added = lumenna.act(find(&heading("Errands").actions, "New Task"), Answer::Text { text: "Buy stamps p2".to_owned() }).unwrap();
    assert!(added.announcement.contains("in Work, under Errands"), "{}", added.announcement);
    assert_eq!(heading("Errands").value.as_deref(), Some("section, 2 tasks"));

    lumenna.act(find(&heading("Errands").actions, "Rename"), Answer::Text { text: "Out and About".to_owned() }).unwrap();
    lumenna.act(find(&heading("Calls").actions, "Move Up"), Answer::Yes).unwrap();
    let order: Vec<String> = lumenna.list_sections("Work").unwrap().rows.into_iter().map(|r| r.title).collect();
    assert_eq!(order, ["Calls", "Out and About"]);

    let delete = find(&heading("Calls").actions, "Delete");
    let Question::Choose { answers, .. } = &delete.question else { panic!("{:?}", delete.question) };
    assert_eq!(answers.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), ["trash", "keep"]);
    lumenna.act(delete, Answer::Picked { id: "keep".to_owned(), length: None }).unwrap();
    let rows = lumenna.list_tasks("#Work").unwrap().rows;
    assert!(rows.iter().any(|r| r.title == "Ring the bank" && r.depth == 0), "kept, under no section");

    let delete = find(&heading("Out and About").actions, "Delete");
    lumenna.act(delete, Answer::Picked { id: "trash".to_owned(), length: None }).unwrap();
    let rows = lumenna.list_tasks("#Work").unwrap().rows;
    assert!(!rows.iter().any(|r| r.title == "Post the parcel"), "trashed with it");
    // One undo gives back the section and its tasks under it.
    lumenna.undo().unwrap();
    let rows = lumenna.list_tasks("#Work").unwrap().rows;
    assert!(rows.iter().any(|r| r.role == "section" && r.title == "Out and About"));
    assert!(rows.iter().any(|r| r.title == "Post the parcel" && r.depth == 1));
}

#[test]
fn move_to_section_offers_the_projects_other_sections_and_none_and_moving_to_another_project_leaves_it() {
    let (_directory, lumenna) = work();
    let task = |title: &str| lumenna.list_tasks("#Work").unwrap().rows.into_iter().find(|r| r.title == title).unwrap();
    let parcel = task("Post the parcel");
    let moving = find(&parcel.actions, "Move to Section");
    assert_eq!((moving.kind, moving.subject), (ActionKind::MoveToSection, Subject::Task));
    let offered: Vec<String> = lumenna.choices(moving.clone()).unwrap().choices.into_iter().map(|c| c.title).collect();
    assert_eq!(offered, ["No Section", "Calls"]);
    let calls = lumenna.choices(moving.clone()).unwrap().choices.into_iter().find(|c| c.title == "Calls").unwrap();
    lumenna.act(moving, Answer::Picked { id: calls.id, length: None }).unwrap();
    assert_eq!(lumenna.list_tasks("#Work & /Calls").unwrap().rows.len(), 2, "a filter naming a section lists its tasks alone");

    lumenna.add_project("Home", None).unwrap();
    lumenna.move_task(&parcel.id, lumenna_host::MoveTarget::Project { name: "Home".to_owned() }).unwrap();
    lumenna.move_task(&parcel.id, lumenna_host::MoveTarget::Project { name: "Work".to_owned() }).unwrap();
    assert_eq!(task("Post the parcel").depth, 0, "a section is its project's: it left it with the project");

    // A project with no sections offers no Move to Section.
    lumenna.add_task("Water the plants #Home").unwrap();
    let plants = lumenna.list_tasks("#Home").unwrap().rows.into_iter().find(|r| r.title == "Water the plants").unwrap();
    assert!(!titles(&plants.actions).contains(&"Move to Section"));
}

#[test]
fn a_project_offers_new_section_and_a_section_named_twice_is_refused() {
    let (_directory, lumenna) = work();
    let projects = lumenna.list_projects().unwrap().rows;
    let work = projects.iter().find(|r| r.title == "Work").unwrap();
    let new = find(&work.actions, "New Section");
    assert_eq!((new.kind, new.subject), (ActionKind::New, Subject::Section));
    lumenna.act(new.clone(), Answer::Text { text: "Reading".to_owned() }).unwrap();
    assert!(lumenna.act(new, Answer::Text { text: "reading".to_owned() }).is_err());
    assert_eq!(lumenna.list_sections("Work").unwrap().rows.len(), 3);
}
