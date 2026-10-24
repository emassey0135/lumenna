//! Real erasure: the store's history begun again without what was erased, which every device
//! takes in, keeping what it did that the rebuild never saw, and which nothing older can
//! bring back.

use lumenna_core::ProjectId;
use lumenna_core::edit::{create_task, purge_task, update_task};
use lumenna_core::model::Task;
use lumenna_core::order::OrderKey;
use lumenna_store::{Documents, Store, SyncName, SyncState};

fn add(store: &mut Store, title: &str) -> Task {
    let task = Task::new(ProjectId::INBOX, title, OrderKey::middle());
    store.apply_recorded(&create_task(task.clone())).unwrap();
    task
}

fn rename(store: &mut Store, task: &Task, title: &str) {
    let before = store.snapshot().0.tasks[&task.id].clone();
    let after = Task { title: title.to_owned(), ..before.clone() };
    store.apply_recorded(&update_task(before, after)).unwrap();
}

fn erase(store: &mut Store, task: &Task) {
    let edit = purge_task(&store.snapshot().0, task.id).unwrap();
    store.erase(Documents::erased_by(&edit)).unwrap();
}

fn titles(store: &mut Store) -> Vec<String> {
    let mut titles: Vec<String> = store.snapshot().0.tasks.values().map(|t| t.title.clone()).collect();
    titles.sort();
    titles
}

/// Syncs two stores as a session does: every document of the newest generation either holds,
/// then each takes in a newer one it was sent.
fn sync(a: &mut Store, b: &mut Store) {
    let names: Vec<SyncName> = a.sync_documents().unwrap().into_iter().chain(b.sync_documents().unwrap()).collect();
    let newest = names.iter().map(|n| n.generation.clone()).max().unwrap();
    let mut documents: Vec<SyncName> = names.into_iter().filter(|n| n.generation == newest).collect();
    documents.sort();
    documents.dedup();
    for name in &documents {
        let (mut to_b, mut to_a) = (SyncState::new(), SyncState::new());
        for _ in 0..100 {
            let from_a = a.sync_message(name, &mut to_b).unwrap();
            let from_b = b.sync_message(name, &mut to_a).unwrap();
            if from_a.is_none() && from_b.is_none() {
                break;
            }
            if let Some(message) = from_a {
                b.receive_sync_message(name, &mut to_a, &message).unwrap();
            }
            if let Some(message) = from_b {
                a.receive_sync_message(name, &mut to_b, &message).unwrap();
            }
        }
    }
    a.finish_adoption().unwrap();
    b.finish_adoption().unwrap();
}

/// Whether any change in any document of `store`'s backup, its whole history, writes `text`.
fn history_holds(store: &mut Store, text: &str) -> bool {
    let backup = lumenna_store::backup::decode(&store.backup().unwrap()).unwrap();
    backup.documents.iter().any(|(_, bytes)| {
        let mut doc = automerge::AutoCommit::load(bytes).unwrap();
        let in_changes = doc.get_changes(&[]).iter().any(|change| {
            change.decode().operations.iter().any(|op| {
                matches!(&op.action, automerge::legacy::OpType::Put(automerge::ScalarValue::Str(s)) if s.contains(text))
            })
        });
        in_changes
    })
}

#[test]
fn an_erased_task_is_in_no_change_the_store_keeps() {
    let mut store = Store::open_in_memory().unwrap();
    let secret = add(&mut store, "the secret");
    add(&mut store, "the groceries");
    rename(&mut store, &secret, "the secret, renamed");
    assert!(history_holds(&mut store, "the secret"), "before, the history holds it");

    erase(&mut store, &secret);
    assert_eq!(titles(&mut store), ["the groceries"]);
    assert!(!history_holds(&mut store, "the secret"));
    assert!(history_holds(&mut store, "the groceries"), "what stays keeps its place");
    assert!(store.generation().is_some());
}

#[test]
fn erasing_takes_the_undo_history_with_it() {
    let mut store = Store::open_in_memory().unwrap();
    let secret = add(&mut store, "the secret");
    add(&mut store, "the groceries");
    erase(&mut store, &secret);
    assert_eq!(store.undo_depths().unwrap(), (0, 0));
}

#[test]
fn another_device_takes_in_the_erasure_and_keeps_what_it_did_meanwhile() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let secret = add(&mut phone, "the secret");
    let groceries = add(&mut phone, "the groceries");
    sync(&mut phone, &mut laptop);
    assert_eq!(titles(&mut laptop), ["the groceries", "the secret"]);

    // Apart: the phone erases; the laptop renames one task and adds another.
    erase(&mut phone, &secret);
    rename(&mut laptop, &groceries, "the groceries for Sunday");
    add(&mut laptop, "the laundry");

    sync(&mut phone, &mut laptop);
    assert_eq!(laptop.generation(), phone.generation());
    assert!(!history_holds(&mut laptop, "the secret"));
    // What the laptop did comes back to the phone on the next sync, in the new history.
    sync(&mut phone, &mut laptop);
    for store in [&mut phone, &mut laptop] {
        assert_eq!(titles(store), ["the groceries for Sunday", "the laundry"]);
    }
}

#[test]
fn an_edit_made_elsewhere_to_an_erased_task_does_not_bring_it_back() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let secret = add(&mut phone, "the secret");
    sync(&mut phone, &mut laptop);

    erase(&mut phone, &secret);
    rename(&mut laptop, &secret, "the secret, changed meanwhile");

    sync(&mut phone, &mut laptop);
    sync(&mut phone, &mut laptop);
    for store in [&mut phone, &mut laptop] {
        assert!(titles(store).is_empty(), "{:?}", titles(store));
        assert!(!history_holds(store, "the secret"));
    }
}

#[test]
fn two_devices_erasing_at_once_both_keep_both_erasures() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let first = add(&mut phone, "the first secret");
    let second = add(&mut phone, "the second secret");
    add(&mut phone, "the groceries");
    sync(&mut phone, &mut laptop);

    erase(&mut phone, &first);
    erase(&mut laptop, &second);
    sync(&mut phone, &mut laptop);
    sync(&mut phone, &mut laptop);

    assert_eq!(phone.generation(), laptop.generation());
    for store in [&mut phone, &mut laptop] {
        assert_eq!(titles(store), ["the groceries"]);
        assert!(!history_holds(store, "secret"));
    }
}

#[test]
fn a_backup_from_before_an_erasure_is_refused_with_why() {
    let mut store = Store::open_in_memory().unwrap();
    let secret = add(&mut store, "the secret");
    let before = store.backup().unwrap();
    erase(&mut store, &secret);
    let refused = store.restore(&before).unwrap_err();
    assert!(refused.to_string().contains("bring them back"), "{refused}");
    assert!(titles(&mut store).is_empty());
}

#[test]
fn an_older_generation_s_changes_are_left_alone() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let secret = add(&mut phone, "the secret");
    sync(&mut phone, &mut laptop);
    // The laptop's changes from the first history, as a watch's parcel would carry them.
    rename(&mut laptop, &secret, "the secret, again");
    let old: Vec<(SyncName, Vec<Vec<u8>>)> = laptop
        .sync_heads()
        .unwrap()
        .into_iter()
        .map(|(name, _)| {
            let changes = laptop.changes_since(&name, &[]).unwrap();
            (name, changes)
        })
        .collect();

    erase(&mut phone, &secret);
    for (name, changes) in &old {
        assert!(!phone.take_changes(name, changes).unwrap());
    }
    assert!(titles(&mut phone).is_empty());
}

#[test]
fn another_process_holding_the_file_reads_the_new_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let mut app = Store::open(&path).unwrap();
    let mut daemon = Store::open(&path).unwrap();
    let secret = add(&mut app, "the secret");
    add(&mut app, "the groceries");
    daemon.refresh().unwrap();
    assert_eq!(titles(&mut daemon), ["the groceries", "the secret"]);

    erase(&mut app, &secret);
    assert!(daemon.refresh().unwrap());
    assert_eq!(daemon.generation(), app.generation());
    assert_eq!(titles(&mut daemon), ["the groceries"]);
    // And what it writes after lands in the new history.
    add(&mut daemon, "the laundry");
    app.refresh().unwrap();
    assert_eq!(titles(&mut app), ["the groceries", "the laundry"]);
}

#[test]
fn a_write_from_the_history_before_is_refused_not_mixed_in() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let mut app = Store::open(&path).unwrap();
    let mut daemon = Store::open(&path).unwrap();
    let secret = add(&mut app, "the secret");
    daemon.refresh().unwrap();
    erase(&mut app, &secret);
    // The daemon writes before it has refreshed.
    let task = Task::new(ProjectId::INBOX, "the laundry", OrderKey::middle());
    let refused = daemon.apply(&create_task(task)).unwrap_err();
    assert!(matches!(refused, lumenna_store::StoreError::Rebuilt), "{refused}");
}

#[test]
fn erasing_keeps_how_often_a_task_was_put_off() {
    use jiff::civil::date;
    let mut store = Store::open_in_memory().unwrap();
    let secret = add(&mut store, "the secret");
    let essay = add(&mut store, "the essay");
    for day in [10, 12, 15] {
        let before = store.snapshot().0.tasks[&essay.id].clone();
        let after = Task { due: Some(lumenna_core::model::Due::on(date(2026, 5, day))), ..before.clone() };
        store.apply_recorded(&update_task(before, after)).unwrap();
    }
    let put_off = store.snapshot().0.postponed.get(&essay.id).copied();
    assert_eq!(put_off, Some(2));
    erase(&mut store, &secret);
    assert_eq!(store.snapshot().0.postponed.get(&essay.id).copied(), put_off);
}

#[test]
fn a_device_that_missed_the_rebuild_takes_it_in_from_one_that_has_it() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    let secret = add(&mut phone, "the secret");
    sync(&mut phone, &mut laptop);
    sync(&mut laptop, &mut tablet);
    // The phone renames before it erases; the tablet never hears of either from it.
    let groceries = add(&mut phone, "the groceries");
    sync(&mut phone, &mut laptop);
    erase(&mut phone, &secret);
    sync(&mut phone, &mut laptop);
    // The tablet, behind the rebuild, renames the secret and adds a task of its own.
    rename(&mut tablet, &secret, "the secret, from the tablet");
    add(&mut tablet, "the laundry");

    sync(&mut laptop, &mut tablet);
    sync(&mut laptop, &mut tablet);
    sync(&mut phone, &mut laptop);
    let _ = groceries;
    for store in [&mut phone, &mut laptop, &mut tablet] {
        assert_eq!(titles(store), ["the groceries", "the laundry"]);
        assert!(!history_holds(store, "the secret"));
    }
}
