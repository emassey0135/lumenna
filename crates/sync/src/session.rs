//! The sync session: every document, reconciled with Automerge's sync protocol.
//!
//! Both sides run the same code; nothing here knows who dialled. A session is:
//!
//! 1. **Hello.** Each side sends the documents it holds. A document either side holds is
//!    synced, so a year this device has never opened arrives from a peer that has — its
//!    genesis is deterministic, so starting it from nothing here forks nothing.
//! 2. **Rounds, one document at a time, in name order.** In each round both sides send one
//!    frame — the next Automerge sync message, or an empty frame for "nothing to say" — and
//!    then read the other's. When both frames of a round were empty, the document is done.
//!    Both sides see the same two frames, so both stop on the same round without any further
//!    signal.
//!
//! Sending before reading is what keeps the lockstep from deadlocking: each side's frame is
//! already in flight when it starts waiting for the other's.
//!
//! Sync state is per session, not saved. Automerge's protocol converges from a fresh state in
//! a few round trips by exchanging heads and Bloom filters, and the document sets here are
//! small; saving per-peer state would buy a round trip and cost a table of state that goes
//! stale whenever a peer compacts.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use lumenna_store::{Adopted, DocId, SyncName, SyncState};

use crate::error::{Result, SyncError};
use crate::framing::{read_frame, write_frame};
use crate::{SharedStore, lock};

/// The session protocol's version. A peer speaking another is refused with a message saying
/// which side needs updating, rather than misread.
const PROTOCOL: u32 = 0;

/// The same protocol, said by a device whose store has been erased for good (a newer
/// generation, `lumenna_store::SyncName`), so that a build from before generations, which
/// would sync its documents' frames into the newer ones', refuses rather than mixes the two
/// histories. This build speaks both.
const ERASED: u32 = 1;

/// The most rounds one document may take. Automerge converges in a handful; this is a guard
/// against a peer that never stops talking, not a tuning parameter.
const MAX_ROUNDS: usize = 1_000;

#[derive(Serialize, Deserialize)]
struct Hello {
    protocol: u32,
    documents: Vec<String>,
}

/// What a session did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// The documents reconciled.
    pub documents: usize,
    /// The documents that changed on this side.
    pub changed: Vec<DocId>,
    /// The newer generation of the store this side took in, when the other had erased for
    /// good.
    pub adopted: Option<Adopted>,
}

/// Runs one sync session over a stream.
///
/// Call it on both sides of a connection. The store should already hold whatever other
/// processes on this device have written — `Store::refresh` — so that it offers the peer
/// everything this device has.
///
/// # Errors
///
/// If the stream fails, the peer speaks another protocol version, or the store refuses what
/// arrives.
pub async fn run<R, W>(store: &SharedStore, reader: &mut R, writer: &mut W) -> Result<Summary>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mine: Vec<SyncName> = lock(store)?.sync_documents()?;
    let erased = mine.iter().any(|name| name.generation.is_some());
    let hello = Hello {
        protocol: if erased { ERASED } else { PROTOCOL },
        documents: mine.iter().map(SyncName::name).collect(),
    };
    let encoded = serde_json::to_vec(&hello)
        .map_err(|e| SyncError::Protocol(format!("could not write a hello: {e}")))?;
    write_frame(writer, &encoded).await?;

    let theirs: Hello = serde_json::from_slice(&read_frame(reader).await?)
        .map_err(|e| SyncError::Protocol(format!("a hello that does not read: {e}")))?;
    if theirs.protocol > ERASED {
        return Err(SyncError::Protocol(format!(
            "it speaks sync protocol {} and this device speaks {ERASED}; update the older one",
            theirs.protocol
        )));
    }

    // Names this version does not know are left alone rather than created: a newer version's
    // kind of document is that version's business. Only the newest generation either side
    // holds is synced, which both sides work out alike: an older one's history must never
    // reach it, and the side holding the older takes the newer in.
    let mut documents = mine;
    documents.extend(theirs.documents.iter().filter_map(|name| SyncName::parse(name)));
    let newest = documents.iter().map(|name| name.generation.clone()).max().flatten();
    documents.retain(|name| name.generation == newest);
    documents.sort();
    documents.dedup();

    let mut summary = Summary { documents: documents.len(), ..Summary::default() };
    for name in documents {
        match sync_document(store, &name, reader, writer).await {
            Ok(true) => summary.changed.push(name.id),
            Ok(false) => {}
            Err(error) => {
                lock(store)?.discard_adoption();
                return Err(error);
            }
        }
    }
    let mut store = lock(store)?;
    summary.adopted = store.finish_adoption()?;
    // Taking a newer generation in changes every document there is.
    if summary.adopted.is_some() {
        summary.changed = store.sync_documents()?.into_iter().map(|name| name.id).collect();
    }
    Ok(summary)
}

async fn sync_document<R, W>(
    store: &SharedStore,
    name: &SyncName,
    reader: &mut R,
    writer: &mut W,
) -> Result<bool>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut state = SyncState::new();
    let mut changed = false;
    for _ in 0..MAX_ROUNDS {
        let outgoing = lock(store)?.sync_message(name, &mut state)?;
        write_frame(writer, outgoing.as_deref().unwrap_or_default()).await?;
        let incoming = read_frame(reader).await?;

        if !incoming.is_empty() {
            changed |= lock(store)?.receive_sync_message(name, &mut state, &incoming)?;
        }
        if outgoing.is_none() && incoming.is_empty() {
            return Ok(changed);
        }
    }
    Err(SyncError::Protocol(format!("{} did not settle", name.id.name())))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lumenna_core::ProjectId;
    use lumenna_core::model::{BlockKind, BlockSeries, Task};
    use lumenna_core::order::OrderKey;
    use lumenna_store::Store;

    use super::*;

    fn store() -> SharedStore {
        Arc::new(Mutex::new(Store::open_in_memory().unwrap()))
    }

    async fn sync(a: &SharedStore, b: &SharedStore) -> (Summary, Summary) {
        let (mut a_end, mut b_end) = tokio::io::duplex(1 << 16);
        let (a_store, b_store) = (a.clone(), b.clone());
        let left = tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut a_end);
            run(&a_store, &mut r, &mut w).await
        });
        let right = tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut b_end);
            run(&b_store, &mut r, &mut w).await
        });
        (left.await.unwrap().unwrap(), right.await.unwrap().unwrap())
    }

    fn add(store: &SharedStore, title: &str) -> Task {
        let task = Task::new(ProjectId::INBOX, title, OrderKey::middle());
        store.lock().unwrap().write(|docs| docs.put_task(&task, None)).unwrap();
        task
    }

    #[tokio::test]
    async fn two_stores_converge_in_both_directions() {
        let (a, b) = (store(), store());
        let from_a = add(&a, "written on the laptop");
        let from_b = add(&b, "written on the phone");

        let (left, right) = sync(&a, &b).await;
        assert!(left.changed.contains(&DocId::Core));
        assert!(right.changed.contains(&DocId::Core));

        for side in [&a, &b] {
            let tasks = side.lock().unwrap().snapshot().0.tasks;
            assert!(tasks.contains_key(&from_a.id) && tasks.contains_key(&from_b.id));
        }
        let inboxes = |s: &SharedStore| {
            s.lock().unwrap().snapshot().0.projects.values().filter(|p| p.is_inbox).count()
        };
        assert_eq!((inboxes(&a), inboxes(&b)), (1, 1), "one Inbox, not two");
    }

    #[tokio::test]
    async fn a_year_only_one_side_has_arrives_on_the_other() {
        let (a, b) = (store(), store());
        let block = BlockSeries::one_off(
            "Retreat",
            BlockKind::Work,
            jiff::civil::date(2029, 4, 1),
            jiff::civil::time(9, 0, 0, 0),
            60,
        )
        .unwrap();
        a.lock().unwrap().write(|docs| docs.put_series(&block, None)).unwrap();

        let (_, right) = sync(&a, &b).await;
        assert!(right.changed.contains(&DocId::Blocks(2029)), "{right:?}");
        let mut b_store = b.lock().unwrap();
        b_store.load_year(2029).unwrap();
        assert!(b_store.snapshot().0.series.contains_key(&block.id));
    }

    #[tokio::test]
    async fn syncing_again_changes_nothing() {
        let (a, b) = (store(), store());
        add(&a, "once");
        sync(&a, &b).await;
        let (left, right) = sync(&a, &b).await;
        assert!(left.changed.is_empty() && right.changed.is_empty());
    }

    #[tokio::test]
    async fn what_arrives_is_written_to_disk_for_every_process_to_see() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profile.sqlite");
        let a = store();
        let b: SharedStore = Arc::new(Mutex::new(Store::open(&path).unwrap()));
        let task = add(&a, "arrived by sync");
        sync(&a, &b).await;

        let mut other_process = Store::open(&path).unwrap();
        assert!(other_process.snapshot().0.tasks.contains_key(&task.id));
    }

    #[tokio::test]
    async fn a_store_erased_for_good_is_taken_in_by_the_other_side_whole() {
        let (a, b) = (store(), store());
        let secret = add(&a, "the secret");
        add(&a, "the groceries");
        sync(&a, &b).await;
        let edit = lumenna_core::edit::purge_task(&a.lock().unwrap().snapshot().0, secret.id).unwrap();
        a.lock().unwrap().erase(lumenna_store::Documents::erased_by(&edit)).unwrap();
        add(&b, "the laundry");

        let (left, right) = sync(&a, &b).await;
        assert!(left.adopted.is_none());
        assert!(right.adopted.is_some(), "{right:?}");
        sync(&a, &b).await;
        let generation = a.lock().unwrap().generation().map(str::to_owned);
        assert!(generation.is_some());
        for side in [&a, &b] {
            let mut side = side.lock().unwrap();
            assert_eq!(side.generation(), generation.as_deref());
            let mut titles: Vec<String> = side.snapshot().0.tasks.values().map(|t| t.title.clone()).collect();
            titles.sort();
            assert_eq!(titles, ["the groceries", "the laundry"]);
        }
    }

    #[tokio::test]
    async fn a_store_erased_for_good_says_so_where_a_build_from_before_would_listen() {
        let a = store();
        let secret = add(&a, "the secret");
        let edit = lumenna_core::edit::purge_task(&a.lock().unwrap().snapshot().0, secret.id).unwrap();
        a.lock().unwrap().erase(lumenna_store::Documents::erased_by(&edit)).unwrap();
        // What a build from before generations reads first: a protocol it refuses.
        let (mut a_end, mut other_end) = tokio::io::duplex(1 << 16);
        let a_store = a.clone();
        tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut a_end);
            let _ = run(&a_store, &mut r, &mut w).await;
        });
        let (mut r, _w) = tokio::io::split(&mut other_end);
        let hello: Hello = serde_json::from_slice(&read_frame(&mut r).await.unwrap()).unwrap();
        assert_ne!(hello.protocol, PROTOCOL);
        assert!(hello.documents.iter().all(|name| name.contains('@')), "{:?}", hello.documents);
    }
}
