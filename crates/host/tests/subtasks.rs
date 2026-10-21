//! A task's subtasks: how many are done, said on its row, and the done ones shown where asked.

use lumenna_host::{Lumenna, MoveTarget};

fn open() -> (tempfile::TempDir, Lumenna) {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().to_str().unwrap()).unwrap();
    (directory, std::sync::Arc::try_unwrap(lumenna).ok().unwrap())
}

#[test]
fn a_parent_says_how_many_subtasks_are_done_and_the_setting_shows_the_done_ones_under_it() {
    let (_directory, lumenna) = open();
    let trip = lumenna.add_task("plan the trip").unwrap().affected.tasks[0].clone();
    for title in ["book the train", "find a hotel"] {
        let id = lumenna.add_task(title).unwrap().affected.tasks[0].clone();
        lumenna.move_task(&id, MoveTarget::Parent { id: trip.clone() }).unwrap();
    }
    let train = lumenna.list_tasks("").unwrap().rows.into_iter().find(|r| r.title == "book the train").unwrap().id;
    lumenna.complete_task(&train).unwrap();

    let rows = lumenna.list_tasks("").unwrap().rows;
    let parent = rows.iter().find(|r| r.title == "plan the trip").unwrap();
    assert_eq!(parent.value.as_deref(), Some("1 of 2 subtasks done"));
    assert!(!rows.iter().any(|r| r.title == "book the train"), "a done subtask is left out by default");

    lumenna.set_setting("show-completed-subtasks", "true").unwrap();
    let rows = lumenna.list_tasks("").unwrap().rows;
    let titles: Vec<&str> = rows.iter().map(|r| r.title.as_str()).collect();
    let at = titles.iter().position(|t| *t == "book the train").expect("shown under its task");
    assert_eq!(rows[at].depth, 1);
    assert_eq!(rows[at].checked, Some(true));
    assert!(titles.iter().position(|t| *t == "plan the trip").unwrap() < at);
}
