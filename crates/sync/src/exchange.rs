//! The sync session's reconciliation, over messages instead of a stream.
//!
//! For a link that carries whole messages with a reply to each, such as the one between an
//! Apple Watch and its iPhone: the watch cannot use Iroh (watchOS allows no sockets outside
//! an audio session), and WatchConnectivity is a request and its answer, not a byte stream.
//!
//! It is the same Automerge sync protocol as [`session`](crate::session), with every
//! document's round in each message, as a session's frames carry them (`crate::rounds`):
//!
//! 1. The side that starts sends a message marked *fresh*: the documents it holds with each
//!    one's heads, and its first sync message for each. The other side begins a new exchange on
//!    seeing it, dropping whatever the last one was taking in.
//! 2. Each answer takes in what arrived and says the next sync message for every document
//!    either side holds, or nothing for one already settled, nor for one both sides last said
//!    they hold at the same heads: a year neither needs anything in is never loaded.
//! 3. When a message that said nothing is answered by one that says nothing, the two agree.
//!
//! Sync state lasts the exchange, not longer, as in a session.

use std::collections::{BTreeMap, BTreeSet};

use lumenna_store::{Adopted, ChangeHash, DocId, Store, SyncName};

use crate::error::{Result, SyncError};
use crate::rounds::{Reader, Rounds, Said, put_bytes, put_len, put_said};

/// The exchange's message format. Another is refused, saying which side to update.
///
/// 0 named the documents alone; 1 names each with its heads.
const PROTOCOL: u32 = 1;

/// The most messages one side sends in an exchange: Automerge settles in a handful, so this
/// guards against a side that never stops, and is not a tuning parameter.
pub const MAX_ROUNDS: usize = 1_000;

/// One side of an exchange.
#[derive(Default)]
pub struct Exchange {
    rounds: Rounds,
    known: BTreeSet<SyncName>,
    /// Each document's heads as the other side last said it holds it.
    theirs: BTreeMap<SyncName, Vec<ChangeHash>>,
    sent: usize,
    answering: bool,
    sent_nothing: bool,
    heard_nothing: bool,
}

impl std::fmt::Debug for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exchange").field("documents", &self.known).field("sent", &self.sent).finish_non_exhaustive()
    }
}

struct Message {
    fresh: bool,
    documents: Vec<(String, Vec<ChangeHash>)>,
    messages: Said,
}

impl Exchange {
    /// A new exchange, for the side that starts one or the side that answers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The next message to send: from the side that starts, the first, then one after each
    /// answer until [`settled`](Self::settled); from the answering side, the answer to what
    /// [`incoming`](Self::incoming) just took in.
    ///
    /// # Errors
    ///
    /// If a document cannot be read, or one side has gone past [`MAX_ROUNDS`].
    pub fn outgoing(&mut self, store: &mut Store) -> Result<Vec<u8>> {
        if self.sent >= MAX_ROUNDS {
            return Err(SyncError::Protocol("its sync never settled".to_owned()));
        }
        let mine = store.sync_heads()?;
        self.known.extend(mine.iter().map(|(name, _)| name.clone()));
        self.newest_only();
        // Both last said the same heads: the same changes, nothing to say. Before the other side
        // has said its heads, only what is in memory already: a year would be loaded to say it,
        // and the other side, told this side's heads, speaks for any it holds otherwise.
        let held: BTreeMap<&SyncName, &Vec<ChangeHash>> = mine.iter().map(|(name, heads)| (name, heads)).collect();
        let loaded = store.documents().loaded_years();
        let saying: Vec<&SyncName> = self
            .known
            .iter()
            .filter(|name| match self.theirs.get(*name) {
                Some(theirs) => held.get(name) != Some(&theirs),
                None => !matches!(name.id, DocId::Blocks(year) if !loaded.contains(&year)),
            })
            .collect();
        let messages = self.rounds.say(store, saying)?;
        let fresh = self.sent == 0 && !self.answering;
        self.sent += 1;
        self.sent_nothing = messages.is_empty();
        Ok(encode(&Message {
            fresh,
            documents: mine.into_iter().map(|(name, heads)| (name.name(), heads)).collect(),
            messages,
        }))
    }

    /// Keeps only the newest generation either side holds, as a session does (`crate::session`).
    fn newest_only(&mut self) {
        let newest = self.known.iter().map(|name| name.generation.clone()).max().flatten();
        self.known.retain(|name| name.generation == newest);
    }

    /// Takes in a message from the other side. Returns the documents that changed here.
    ///
    /// A message marked fresh begins a new exchange on this side, whatever came before: the
    /// other side started again.
    ///
    /// # Errors
    ///
    /// If the message does not read, comes from another format, or the store refuses it.
    pub fn incoming(&mut self, store: &mut Store, bytes: &[u8]) -> Result<Vec<DocId>> {
        let message = decode(bytes)?;
        if message.fresh {
            // What the last exchange was taking in goes with it.
            *self = Self { answering: true, ..Self::default() };
        }
        // A kind of document this version does not know is left alone, as in a session.
        for (name, heads) in &message.documents {
            if let Some(name) = SyncName::parse(name) {
                self.known.insert(name.clone());
                self.theirs.insert(name, heads.clone());
            }
        }
        self.known.extend(store.sync_documents()?);
        self.newest_only();
        self.heard_nothing = message.messages.is_empty();
        // Only the documents of the generation agreed on, as in a session: a message for an older
        // generation's must never reach this store.
        let known = &self.known;
        self.rounds.hear(store, &message.messages, |name| known.contains(name))
    }

    /// Whether both sides have nothing more to say: the last message sent was empty, and so
    /// was the answer to it.
    #[must_use]
    pub const fn settled(&self) -> bool {
        self.sent > 0 && self.sent_nothing && self.heard_nothing
    }

    /// Once [`settled`](Self::settled), takes in the newer generation the other side sent, if
    /// it had erased for good (`Store::finish_adoption`): whole only now. Nothing before.
    ///
    /// # Errors
    ///
    /// If a document cannot be read or written.
    pub fn finish(&mut self, store: &mut Store) -> Result<Option<Adopted>> {
        if !self.settled() {
            return Ok(None);
        }
        self.rounds.adopt(store, &self.known)
    }
}

// The format: the protocol and the fresh mark, the documents held, then each sync message
// by document. Lengths are big-endian; names are UTF-8. Small and plain, since it crosses
// Bluetooth.
fn encode(message: &Message) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&PROTOCOL.to_be_bytes());
    out.push(u8::from(message.fresh));
    put_len(&mut out, message.documents.len());
    for (name, heads) in &message.documents {
        put_bytes(&mut out, name.as_bytes());
        put_len(&mut out, heads.len());
        for head in heads {
            out.extend_from_slice(&head.0);
        }
    }
    put_said(&mut out, &message.messages);
    out
}

fn decode(bytes: &[u8]) -> Result<Message> {
    let mut reader = Reader::new(bytes);
    let protocol = reader.u32()?;
    if protocol != PROTOCOL {
        return Err(SyncError::Protocol(format!(
            "it speaks link protocol {protocol} and this device speaks {PROTOCOL}; update the older one"
        )));
    }
    let fresh = reader.take(1)?[0] != 0;
    let documents = (0..reader.u32()?)
        .map(|_| {
            let name = reader.text()?;
            let heads = (0..reader.u32()?)
                .map(|_| Ok(ChangeHash(reader.take(32)?.try_into().unwrap_or([0; 32]))))
                .collect::<Result<_>>()?;
            Ok((name, heads))
        })
        .collect::<Result<_>>()?;
    let messages = reader.said()?;
    reader.end()?;
    Ok(Message { fresh, documents, messages })
}

// ---------------------------------------------------------------------------------------
// Parcels: changes handed on when the other side cannot answer
// ---------------------------------------------------------------------------------------

/// What marks a parcel, after the protocol: never a message's fresh mark, 0 or 1.
const PARCEL: u8 = 2;

/// Where this device keeps the heads the two sides last agreed on.
const AGREED: &str = "link-agreed";

/// Records that the two sides agree, as an exchange settling says: every document's heads as
/// this device has them now, which the other side has too. A parcel sends what came after.
///
/// # Errors
///
/// If the store cannot be read or written.
pub fn note_agreed(store: &mut Store) -> Result<()> {
    let heads = store.sync_heads()?;
    let text = lumenna_store::heads_to_text(heads.iter().map(|(name, heads)| (name.name(), heads.as_slice())));
    store.set_local_value(AGREED, text.as_bytes())?;
    Ok(())
}

/// The heads last agreed on, by document; none for a document never agreed on, or for any
/// when nothing ever settled.
fn agreed(store: &Store) -> Result<BTreeMap<SyncName, Vec<lumenna_store::ChangeHash>>> {
    let text = store.local_value(AGREED)?.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
    Ok(lumenna_store::heads_from_text(&text)
        .into_iter()
        .filter_map(|(name, heads)| Some((SyncName::parse(&name)?, heads)))
        .collect())
}

/// Every change the other side may lack, by document: all that came after the heads the two
/// last agreed on (`note_agreed`), for a link that queues it to be delivered when the two are
/// next in reach, with no reply (WatchConnectivity's `transferUserInfo`). Nothing when there is
/// nothing new. Sent again until the next exchange settles: a change the other side already
/// has is taken in as nothing.
///
/// # Errors
///
/// If the store cannot be read.
pub fn parcel(store: &mut Store) -> Result<Option<Vec<u8>>> {
    let agreed = agreed(store)?;
    let mut documents = Vec::new();
    for (name, _) in store.sync_heads()? {
        let since = agreed.get(&name).cloned().unwrap_or_default();
        let changes = store.changes_since(&name, &since)?;
        if !changes.is_empty() {
            documents.push((name.name(), changes));
        }
    }
    if documents.is_empty() {
        return Ok(None);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&PROTOCOL.to_be_bytes());
    out.push(PARCEL);
    put_len(&mut out, documents.len());
    for (name, changes) in &documents {
        put_bytes(&mut out, name.as_bytes());
        put_len(&mut out, changes.len());
        for change in changes {
            put_bytes(&mut out, change);
        }
    }
    Ok(Some(out))
}

/// Takes in a parcel from the other side. Returns the documents that changed here.
///
/// # Errors
///
/// If the parcel does not read, comes from another format, or the store refuses a change.
pub fn unparcel(store: &mut Store, bytes: &[u8]) -> Result<Vec<DocId>> {
    let mut reader = Reader::new(bytes);
    let protocol = reader.u32()?;
    if protocol != PROTOCOL || reader.take(1)?[0] != PARCEL {
        return Err(SyncError::Protocol(format!(
            "it hands on changes in link protocol {protocol}, and this device speaks {PROTOCOL}; update the older one"
        )));
    }
    let mut changed = Vec::new();
    for _ in 0..reader.u32()? {
        let name = reader.text()?;
        let changes = (0..reader.u32()?).map(|_| Ok(reader.chunk()?.to_vec())).collect::<Result<Vec<_>>>()?;
        // A kind of document this version does not know is left alone, as in an exchange, and
        // another generation's changes are too (`Store::take_changes`): a newer one comes by an
        // exchange, whole.
        let Some(name) = SyncName::parse(&name) else { continue };
        if store.take_changes(&name, &changes)? {
            changed.push(name.id);
        }
    }
    if reader.at != bytes.len() {
        return Err(SyncError::Protocol("a parcel with something after its end".to_owned()));
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use lumenna_core::ProjectId;
    use lumenna_core::model::Task;
    use lumenna_core::order::OrderKey;

    use super::*;

    fn add(store: &mut Store, title: &str) {
        let task = Task::new(ProjectId::INBOX, title, OrderKey::middle());
        store.write(|docs| docs.put_task(&task, None)).unwrap();
    }

    fn titles(store: &mut Store) -> Vec<String> {
        let mut titles: Vec<String> = store.snapshot().0.tasks.values().map(|t| t.title.clone()).collect();
        titles.sort();
        titles
    }

    /// Runs an exchange to the end, `starting` sending first, as the watch does.
    fn exchange(starting: &mut Store, answering: &mut Store) -> usize {
        let (mut ours, mut theirs) = (Exchange::new(), Exchange::new());
        let mut rounds = 0;
        while !ours.settled() {
            let request = ours.outgoing(starting).unwrap();
            theirs.incoming(answering, &request).unwrap();
            let reply = theirs.outgoing(answering).unwrap();
            ours.incoming(starting, &reply).unwrap();
            rounds += 1;
            assert!(rounds < 50, "the exchange never settled");
        }
        rounds
    }

    #[test]
    fn an_exchange_brings_each_side_what_the_other_wrote_and_settles() {
        let mut watch = Store::open_in_memory().unwrap();
        let mut phone = Store::open_in_memory().unwrap();
        add(&mut watch, "from the watch");
        add(&mut phone, "from the phone");

        exchange(&mut watch, &mut phone);

        assert_eq!(titles(&mut watch), ["from the phone", "from the watch"]);
        assert_eq!(titles(&mut phone), ["from the phone", "from the watch"]);
    }

    #[test]
    fn an_exchange_between_stores_already_alike_loads_no_year() {
        let dir = tempfile::tempdir().unwrap();
        let (watch_path, phone_path) = (dir.path().join("watch.sqlite"), dir.path().join("phone.sqlite"));
        {
            let (mut watch, mut phone) = (Store::open(&watch_path).unwrap(), Store::open(&phone_path).unwrap());
            add(&mut watch, "water the plants");
            for year in [2027, 2028] {
                let block = lumenna_core::model::BlockSeries::one_off(
                    "Retreat",
                    lumenna_core::model::BlockKind::Work,
                    jiff::civil::date(year, 4, 1),
                    jiff::civil::time(9, 0, 0, 0),
                    60,
                )
                .unwrap();
                phone.write(|docs| docs.put_series(&block, None)).unwrap();
            }
            exchange(&mut watch, &mut phone);
        }
        // Opened afresh, as the watch's app is: no year in memory on either side.
        let (mut watch, mut phone) = (Store::open(&watch_path).unwrap(), Store::open(&phone_path).unwrap());
        exchange(&mut watch, &mut phone);
        assert!(watch.documents().loaded_years().is_empty(), "the watch loaded a year it needed nothing in");
        assert!(phone.documents().loaded_years().is_empty(), "the phone loaded a year the watch needed nothing in");
        note_agreed(&mut watch).unwrap();
        assert!(parcel(&mut watch).unwrap().is_none());
        assert!(watch.documents().loaded_years().is_empty(), "nor to make a parcel");
    }

    #[test]
    fn a_message_for_a_document_of_an_older_generation_is_left_alone() {
        let (mut watch, mut phone) = (Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap());
        add(&mut watch, "the secret");
        exchange(&mut watch, &mut phone);
        let task = watch.snapshot().0.tasks.values().next().unwrap().clone();
        // The phone erases for good; the watch, behind, starts an exchange in the history before.
        let edit = lumenna_core::edit::purge_task(&phone.snapshot().0, task.id).unwrap();
        phone.erase(lumenna_store::Documents::erased_by(&edit)).unwrap();
        let mut theirs = Exchange::new();
        let changed = theirs.incoming(&mut phone, &Exchange::new().outgoing(&mut watch).unwrap()).unwrap();
        assert!(changed.is_empty());
        assert!(phone.snapshot().0.tasks.is_empty(), "nothing of the history before came back");
    }

    #[test]
    fn a_fresh_message_starts_the_answering_side_again() {
        let mut watch = Store::open_in_memory().unwrap();
        let mut phone = Store::open_in_memory().unwrap();
        let mut theirs = Exchange::new();
        // A first exchange abandoned halfway, as when the watch goes out of reach.
        let mut abandoned = Exchange::new();
        theirs.incoming(&mut phone, &abandoned.outgoing(&mut watch).unwrap()).unwrap();
        theirs.outgoing(&mut phone).unwrap();

        add(&mut watch, "written since");
        let mut ours = Exchange::new();
        while !ours.settled() {
            theirs.incoming(&mut phone, &ours.outgoing(&mut watch).unwrap()).unwrap();
            ours.incoming(&mut watch, &theirs.outgoing(&mut phone).unwrap()).unwrap();
        }
        assert_eq!(titles(&mut phone), ["written since"]);
    }

    #[test]
    fn a_message_from_another_format_or_cut_short_is_refused() {
        let mut store = Store::open_in_memory().unwrap();
        let mut message = Exchange::new().outgoing(&mut store).unwrap();
        let error = Exchange::new().incoming(&mut store, &message[..message.len() - 1]).unwrap_err();
        assert!(error.to_string().contains("cut short"), "{error}");
        message[3] = 9;
        let error = Exchange::new().incoming(&mut store, &message).unwrap_err();
        assert!(error.to_string().contains("update the older one"), "{error}");
    }

    #[test]
    fn a_parcel_carries_what_came_after_the_last_agreement_and_takes_no_reply() {
        let (mut watch, mut phone) = (Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap());
        add(&mut watch, "water the plants");
        exchange(&mut watch, &mut phone);
        note_agreed(&mut watch).unwrap();
        note_agreed(&mut phone).unwrap();
        assert!(parcel(&mut watch).unwrap().is_none(), "nothing new since they agreed");

        // Away from the phone: the change goes into a parcel the system delivers later.
        add(&mut watch, "call the bank");
        let parcel = parcel(&mut watch).unwrap().expect("what came after");
        assert_eq!(unparcel(&mut phone, &parcel).unwrap(), [DocId::Core]);
        assert_eq!(titles(&mut phone), ["call the bank", "water the plants"]);
        assert!(unparcel(&mut phone, &parcel).unwrap().is_empty(), "delivered twice, taken once");
    }

    #[test]
    fn a_parcel_before_anything_was_agreed_carries_everything() {
        let (mut watch, mut phone) = (Store::open_in_memory().unwrap(), Store::open_in_memory().unwrap());
        add(&mut watch, "water the plants");
        let parcel = parcel(&mut watch).unwrap().expect("everything");
        unparcel(&mut phone, &parcel).unwrap();
        assert_eq!(titles(&mut phone), ["water the plants"]);
    }
}
