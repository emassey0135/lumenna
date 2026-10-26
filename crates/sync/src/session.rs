//! The sync session: every document, reconciled with Automerge's sync protocol.
//!
//! Both sides run the same code; nothing here knows who dialled. A session is:
//!
//! 1. **Hello.** Each side sends the documents it holds, and each one's heads. A document
//!    either side holds is synced, so a year this device has never opened arrives from a peer
//!    that has — its genesis is deterministic, so starting it from nothing here forks nothing —
//!    unless both hold it at the same heads, which says they hold the same changes: it is left
//!    out, so a year neither side needs anything in is never loaded (`Store::sync_heads`
//!    answers without loading it), which is what keeps a watch's memory to the years it shows.
//! 2. **Rounds, every document at once** (`crate::rounds`). In each round both sides send one
//!    frame — the next Automerge sync message for every document that has one, or an empty
//!    frame for "nothing to say" — and then read the other's. When both frames of a round were
//!    empty, the session is done. Both sides see the same two frames, so both stop on the same
//!    round without any further signal. One round trip moves every document a step, so a
//!    session over a relay costs the round trips of its slowest document, not of them all.
//!
//! Sending before reading is what keeps the lockstep from deadlocking: each side's frame is
//! already in flight when it starts waiting for the other's.
//!
//! Sync state is per session, not saved. Automerge's protocol converges from a fresh state in
//! a few round trips by exchanging heads and Bloom filters, and the document sets here are
//! small; saving per-peer state would buy a round trip and cost a table of state that goes
//! stale whenever a peer compacts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use lumenna_store::{Adopted, DocId, SyncName};

use crate::error::{Result, SyncError};
use crate::framing::{read_frame, write_frame};
use crate::rounds::{Reader, Rounds, put_said};
use crate::{SharedStore, lock};

/// The session protocol's version. A peer speaking another is refused with a message saying
/// which side needs updating, rather than misread.
///
/// 0 was a round per document, and 1 the same said by a store erased for good, so that a build
/// from before generations refused it. Every build that speaks 2 knows generations
/// (`lumenna_store::SyncName`), so an erased store says 2 like any other.
const PROTOCOL: u32 = 2;

/// The most rounds one session may take. Automerge converges in a handful; this is a guard
/// against a peer that never stops talking, not a tuning parameter.
const MAX_ROUNDS: usize = 1_000;

#[derive(Serialize, Deserialize)]
struct Hello {
    protocol: u32,
    documents: Vec<String>,
    /// Each document's heads, by name: what a build from before them leaves out, and then
    /// every document is synced, as it was.
    #[serde(default)]
    heads: BTreeMap<String, Vec<String>>,
}

/// What a session did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// The documents reconciled: those the two sides did not already hold alike.
    pub documents: usize,
    /// The documents that changed on this side.
    pub changed: Vec<DocId>,
    /// The rounds it took, the last of them the one in which neither side had anything to say.
    pub rounds: usize,
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
    let held = lock(store)?.sync_heads()?;
    let heads: BTreeMap<String, Vec<String>> =
        held.iter().map(|(name, heads)| (name.name(), heads.iter().map(ToString::to_string).collect())).collect();
    let mine: Vec<SyncName> = held.into_iter().map(|(name, _)| name).collect();
    let hello = Hello { protocol: PROTOCOL, documents: mine.iter().map(SyncName::name).collect(), heads };
    let encoded = serde_json::to_vec(&hello).map_err(|e| SyncError::Local(format!("could not write a hello: {e}")))?;
    write_frame(writer, &encoded).await?;

    let theirs: Hello = serde_json::from_slice(&read_frame(reader).await?)
        .map_err(|e| SyncError::Protocol(format!("a hello that does not read: {e}")))?;
    if theirs.protocol != PROTOCOL {
        return Err(SyncError::Protocol(format!(
            "it speaks sync protocol {} and this device speaks {PROTOCOL}; update the older one",
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
    // Both at the same heads: the same changes, so nothing to say either way.
    let alike = |name: &SyncName| {
        let name = name.name();
        matches!((hello.heads.get(&name), theirs.heads.get(&name)), (Some(a), Some(b)) if a == b)
    };
    documents.retain(|name| !alike(name));
    let documents: BTreeSet<SyncName> = documents.into_iter().collect();

    // A newer generation being taken in goes with `rounds`: a session that fails or is
    // cancelled drops what it held aside, and no other session sees it.
    let mut rounds = Rounds::default();
    let mut changed = BTreeSet::new();
    let count = reconcile(store, &mut rounds, &documents, &mut changed, reader, writer).await?;
    let mut summary =
        Summary { documents: documents.len(), changed: changed.into_iter().collect(), rounds: count, adopted: None };
    let mut store = lock(store)?;
    summary.adopted = rounds.adopt(&mut store, &documents)?;
    // Taking a newer generation in changes every document there is.
    if summary.adopted.is_some() {
        summary.changed = store.sync_documents()?.into_iter().map(|name| name.id).collect();
    }
    Ok(summary)
}

/// The rounds, until a round in which neither side had anything to say. Returns how many it took.
async fn reconcile<R, W>(
    store: &SharedStore,
    rounds: &mut Rounds,
    documents: &BTreeSet<SyncName>,
    changed: &mut BTreeSet<DocId>,
    reader: &mut R,
    writer: &mut W,
) -> Result<usize>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    for round in 0..MAX_ROUNDS {
        let said = rounds.say(&mut *lock(store)?, documents)?;
        let mut outgoing = Vec::new();
        if !said.is_empty() {
            put_said(&mut outgoing, &said);
        }
        write_frame(writer, &outgoing).await?;

        let incoming = read_frame(reader).await?;
        let heard = if incoming.is_empty() {
            Vec::new()
        } else {
            let mut reading = Reader::new(&incoming);
            let heard = reading.said()?;
            reading.end()?;
            heard
        };
        // Only the documents both sides agreed on: a message for any other, such as an older
        // generation's, must never reach this store.
        changed.extend(rounds.hear(&mut *lock(store)?, &heard, |name| documents.contains(name))?);
        if said.is_empty() && heard.is_empty() {
            return Ok(round + 1);
        }
    }
    Err(SyncError::Protocol("its sync never settled".to_owned()))
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
        // What a build from before generations reads first: a protocol it refuses, since it
        // took nothing above 1.
        let (mut a_end, mut other_end) = tokio::io::duplex(1 << 16);
        let a_store = a.clone();
        tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut a_end);
            let _ = run(&a_store, &mut r, &mut w).await;
        });
        let (mut r, _w) = tokio::io::split(&mut other_end);
        let hello: Hello = serde_json::from_slice(&read_frame(&mut r).await.unwrap()).unwrap();
        assert!(hello.protocol > 1, "{}", hello.protocol);
        assert!(hello.documents.iter().all(|name| name.contains('@')), "{:?}", hello.documents);
    }

    #[tokio::test]
    async fn a_peer_speaking_a_round_per_document_is_told_to_update() {
        let a = store();
        let (mut a_end, mut other_end) = tokio::io::duplex(1 << 16);
        let a_store = a.clone();
        let session = tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut a_end);
            run(&a_store, &mut r, &mut w).await
        });
        let (mut r, mut w) = tokio::io::split(&mut other_end);
        let older = serde_json::to_vec(&Hello { protocol: 0, documents: vec!["core".to_owned()], heads: BTreeMap::new() }).unwrap();
        write_frame(&mut w, &older).await.unwrap();
        read_frame(&mut r).await.unwrap();
        let error = session.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("update the older one"), "{error}");
    }

    #[tokio::test]
    async fn a_sync_between_two_stores_already_alike_loads_no_year() {
        let dir = tempfile::tempdir().unwrap();
        let (a_path, b_path) = (dir.path().join("a.sqlite"), dir.path().join("b.sqlite"));
        {
            let a: SharedStore = Arc::new(Mutex::new(Store::open(&a_path).unwrap()));
            let b: SharedStore = Arc::new(Mutex::new(Store::open(&b_path).unwrap()));
            add(&a, "in the core");
            for year in [2027, 2028] {
                let block = BlockSeries::one_off(
                    "Retreat",
                    BlockKind::Work,
                    jiff::civil::date(year, 4, 1),
                    jiff::civil::time(9, 0, 0, 0),
                    60,
                )
                .unwrap();
                a.lock().unwrap().write(|docs| docs.put_series(&block, None)).unwrap();
            }
            sync(&a, &b).await;
        }
        // Each opened afresh, as a watch's app is: no year in memory.
        let a: SharedStore = Arc::new(Mutex::new(Store::open(&a_path).unwrap()));
        let b: SharedStore = Arc::new(Mutex::new(Store::open(&b_path).unwrap()));
        let (left, right) = sync(&a, &b).await;
        assert!(left.changed.is_empty() && right.changed.is_empty());
        for side in [&a, &b] {
            assert!(side.lock().unwrap().documents().loaded_years().is_empty(), "no year was loaded to sync");
        }

        // A change to one year loads that year on both, and only it.
        let block = BlockSeries::one_off(
            "Retreat again",
            BlockKind::Work,
            jiff::civil::date(2028, 6, 1),
            jiff::civil::time(9, 0, 0, 0),
            60,
        )
        .unwrap();
        a.lock().unwrap().write(|docs| docs.put_series(&block, None)).unwrap();
        let (_, right) = sync(&a, &b).await;
        assert_eq!(right.changed, [DocId::Blocks(2028)]);
        assert_eq!(b.lock().unwrap().documents().loaded_years(), [2028]);
    }

    #[tokio::test]
    async fn a_session_cut_short_leaves_nothing_of_a_newer_generation_for_the_next() {
        let (a, b, c) = (store(), store(), store());
        let secret = add(&a, "the secret");
        add(&a, "the groceries");
        sync(&a, &b).await;
        sync(&a, &c).await;
        let edit = lumenna_core::edit::purge_task(&b.lock().unwrap().snapshot().0, secret.id).unwrap();
        b.lock().unwrap().erase(lumenna_store::Documents::erased_by(&edit)).unwrap();

        // A session with the erased store that hears its core and then is gone, as one cut off by
        // a time limit: what it held aside goes with it.
        {
            let newer: BTreeSet<SyncName> =
                b.lock().unwrap().sync_documents().unwrap().into_iter().filter(|n| n.id == DocId::Core).collect();
            let (mut on_a, mut on_b) = (Rounds::default(), Rounds::default());
            for _ in 0..4 {
                let from_a = on_a.say(&mut a.lock().unwrap(), &newer).unwrap();
                let from_b = on_b.say(&mut b.lock().unwrap(), &newer).unwrap();
                on_b.hear(&mut b.lock().unwrap(), &from_a, |_| true).unwrap();
                on_a.hear(&mut a.lock().unwrap(), &from_b, |_| true).unwrap();
            }
        }
        // The next session, with a store of the same history as this one's, takes nothing in.
        let (left, _) = sync(&a, &c).await;
        assert!(left.adopted.is_none(), "{left:?}");
        assert_eq!(a.lock().unwrap().generation(), None);
        let mut titles: Vec<String> = a.lock().unwrap().snapshot().0.tasks.values().map(|t| t.title.clone()).collect();
        titles.sort();
        assert_eq!(titles, ["the groceries", "the secret"]);
    }

    #[tokio::test]
    async fn every_document_moves_in_the_same_round_trips() {
        let (a, b) = (store(), store());
        add(&a, "in the core");
        for year in 2027..2032 {
            let block = BlockSeries::one_off(
                "Retreat",
                BlockKind::Work,
                jiff::civil::date(year, 4, 1),
                jiff::civil::time(9, 0, 0, 0),
                60,
            )
            .unwrap();
            a.lock().unwrap().write(|docs| docs.put_series(&block, None)).unwrap();
        }

        let (left, right) = sync(&a, &b).await;
        // A round per document could never take fewer rounds than there are documents.
        assert!(left.rounds < left.documents, "{} rounds for {} documents", left.rounds, left.documents);
        assert_eq!(left.rounds, right.rounds, "both sides stop on the same round");
        for year in 2027..2032 {
            assert!(right.changed.contains(&DocId::Blocks(year)), "{right:?}");
        }
        assert!(right.changed.contains(&DocId::Core), "{right:?}");
    }
}
