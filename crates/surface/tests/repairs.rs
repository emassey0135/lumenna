//! A loop that two devices made between them is fixed once, written down, and said once.

use lumenna_surface::{Lumenna, MoveTarget};

fn open(at: &std::path::Path) -> std::sync::Arc<Lumenna> {
    Lumenna::open(at.to_str().unwrap()).unwrap()
}

fn id(lumenna: &Lumenna, title: &str) -> String {
    lumenna.list_tasks("").unwrap().rows.into_iter().find(|r| r.title == title).unwrap().id
}

#[test]
fn a_loop_made_by_two_devices_is_repaired_and_said_once() {
    let directory = tempfile::tempdir().unwrap();
    let backups = directory.path().join("backups");
    let a = open(&directory.path().join("a"));
    a.add_task("essay").unwrap();
    a.add_task("outline").unwrap();
    let shared = a.backup(Some(backups.to_str().unwrap().to_owned())).unwrap().path;

    // The same two tasks on a second device, which puts them the other way round.
    let b = open(&directory.path().join("b"));
    b.restore(&shared).unwrap();
    let (essay, outline) = (id(&a, "essay"), id(&a, "outline"));
    a.move_task(&outline, MoveTarget::Parent { id: essay.clone() }).unwrap();
    b.move_task(&essay, MoveTarget::Parent { id: outline.clone() }).unwrap();
    let theirs = b.backup(Some(backups.to_str().unwrap().to_owned())).unwrap().path;

    // Said by what brought the loop in.
    let restored = a.restore(&theirs).unwrap();
    assert!(
        restored.notices.iter().any(|n| n.contains("was in a loop of subtasks")),
        "the repair is said: {:?}",
        restored.notices
    );
    let after = a.list_tasks("").unwrap();
    assert!(after.rows.iter().any(|r| r.depth == 0), "something is at the top again");
    assert!(!after.notices.iter().any(|n| n.contains("loop")), "and said once: {:?}", after.notices);

    // Written down, not just repaired on read: the store itself no longer holds the loop.
    let reopened = open(&directory.path().join("a"));
    assert!(!reopened.list_tasks("").unwrap().notices.iter().any(|n| n.contains("loop")));
}
