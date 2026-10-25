//! Erasing permanently: never what deleting does, asked first, waiting for every device, and
//! taking the automatic backups with it.

use lumenna_core::id::NodeId;
use lumenna_core::model::Device;
use lumenna_host::{Action, ActionKind, Answer, Lumenna, Question};

struct Opened {
    directory: tempfile::TempDir,
    lumenna: std::sync::Arc<Lumenna>,
}

/// A profile whose backups go to a folder of its own.
fn open() -> Opened {
    let directory = tempfile::tempdir().unwrap();
    let lumenna = Lumenna::open(directory.path().join("profile").to_str().unwrap()).unwrap();
    let backups = directory.path().join("backups");
    lumenna.set_setting("backup-dir", backups.to_str().unwrap()).unwrap();
    Opened { directory, lumenna }
}

impl Opened {
    fn backups(&self) -> Vec<std::path::PathBuf> {
        lumenna_store::backup::list(&self.directory.path().join("backups")).unwrap().into_iter().map(|b| b.path).collect()
    }
}

fn add(lumenna: &Lumenna, text: &str) -> String {
    lumenna.add_task(text).unwrap().affected.tasks[0].clone()
}

fn trashed_actions(lumenna: &Lumenna) -> Vec<Action> {
    lumenna.list_tasks("deleted").unwrap().rows.remove(0).actions
}

fn titles(lumenna: &Lumenna, query: &str) -> Vec<String> {
    lumenna.list_tasks(query).unwrap().rows.into_iter().map(|r| r.title).collect()
}

#[test]
fn a_trashed_task_offers_erasing_permanently_after_deleting_from_the_trash_and_never_first() {
    let opened = open();
    let id = add(&opened.lumenna, "the secret");
    opened.lumenna.trash_task(&id).unwrap();
    let actions = trashed_actions(&opened.lumenna);
    let erase = actions.iter().find(|a| a.kind == ActionKind::ErasePermanently).unwrap();
    assert_eq!(actions.last().map(|a| a.kind), Some(ActionKind::ErasePermanently));
    assert!(!erase.primary, "a menu away, never a row's own");
    assert!(erase.destructive);
    let Question::Confirm { title, message, yes } = &erase.question else { panic!("{:?}", erase.question) };
    assert_eq!(title, "Erase the secret permanently?");
    assert!(message.starts_with("Permanently erase — rebuilds your database and re-syncs all devices."), "{message}");
    assert_eq!(yes, "Erase Permanently");
    // A task not in the trash offers nothing of the kind.
    add(&opened.lumenna, "the groceries");
    let open_actions = &opened.lumenna.list_tasks("").unwrap().rows[0].actions;
    assert!(open_actions.iter().all(|a| a.kind != ActionKind::ErasePermanently));
}

#[test]
fn erasing_permanently_is_done_only_on_a_yes_and_takes_the_task_from_the_history() {
    let opened = open();
    let id = add(&opened.lumenna, "the secret");
    opened.lumenna.trash_task(&id).unwrap();
    let erase = trashed_actions(&opened.lumenna).into_iter().find(|a| a.kind == ActionKind::ErasePermanently).unwrap();
    assert!(!opened.lumenna.act(erase.clone(), Answer::Text { text: String::new() }).unwrap().changed);
    let done = opened.lumenna.act(erase, Answer::Yes).unwrap();
    assert_eq!(done.announcement, "Erased the secret permanently");
    assert!(titles(&opened.lumenna, "deleted").is_empty());
    let backup = opened.lumenna.backup_file().unwrap();
    assert!(!String::from_utf8_lossy(&backup.bytes).contains("the secret"));
}

#[test]
fn erasing_permanently_needs_the_task_in_the_trash() {
    let opened = open();
    let id = add(&opened.lumenna, "the groceries");
    let refused = opened.lumenna.erase_task_permanently(&id).unwrap_err();
    assert!(refused.to_string().contains("not in the trash"), "{refused}");
}

#[test]
fn erasing_waits_for_every_paired_device_to_be_updated_and_says_which_is_not() {
    let opened = open();
    let id = add(&opened.lumenna, "the secret");
    opened.lumenna.trash_task(&id).unwrap();
    let old = Device {
        node_id: NodeId::from_bytes([3; 32]),
        name: "Old Laptop".to_owned(),
        platform: "linux".to_owned(),
        paired_at: lumenna_core::time::now(),
        last_seen: lumenna_core::time::now(),
        schema: 1,
    };
    opened.lumenna.store().write(|docs| docs.put_device(&old, None)).unwrap();
    let refused = opened.lumenna.erase_task_permanently(&id).unwrap_err();
    assert!(refused.to_string().contains("Old Laptop runs an older version"), "{refused}");
    assert_eq!(titles(&opened.lumenna, "deleted"), ["the secret"], "nothing was erased");
}

#[test]
fn erasing_deletes_the_automatic_backups_and_takes_a_new_one() {
    let opened = open();
    let id = add(&opened.lumenna, "the secret");
    opened.lumenna.backup(None).unwrap();
    // A backup is named to the millisecond, so two in the same one are one file.
    std::thread::sleep(std::time::Duration::from_millis(5));
    opened.lumenna.backup(None).unwrap();
    let before = opened.backups();
    assert_eq!(before.len(), 2);
    opened.lumenna.trash_task(&id).unwrap();
    opened.lumenna.erase_task_permanently(&id).unwrap();
    let after = opened.backups();
    assert_eq!(after.len(), 1, "{after:?}");
    assert!(!before.contains(&after[0]), "a new one");
    let bytes = std::fs::read(&after[0]).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("the secret"));
}

#[test]
fn erasing_everything_deleted_empties_the_trash_for_good() {
    let opened = open();
    for title in ["the first secret", "the second secret"] {
        let id = add(&opened.lumenna, title);
        opened.lumenna.trash_task(&id).unwrap();
    }
    add(&opened.lumenna, "the groceries");
    let done = opened.lumenna.erase_everything_deleted().unwrap();
    assert_eq!(done.announcement, "Erased 2 tasks and the history of everything deleted");
    assert!(titles(&opened.lumenna, "deleted").is_empty());
    assert_eq!(titles(&opened.lumenna, ""), ["the groceries"]);
}

#[test]
fn a_process_sharing_the_profile_deletes_nothing_twice() {
    let opened = open();
    let other = Lumenna::open(opened.lumenna.path().to_str().unwrap()).unwrap();
    let id = add(&opened.lumenna, "the secret");
    opened.lumenna.trash_task(&id).unwrap();
    opened.lumenna.erase_task_permanently(&id).unwrap();
    let after = opened.backups();
    // The other process learns of the erasure, and leaves the new backup alone.
    other.list_tasks("").unwrap();
    assert_eq!(opened.backups(), after);
}
