//! Pairing and sync over real Iroh endpoints, on this machine and nothing else.
//!
//! `Network::LocalOnly` binds with no relay and no lookup service, and each test hands the
//! other side an address directly, so these run offline and touch no outside server.

use std::sync::{Arc, Mutex};

use lumenna_core::ProjectId;
use lumenna_core::model::Task;
use lumenna_core::order::OrderKey;
use lumenna_store::Store;
use lumenna_sync::invite::{Invitation, identity};
use lumenna_sync::node::{Network, Node};
use lumenna_sync::{SharedStore, SyncError};

fn store() -> SharedStore {
    Arc::new(Mutex::new(Store::open_in_memory().unwrap()))
}

fn add(store: &SharedStore, title: &str) -> Task {
    let task = Task::new(ProjectId::INBOX, title, OrderKey::middle());
    store.lock().unwrap().write(|docs| docs.put_task(&task, None)).unwrap();
    task
}

fn has(store: &SharedStore, task: &Task) -> bool {
    store.lock().unwrap().snapshot().0.tasks.contains_key(&task.id)
}

/// Pairs two stores, each person answering `yes_a` and `yes_b` to the words.
async fn pair(
    a: &SharedStore,
    b: &SharedStore,
    yes_a: bool,
    yes_b: bool,
) -> (lumenna_sync::Result<lumenna_sync::invite::Paired>, lumenna_sync::Result<lumenna_sync::invite::Paired>, Vec<String>, Vec<String>) {
    let waiting = Invitation::open(Network::LocalOnly, true).await.unwrap();
    let joining = Invitation::open(Network::LocalOnly, false).await.unwrap();
    let address = waiting.addr();
    let (a_store, b_store) = (a.clone(), b.clone());
    let (seen_a, seen_b) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
    let (words_a, words_b) = (seen_a.clone(), seen_b.clone());

    let left = tokio::spawn(async move {
        let (conn, role) = waiting.wait().await.unwrap();
        let me = identity(&a_store, "laptop", "linux").unwrap();
        let result = waiting
            .pair(&conn, role, &a_store, me, |words| async move {
                *words_a.lock().unwrap() = words;
                yes_a
            })
            .await;
        waiting.close().await;
        result
    });
    let right = tokio::spawn(async move {
        let (conn, role) = joining.meet(Some(address)).await.unwrap();
        let me = identity(&b_store, "phone", "android").unwrap();
        let result = joining
            .pair(&conn, role, &b_store, me, |words| async move {
                *words_b.lock().unwrap() = words;
                yes_b
            })
            .await;
        joining.close().await;
        result
    });
    let (left, right) = (left.await.unwrap(), right.await.unwrap());
    let (a_words, b_words) = (seen_a.lock().unwrap().clone(), seen_b.lock().unwrap().clone());
    (left, right, a_words, b_words)
}

#[tokio::test]
async fn pairing_shows_the_same_words_enrolls_both_and_syncs_at_once() {
    let (a, b) = (store(), store());
    let from_a = add(&a, "written on the laptop");
    let from_b = add(&b, "written on the phone");

    let (left, right, words_a, words_b) = pair(&a, &b, true, true).await;
    let (left, right) = (left.unwrap(), right.unwrap());
    assert_eq!(words_a, words_b, "the person compares these");
    assert_eq!(words_a.len(), 3);
    assert_eq!(left.peer.name, "phone");
    assert_eq!(right.peer.name, "laptop");

    for side in [&a, &b] {
        let devices = side.lock().unwrap().snapshot().0.devices;
        let mut names: Vec<_> = devices.values().map(|d| d.name.clone()).collect();
        names.sort();
        assert_eq!(names, ["laptop", "phone"]);
        assert!(has(side, &from_a) && has(side, &from_b), "the first sync ran");
    }
}

#[tokio::test]
async fn a_no_on_either_device_pairs_nothing() {
    let (a, b) = (store(), store());
    let (left, right, _, _) = pair(&a, &b, true, false).await;
    assert!(matches!(left, Err(SyncError::NotPaired(_))), "{left:?}");
    assert!(matches!(right, Err(SyncError::NotPaired(_))), "{right:?}");
    for side in [&a, &b] {
        assert!(side.lock().unwrap().snapshot().0.devices.is_empty());
    }
}

#[tokio::test]
async fn paired_devices_sync_later_through_their_device_keys() {
    let (a, b) = (store(), store());
    let (left, _, _, _) = pair(&a, &b, true, true).await;
    left.unwrap();

    let node_a = Node::bind(a.clone(), Network::LocalOnly).await.unwrap();
    let node_b = Node::bind(b.clone(), Network::LocalOnly).await.unwrap();
    let serving = node_a.clone();
    tokio::spawn(async move { serving.serve().await });

    let later = add(&b, "written after pairing");
    let summary = node_b.sync_with(node_a.addr()).await.unwrap();
    assert!(summary.documents >= 2);
    assert!(has(&a, &later), "it reached the laptop");

    node_a.close().await;
    node_b.close().await;
}

#[tokio::test]
async fn a_device_that_was_never_paired_is_refused() {
    let (a, stranger) = (store(), store());
    let node_a = Node::bind(a.clone(), Network::LocalOnly).await.unwrap();
    let node_s = Node::bind(stranger.clone(), Network::LocalOnly).await.unwrap();
    let serving = node_a.clone();
    tokio::spawn(async move { serving.serve().await });

    // The stranger does not list the laptop either, so it will not even dial.
    let refused = node_s.sync_with(node_a.addr()).await;
    assert!(matches!(refused, Err(SyncError::Refused(_))), "{refused:?}");

    // Even with the laptop forced into its own roster, the laptop's roster is what decides.
    let laptop = lumenna_core::model::Device {
        node_id: node_a.id(),
        name: "laptop".to_owned(),
        platform: "linux".to_owned(),
        paired_at: lumenna_core::time::now(),
        last_seen: lumenna_core::time::now(),
        schema: 1,
    };
    stranger.lock().unwrap().write(|docs| docs.put_device(&laptop, None)).unwrap();
    let secret = add(&stranger, "planted by a stranger");
    assert!(node_s.sync_with(node_a.addr()).await.is_err());
    assert!(!has(&a, &secret), "nothing from an unpaired device was taken in");

    node_a.close().await;
    node_s.close().await;
}
