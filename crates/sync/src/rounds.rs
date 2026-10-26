//! Every document's round of Automerge's sync protocol at once, as the session over a stream
//! and the exchange over messages both run it: each side says the next sync message for every
//! document that has one, as a list by name, and takes in the other's list. One round trip
//! moves every document a step, so a device holding several years' documents settles in as
//! many round trips as its slowest document needs, not the sum of them all.
//!
//! The list on the wire: a count, then each document's name and its message, every length a
//! big-endian `u32` and every name UTF-8.

use std::collections::BTreeMap;

use lumenna_store::{Adopted, Adoption, DocId, Store, SyncName, SyncState};

use crate::error::{Result, SyncError};

/// What one side says in a round: each document's next sync message, by name.
pub(crate) type Said = Vec<(String, Vec<u8>)>;

/// One side's sync state for every document, kept for the session or exchange it serves, and
/// the newer generation it is taking in, if the other side has one: its own, so it goes with
/// the session or exchange, whole or not, and no other one touches it.
#[derive(Default)]
pub(crate) struct Rounds {
    states: BTreeMap<SyncName, SyncState>,
    adoption: Adoption,
}

impl Rounds {
    /// The next sync message for each of `documents` that has one to say.
    pub(crate) fn say<'a>(&mut self, store: &mut Store, documents: impl IntoIterator<Item = &'a SyncName>) -> Result<Said> {
        let mut said = Vec::new();
        for name in documents {
            let state = self.states.entry(name.clone()).or_default();
            if let Some(message) = store.sync_message(name, state, &mut self.adoption)? {
                said.push((name.name(), message));
            }
        }
        Ok(said)
    }

    /// Takes in what the other side said, each document `taken` accepts. Returns the documents
    /// that changed here. A name this version cannot read is left alone: a newer version's kind
    /// of document is that version's business.
    pub(crate) fn hear(&mut self, store: &mut Store, said: &Said, taken: impl Fn(&SyncName) -> bool) -> Result<Vec<DocId>> {
        let mut changed = Vec::new();
        for (name, message) in said {
            let Some(name) = SyncName::parse(name).filter(|name| taken(name)) else { continue };
            let state = self.states.entry(name.clone()).or_default();
            if store.receive_sync_message(&name, state, &mut self.adoption, message)? {
                changed.push(name.id);
            }
        }
        Ok(changed)
    }

    /// Takes in the newer generation the other side sent, once every one of `agreed` of it has
    /// arrived (`Store::finish_adoption`). It is used up either way.
    pub(crate) fn adopt<'a>(&mut self, store: &mut Store, agreed: impl IntoIterator<Item = &'a SyncName>) -> Result<Option<Adopted>> {
        Ok(store.finish_adoption(std::mem::take(&mut self.adoption), agreed)?)
    }
}

/// `said` as the list on the wire, after whatever `out` already holds.
pub(crate) fn put_said(out: &mut Vec<u8>, said: &Said) {
    put_len(out, said.len());
    for (name, message) in said {
        put_bytes(out, name.as_bytes());
        put_bytes(out, message);
    }
}

pub(crate) fn put_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&u32::try_from(len).unwrap_or(u32::MAX).to_be_bytes());
}

pub(crate) fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_len(out, bytes.len());
    out.extend_from_slice(bytes);
}

/// Reads what [`put_said`] and its kind wrote, refusing anything cut short.
pub(crate) struct Reader<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(len).filter(|end| *end <= self.bytes.len());
        let end = end.ok_or_else(|| SyncError::Protocol("a sync message cut short".to_owned()))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    pub(crate) fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn chunk(&mut self) -> Result<&'a [u8]> {
        let len = self.u32()? as usize;
        self.take(len)
    }

    pub(crate) fn text(&mut self) -> Result<String> {
        String::from_utf8(self.chunk()?.to_vec())
            .map_err(|_| SyncError::Protocol("a sync message naming a document in something not UTF-8".to_owned()))
    }

    pub(crate) fn said(&mut self) -> Result<Said> {
        (0..self.u32()?).map(|_| Ok((self.text()?, self.chunk()?.to_vec()))).collect()
    }

    /// Refuses anything left after the end.
    pub(crate) fn end(&self) -> Result<()> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(SyncError::Protocol("a sync message with something after its end".to_owned()))
        }
    }
}
