//! Real erasure: the store's history begun again without what was erased, which every device
//! takes in, keeping what it did that the rebuild never saw, and which nothing older can
//! bring back.

use lumenna_core::ProjectId;
use lumenna_core::edit::{create_task, purge_task, update_task};
use lumenna_core::model::Task;
use lumenna_core::order::OrderKey;
use lumenna_store::{Adoption, Documents, Store, SyncName, SyncState};

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
    sync_only(a, b, |_| true);
}

/// [`sync`], but only the documents `only` picks are carried, as when a session is cut short:
/// each side still names every document it holds.
fn sync_only(a: &mut Store, b: &mut Store, only: impl Fn(&SyncName) -> bool) {
    let names: Vec<SyncName> = a.sync_documents().unwrap().into_iter().chain(b.sync_documents().unwrap()).collect();
    let newest = names.iter().map(|n| n.generation.clone()).max().unwrap();
    let mut documents: Vec<SyncName> = names.into_iter().filter(|n| n.generation == newest).collect();
    documents.sort();
    documents.dedup();
    let (mut a_adopting, mut b_adopting) = (Adoption::default(), Adoption::default());
    for name in documents.iter().filter(|name| only(name)) {
        let (mut to_b, mut to_a) = (SyncState::new(), SyncState::new());
        for _ in 0..100 {
            let from_a = a.sync_message(name, &mut to_b, &mut a_adopting).unwrap();
            let from_b = b.sync_message(name, &mut to_a, &mut b_adopting).unwrap();
            if from_a.is_none() && from_b.is_none() {
                break;
            }
            if let Some(message) = from_a {
                b.receive_sync_message(name, &mut to_a, &mut b_adopting, &message).unwrap();
            }
            if let Some(message) = from_b {
                a.receive_sync_message(name, &mut to_b, &mut a_adopting, &message).unwrap();
            }
        }
    }
    a.finish_adoption(a_adopting, &documents).unwrap();
    b.finish_adoption(b_adopting, &documents).unwrap();
}

/// Whether any change in any document of `store`'s backup, its whole history, writes `text`.
fn history_holds(store: &mut Store, text: &str) -> bool {
    let backup = lumenna_store::backup::decode(&store.backup().unwrap()).unwrap();
    backup.documents.iter().any(|(_, bytes)| {
        let mut doc = automerge::AutoCommit::load(bytes).unwrap();
        doc.get_changes(&[]).iter().any(|change| {
            change.decode().operations.iter().any(|op| {
                matches!(&op.action, automerge::legacy::OpType::Put(automerge::ScalarValue::Str(s)) if s.contains(text))
            })
        })
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

/// Three devices that share `the shared` and `the old secret`: what every scenario below starts from.
fn three_devices() -> (Store, Store, Store, Task) {
    let (mut a, mut b, mut c) = (Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap());
    add(&mut a, "the shared");
    let secret = add(&mut a, "the old secret");
    sync(&mut a, &mut b);
    sync(&mut a, &mut c);
    (a, b, c, secret)
}

/// Every pair of the three synced, twice over, so each has what any had.
fn sync_all(a: &mut Store, b: &mut Store, c: &mut Store) {
    for _ in 0..2 {
        sync(a, b);
        sync(b, c);
        sync(a, c);
    }
}

#[test]
fn two_devices_taking_in_one_erasure_keep_a_rename_made_before_it_of_a_task_the_eraser_never_had() {
    // Each device's actor is random, and two copies of one record lost one of them about half
    // the time: tried often enough that a loss would show.
    for _ in 0..12 {
        let (mut a, mut b, mut c, secret) = three_devices();
        let task = add(&mut a, "the essay");
        sync(&mut a, &mut c);
        erase(&mut b, &secret);
        rename(&mut a, &task, "the essay, renamed");

        sync(&mut a, &mut b);
        sync(&mut c, &mut b);
        sync_all(&mut a, &mut b, &mut c);
        for store in [&mut a, &mut b, &mut c] {
            assert_eq!(titles(store), ["the essay, renamed", "the shared"]);
        }
    }
}

#[test]
fn two_devices_taking_in_one_erasure_keep_a_rename_made_after_it_of_a_task_the_eraser_never_had() {
    for _ in 0..12 {
        let (mut a, mut b, mut c, secret) = three_devices();
        let task = add(&mut a, "the essay");
        sync(&mut a, &mut c);
        erase(&mut b, &secret);

        sync(&mut a, &mut b);
        sync(&mut c, &mut b);
        rename(&mut a, &task, "the essay, renamed");
        add(&mut c, "the laundry");
        sync_all(&mut a, &mut b, &mut c);
        for store in [&mut a, &mut b, &mut c] {
            assert_eq!(titles(store), ["the essay, renamed", "the laundry", "the shared"]);
        }
    }
}

fn add_block(store: &mut Store, title: &str) -> lumenna_core::model::BlockSeries {
    let block = lumenna_core::model::BlockSeries::one_off(
        title,
        lumenna_core::model::BlockKind::Work,
        jiff::civil::date(2027, 4, 1),
        jiff::civil::time(9, 0, 0, 0),
        60,
    )
    .unwrap();
    store.write(|docs| docs.put_series(&block, None)).unwrap();
    block
}

fn block_titles(store: &mut Store) -> Vec<String> {
    store.load_all_years().unwrap();
    let mut titles: Vec<String> = store.snapshot().0.series.values().map(|s| s.title.clone()).collect();
    titles.sort();
    titles
}

#[test]
fn a_newer_generation_synced_only_in_part_is_not_taken_in_until_it_is_whole() {
    for _ in 0..12 {
        let mut phone = Store::open_in_memory().unwrap();
        let mut laptop = Store::open_in_memory().unwrap();
        let secret = add(&mut phone, "the secret");
        let block = add_block(&mut phone, "Retreat");
        sync(&mut phone, &mut laptop);
        erase(&mut phone, &secret);
        let before = laptop.snapshot().0.series[&block.id].clone();
        let after = lumenna_core::model::BlockSeries { title: "Retreat, moved".to_owned(), ..before.clone() };
        laptop.write(|docs| docs.put_series(&after, Some(&before))).unwrap();

        // Only the core and the devices of the newer generation arrive: the year never does.
        sync_only(&mut phone, &mut laptop, |name| !matches!(name.id, lumenna_store::DocId::Blocks(_)));
        assert_ne!(laptop.generation(), phone.generation(), "not whole, so not taken in");

        sync(&mut phone, &mut laptop);
        sync(&mut phone, &mut laptop);
        assert_eq!(laptop.generation(), phone.generation());
        for store in [&mut phone, &mut laptop] {
            assert_eq!(block_titles(store), ["Retreat, moved"]);
        }
    }
}

#[test]
fn two_sessions_taking_in_one_generation_each_keep_their_own_and_the_second_merges_into_the_first() {
    let mut phone = Store::open_in_memory().unwrap();
    let mut tablet = Store::open_in_memory().unwrap();
    let mut laptop = Store::open_in_memory().unwrap();
    let secret = add(&mut phone, "the secret");
    sync(&mut phone, &mut laptop);
    erase(&mut phone, &secret);
    sync(&mut phone, &mut tablet);
    add(&mut tablet, "written on the tablet since");

    // The laptop hears the newer generation from the phone and the tablet at once, a session
    // each; the phone's session is cut short after the core.
    let names: Vec<SyncName> = phone.sync_documents().unwrap();
    let (mut from_phone, mut from_tablet) = (Adoption::default(), Adoption::default());
    let carry = |laptop: &mut Store, other: &mut Store, adoption: &mut Adoption, name: &SyncName| {
        let (mut to_other, mut to_laptop) = (SyncState::new(), SyncState::new());
        let mut theirs = Adoption::default();
        for _ in 0..100 {
            let mine = laptop.sync_message(name, &mut to_other, adoption).unwrap();
            let back = other.sync_message(name, &mut to_laptop, &mut theirs).unwrap();
            if mine.is_none() && back.is_none() {
                break;
            }
            if let Some(message) = mine {
                other.receive_sync_message(name, &mut to_laptop, &mut theirs, &message).unwrap();
            }
            if let Some(message) = back {
                laptop.receive_sync_message(name, &mut to_other, adoption, &message).unwrap();
            }
        }
    };
    let core = names.iter().find(|n| n.id == lumenna_store::DocId::Core).unwrap();
    carry(&mut laptop, &mut phone, &mut from_phone, core);
    for name in &names {
        carry(&mut laptop, &mut tablet, &mut from_tablet, name);
    }
    assert!(laptop.finish_adoption(from_tablet, &names).unwrap().is_some(), "the whole one is taken in");
    assert!(laptop.finish_adoption(from_phone, &names).unwrap().is_none(), "the part is not, nor thrown into the other");
    assert_eq!(laptop.generation(), phone.generation());
    assert_eq!(titles(&mut laptop), ["written on the tablet since"]);
}

#[test]
fn a_process_holding_the_history_before_cannot_compact_it_back_over_the_erasure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let mut app = Store::open(&path).unwrap();
    let mut daemon = Store::open(&path).unwrap();
    let secret = add(&mut app, "the secret");
    add(&mut app, "the groceries");
    daemon.refresh().unwrap();

    erase(&mut app, &secret);
    let refused = daemon.compact_document(lumenna_store::DocId::Core).unwrap_err();
    assert!(matches!(refused, lumenna_store::StoreError::Rebuilt), "{refused}");
    let mut reopened = Store::open(&path).unwrap();
    assert!(!history_holds(&mut reopened, "the secret"));
    // And the daemon has moved on to the new history, so its next write lands.
    add(&mut daemon, "the laundry");
    app.refresh().unwrap();
    assert_eq!(titles(&mut app), ["the groceries", "the laundry"]);
}

#[test]
fn a_write_refused_as_rebuilt_leaves_the_process_on_the_new_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let mut app = Store::open(&path).unwrap();
    let mut daemon = Store::open(&path).unwrap();
    let secret = add(&mut app, "the secret");
    add(&mut app, "the groceries");
    daemon.refresh().unwrap();
    erase(&mut app, &secret);

    let task = Task::new(ProjectId::INBOX, "the laundry", OrderKey::middle());
    let refused = daemon.apply(&create_task(task.clone())).unwrap_err();
    assert!(matches!(refused, lumenna_store::StoreError::Rebuilt), "{refused}");
    assert_eq!(daemon.generation(), app.generation());
    assert_eq!(titles(&mut daemon), ["the groceries"]);
    // Tried again, it lands, without a refresh in between.
    daemon.apply(&create_task(task)).unwrap();
    app.refresh().unwrap();
    assert_eq!(titles(&mut app), ["the groceries", "the laundry"]);
}

#[test]
fn a_rebuild_is_not_written_over_a_change_another_process_made_after_it_read() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let mut app = Store::open(&path).unwrap();
    add(&mut app, "the groceries");
    let sequence = app.db().sequence().unwrap();
    let mut daemon = Store::open(&path).unwrap();
    add(&mut daemon, "the laundry");

    let mut db = lumenna_store::Db::open(&path).unwrap();
    assert!(!db.replace_generation(None, sequence, "rebuilt", &[], "").unwrap(), "the file moved on since it was read");
    let mut reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.generation(), None);
    assert_eq!(titles(&mut reopened), ["the groceries", "the laundry"]);
}

#[test]
fn an_export_from_before_an_erasure_does_not_bring_back_what_was_erased() {
    let mut store = Store::open_in_memory().unwrap();
    let secret = add(&mut store, "the secret");
    add(&mut store, "the groceries");
    let completion = lumenna_core::model::TaskCompletion::new(secret.id, None);
    store.write(|docs| docs.put_completion(&completion, None)).unwrap();
    let json = lumenna_store::export::export_json(&store.snapshot().0, jiff::Timestamp::now());

    erase(&mut store, &secret);
    let report = store.import(&lumenna_store::export::parse_json(&json).unwrap()).unwrap();
    assert_eq!(titles(&mut store), ["the groceries"]);
    assert!(store.snapshot().0.completions.is_empty(), "nor anything of the erased task's");
    assert_eq!(report.erased, 2, "{report:?}");
    assert_eq!(report.skipped, 2, "{report:?}");
    assert!(!history_holds(&mut store, "the secret"));
}

#[test]
fn a_store_opened_again_still_counts_how_often_a_task_was_put_off() {
    use jiff::civil::date;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profile.sqlite");
    let essay = {
        let mut store = Store::open(&path).unwrap();
        let essay = add(&mut store, "the essay");
        for day in [10, 12] {
            let before = store.snapshot().0.tasks[&essay.id].clone();
            let after = Task { due: Some(lumenna_core::model::Due::on(date(2026, 5, day))), ..before.clone() };
            store.apply_recorded(&update_task(before, after)).unwrap();
        }
        essay
    };
    let mut reopened = Store::open(&path).unwrap();
    assert_eq!(reopened.snapshot().0.postponed.get(&essay.id).copied(), Some(1));
}

#[test]
fn an_erased_task_s_dismissed_default_reminder_goes_with_it_after_the_defaults_change() {
    use lumenna_core::model::{ReminderAck, ReminderAction, ReminderAnchor, ReminderTarget, Trigger};
    let mut store = Store::open_in_memory().unwrap();
    let ten_before = Trigger::before(ReminderAnchor::Due, 10);
    let settings = store.snapshot().0.settings;
    let with_default = lumenna_core::model::Settings { default_task_reminders: vec![ten_before.clone()], ..settings.clone() };
    store.write(|docs| docs.put_settings(&with_default, Some(&settings))).unwrap();
    let secret = add(&mut store, "the secret");
    let groceries = add(&mut store, "the groceries");
    for task in [&secret, &groceries] {
        let key = lumenna_core::remind::default_key(&ReminderTarget::Task(task.id), &ten_before);
        let ack = ReminderAck {
            reminder_id: key,
            occurrence_date: jiff::civil::date(2026, 5, 1),
            action: ReminderAction::Dismissed,
            acked_at: jiff::Timestamp::now(),
            acked_by: lumenna_core::id::NodeId::from_bytes([1; 32]),
            target: None,
        };
        let edit = lumenna_core::edit::acknowledge(&store.snapshot().0, ack, "Dismissed".to_owned());
        store.apply_recorded(&edit).unwrap();
    }
    assert!(store.snapshot().0.acks.values().all(|ack| ack.target.is_some()), "each says what it was for");

    // The default changes, so the old key is no longer one any task's default fires under.
    let thirty_before = lumenna_core::model::Settings {
        default_task_reminders: vec![Trigger::before(ReminderAnchor::Due, 30)],
        ..with_default.clone()
    };
    store.write(|docs| docs.put_settings(&thirty_before, Some(&with_default))).unwrap();
    erase(&mut store, &secret);

    let acks = store.snapshot().0.acks;
    assert_eq!(acks.len(), 1, "{acks:?}");
    assert!(acks.values().all(|ack| ack.target == Some(ReminderTarget::Task(groceries.id))));
}

fn note(store: &mut Store, task: &Task, notes: &str) {
    let before = store.snapshot().0.tasks[&task.id].clone();
    let after = Task { notes: notes.to_owned(), ..before.clone() };
    store.apply_recorded(&update_task(before, after)).unwrap();
}

fn notes_of(store: &mut Store, task: &Task) -> String {
    store.snapshot().0.tasks[&task.id].notes.clone()
}

#[test]
fn two_devices_taking_in_one_erasure_with_notes_edited_apart_keep_both_edits() {
    for _ in 0..12 {
        let (mut a, mut b, mut c, secret) = three_devices();
        let task = add(&mut a, "the essay");
        note(&mut a, &task, "Read the sources.");
        sync(&mut a, &mut c);
        erase(&mut b, &secret);
        // Apart, before either has taken the erasure in: each adds to the notes.
        note(&mut a, &task, "Read the sources. Draft the outline.");
        note(&mut c, &task, "Read the sources twice.");

        sync(&mut a, &mut b);
        sync(&mut c, &mut b);
        sync_all(&mut a, &mut b, &mut c);
        let notes = notes_of(&mut a, &task);
        for store in [&mut b, &mut c] {
            assert_eq!(notes_of(store, &task), notes, "all three agree");
        }
        assert!(notes.contains("Draft the outline.") && notes.contains("twice"), "neither edit lost: {notes:?}");
        assert_eq!(notes.matches("Read the sources").count(), 1, "the shared text once: {notes:?}");
    }
}

#[test]
fn notes_one_device_went_further_with_are_taken_whole_not_doubled() {
    for _ in 0..12 {
        let (mut a, mut b, mut c, secret) = three_devices();
        let task = add(&mut a, "the essay");
        note(&mut a, &task, "Read the sources.");
        sync(&mut a, &mut c);
        erase(&mut b, &secret);
        note(&mut a, &task, "Read the sources. Draft the outline.");

        sync(&mut a, &mut b);
        sync(&mut c, &mut b);
        sync_all(&mut a, &mut b, &mut c);
        for store in [&mut a, &mut b, &mut c] {
            assert_eq!(notes_of(store, &task), "Read the sources. Draft the outline.");
        }
    }
}
