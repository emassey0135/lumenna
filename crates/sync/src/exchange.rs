//! The sync session's reconciliation, over messages instead of a stream.
//!
//! For a link that carries whole messages with a reply to each, such as the one between an
//! Apple Watch and its iPhone: the watch cannot use Iroh (watchOS allows no sockets outside
//! an audio session), and WatchConnectivity is a request and its answer, not a byte stream.
//!
//! It is the same Automerge sync protocol as [`session`](crate::session), with every
//! document in each message rather than one document per frame, so a round trip does every
//! document's round at once:
//!
//! 1. The side that starts sends a message marked *fresh*: the documents it holds, and its
//!    first sync message for each. The other side begins a new exchange on seeing it.
//! 2. Each answer takes in what arrived and says the next sync message for every document
//!    either side holds, or nothing for one already settled.
//! 3. When a message that said nothing is answered by one that says nothing, the two agree.
//!
//! Sync state lasts the exchange, not longer, as in a session.

use std::collections::{BTreeMap, BTreeSet};

use lumenna_store::{DocId, Store, SyncState};

use crate::error::{Result, SyncError};

/// The exchange's message format. Another is refused, saying which side to update.
const PROTOCOL: u32 = 0;

/// The most messages one side sends in an exchange: Automerge settles in a handful, so this
/// guards against a side that never stops, and is not a tuning parameter.
pub const MAX_ROUNDS: usize = 1_000;

/// One side of an exchange.
#[derive(Default)]
pub struct Exchange {
    states: BTreeMap<DocId, SyncState>,
    known: BTreeSet<DocId>,
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
    documents: Vec<String>,
    messages: Vec<(String, Vec<u8>)>,
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
            return Err(SyncError::Protocol("the other device's sync did not settle".to_owned()));
        }
        let mine = store.sync_documents()?;
        self.known.extend(mine.iter().copied());
        let mut messages = Vec::new();
        for &id in &self.known {
            let state = self.states.entry(id).or_default();
            if let Some(message) = store.sync_message(id, state)? {
                messages.push((id.name(), message));
            }
        }
        let fresh = self.sent == 0 && !self.answering;
        self.sent += 1;
        self.sent_nothing = messages.is_empty();
        Ok(encode(&Message { fresh, documents: mine.iter().map(|id| id.name()).collect(), messages }))
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
            *self = Self { answering: true, ..Self::default() };
        }
        // A kind of document this version does not know is left alone, as in a session.
        self.known.extend(message.documents.iter().filter_map(|name| DocId::parse(name)));
        self.heard_nothing = message.messages.is_empty();
        let mut changed = Vec::new();
        for (name, sync) in &message.messages {
            let Some(id) = DocId::parse(name) else { continue };
            self.known.insert(id);
            let state = self.states.entry(id).or_default();
            if store.receive_sync_message(id, state, sync)? {
                changed.push(id);
            }
        }
        Ok(changed)
    }

    /// Whether both sides have nothing more to say: the last message sent was empty, and so
    /// was the answer to it.
    #[must_use]
    pub const fn settled(&self) -> bool {
        self.sent > 0 && self.sent_nothing && self.heard_nothing
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
    for name in &message.documents {
        put_bytes(&mut out, name.as_bytes());
    }
    put_len(&mut out, message.messages.len());
    for (name, bytes) in &message.messages {
        put_bytes(&mut out, name.as_bytes());
        put_bytes(&mut out, bytes);
    }
    out
}

fn put_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_be_bytes());
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_len(out, bytes.len());
    out.extend_from_slice(bytes);
}

fn decode(bytes: &[u8]) -> Result<Message> {
    let mut reader = Reader { bytes, at: 0 };
    let protocol = reader.u32()?;
    if protocol != PROTOCOL {
        return Err(SyncError::Protocol(format!(
            "it speaks link protocol {protocol} and this device speaks {PROTOCOL}; update the older one"
        )));
    }
    let fresh = reader.take(1)?[0] != 0;
    let documents = (0..reader.u32()?).map(|_| reader.text()).collect::<Result<_>>()?;
    let messages = (0..reader.u32()?)
        .map(|_| Ok((reader.text()?, reader.chunk()?.to_vec())))
        .collect::<Result<_>>()?;
    if reader.at != bytes.len() {
        return Err(SyncError::Protocol("a link message with something after its end".to_owned()));
    }
    Ok(Message { fresh, documents, messages })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(len).filter(|end| *end <= self.bytes.len());
        let end = end.ok_or_else(|| SyncError::Protocol("a link message cut short".to_owned()))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn chunk(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    fn text(&mut self) -> Result<String> {
        String::from_utf8(self.chunk()?.to_vec())
            .map_err(|_| SyncError::Protocol("a link message naming a document in something not UTF-8".to_owned()))
    }
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
    let mut text = String::new();
    for (id, heads) in store.sync_heads()? {
        text.push_str(&id.name());
        for head in heads {
            text.push(' ');
            text.push_str(&head.to_string());
        }
        text.push('\n');
    }
    store.set_local_value(AGREED, text.as_bytes())?;
    Ok(())
}

/// The heads last agreed on, by document; none for a document never agreed on, or for any
/// when nothing ever settled.
fn agreed(store: &Store) -> Result<BTreeMap<DocId, Vec<lumenna_store::ChangeHash>>> {
    let text = store.local_value(AGREED)?.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
    Ok(text
        .lines()
        .filter_map(|line| {
            let mut words = line.split(' ');
            let id = DocId::parse(words.next()?)?;
            Some((id, words.filter_map(|head| head.parse().ok()).collect()))
        })
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
    for (id, _) in store.sync_heads()? {
        let since = agreed.get(&id).cloned().unwrap_or_default();
        let changes = store.changes_since(id, &since)?;
        if !changes.is_empty() {
            documents.push((id.name(), changes));
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
    let mut reader = Reader { bytes, at: 0 };
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
        // A kind of document this version does not know is left alone, as in an exchange.
        let Some(id) = DocId::parse(&name) else { continue };
        if store.take_changes(id, &changes)? {
            changed.push(id);
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
