//! Syncing over a link the platform already trusts: a Wear OS watch's to its phone.
//!
//! Google's Wearable Data Layer joins only a phone and the watch paired with it, and only
//! apps with the same package and signing key, so a link it opens is between two of this
//! person's own devices as surely as a pairing whose words matched. Over it, each side says
//! who it is, the first time enrolls both in `devices` — which takes the watch into the
//! whole network, as a pairing does — and then the documents are reconciled with the
//! ordinary session.
//!
//! An introduction happens once per pair of devices. After that, a device unpaired elsewhere
//! stays unpaired: the link syncs only devices both sides still list, and never enrolls one
//! again. Whether it introduced itself already is each side's own, in `local_state`.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::{Result, SyncError};
use crate::framing::{read_frame, write_frame};
use crate::pairing::Identity;
use crate::session::{self, Summary};
use crate::{SharedStore, lock};

/// What each side says before anything is synced.
#[derive(Serialize, Deserialize)]
struct Hello {
    identity: Identity,
    /// Whether this side will go on: both must.
    go: bool,
    /// Why not, when it will not.
    #[serde(default)]
    reason: Option<String>,
}

/// What an introduction did.
#[derive(Debug)]
pub struct Introduced {
    /// The other device.
    pub peer: Identity,
    /// Whether this was the first time, so both were enrolled.
    pub enrolled: bool,
    /// What the session brought.
    pub summary: Summary,
}

fn local_key(peer: &str) -> String {
    format!("introduced/{peer}")
}

/// Introduces this device (`me`) over a trusted link and syncs, on both sides alike.
///
/// # Errors
///
/// If the link fails, the other side is this device, or either side will not go on: a
/// device that was introduced before and unpaired since.
pub async fn introduce_and_sync<R, W>(store: &SharedStore, me: &Identity, reader: &mut R, writer: &mut W) -> Result<Introduced>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    // This side's half first, then the other's: sending before reading cannot deadlock.
    write_frame(writer, &encode(&Hello { identity: me.clone(), go: true, reason: None })?).await?;
    let theirs: Hello = decode(&read_frame(reader).await?)?;
    let peer = theirs.identity;

    let decision = decide(store, me, &peer);
    let mine = Hello { identity: me.clone(), go: decision.is_ok(), reason: decision.as_ref().err().cloned() };
    write_frame(writer, &encode(&mine)?).await?;
    let answer: Hello = decode(&read_frame(reader).await?)?;
    let enrolled = match (decision, answer.go) {
        (Ok(enrol), true) => enrol,
        (Err(reason), _) => return Err(SyncError::Refused(reason)),
        (Ok(_), false) => {
            return Err(SyncError::Refused(answer.reason.unwrap_or_else(|| "the other device would not sync".to_owned())));
        }
    };
    if enrolled {
        enroll(store, me, &peer)?;
    }
    let summary = session::run(store, reader, writer).await?;
    let mut held = lock(store)?;
    held.set_local_flag(&local_key(&peer.node_id))?;
    held.note_linked(&peer.node_id);
    let _ = held.record_peer(&peer.node_id, None);
    Ok(Introduced { peer, enrolled, summary })
}

/// Whether to go on, and whether to enroll first: `Ok(true)` the first time, `Ok(false)`
/// when both are already paired, a refusal when the peer was introduced and unpaired since.
fn decide(store: &SharedStore, me: &Identity, peer: &Identity) -> std::result::Result<bool, String> {
    if peer.node_id == me.node_id {
        return Err("that is this device, so there is nothing to sync".to_owned());
    }
    let held = lock(store).map_err(|e| e.to_string())?;
    let devices = held.snapshot().0.devices;
    let listed = |id: &str| devices.keys().any(|k| k.to_string() == id);
    let introduced = held.local_flag(&local_key(&peer.node_id)).unwrap_or(false);
    match (listed(&peer.node_id) && listed(&me.node_id), introduced) {
        (true, _) => Ok(false),
        (false, false) => Ok(true),
        (false, true) => Err(format!(
            "{} was unpaired, so it is not synced over the watch's link; pair it again to sync",
            peer.name
        )),
    }
}

fn enroll(store: &SharedStore, me: &Identity, peer: &Identity) -> Result<()> {
    let now = lumenna_core::time::now();
    let device = |identity: &Identity| -> Result<lumenna_core::model::Device> {
        Ok(lumenna_core::model::Device {
            node_id: identity
                .node_id
                .parse()
                .map_err(|_| SyncError::Protocol(format!("'{}' is not a device key", identity.node_id)))?,
            name: identity.name.clone(),
            platform: identity.platform.clone(),
            paired_at: now,
            last_seen: now,
            schema: identity.schema,
        })
    };
    let devices = [device(me)?, device(peer)?];
    let mut held = lock(store)?;
    let edit = lumenna_core::edit::enroll_devices(&held.snapshot().0, &devices);
    if !edit.is_empty() {
        held.apply_recorded(&edit)?;
    }
    Ok(())
}

fn encode(hello: &Hello) -> Result<Vec<u8>> {
    serde_json::to_vec(hello).map_err(|e| SyncError::Protocol(format!("could not write an introduction: {e}")))
}

fn decode(bytes: &[u8]) -> Result<Hello> {
    serde_json::from_slice(bytes).map_err(|e| SyncError::Protocol(format!("an introduction that does not read: {e}")))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use lumenna_store::Store;

    use super::*;

    fn store() -> SharedStore {
        Arc::new(Mutex::new(Store::open_in_memory().unwrap()))
    }

    fn me(store: &SharedStore, name: &str) -> Identity {
        crate::invite::identity(store, name, "android").unwrap()
    }

    async fn link(a: &SharedStore, b: &SharedStore) -> (Result<Introduced>, Result<Introduced>) {
        let (ida, idb) = (me(a, "Phone"), me(b, "Watch"));
        let (mut a_end, mut b_end) = tokio::io::duplex(1 << 16);
        let (a2, b2) = (a.clone(), b.clone());
        let left = tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut a_end);
            introduce_and_sync(&a2, &ida, &mut r, &mut w).await
        });
        let right = tokio::spawn(async move {
            let (mut r, mut w) = tokio::io::split(&mut b_end);
            introduce_and_sync(&b2, &idb, &mut r, &mut w).await
        });
        (left.await.unwrap(), right.await.unwrap())
    }

    fn names(store: &SharedStore) -> Vec<String> {
        let mut names: Vec<String> = store.lock().unwrap().snapshot().0.devices.values().map(|d| d.name.clone()).collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn the_first_link_pairs_both_devices_without_words_and_syncs() {
        let (phone, watch) = (store(), store());
        let (left, right) = link(&phone, &watch).await;
        assert!(left.unwrap().enrolled && right.unwrap().enrolled);
        assert_eq!(names(&phone), ["Phone", "Watch"]);
        assert_eq!(names(&watch), ["Phone", "Watch"], "the devices list came across too");
    }

    #[tokio::test]
    async fn a_watch_unpaired_since_is_not_paired_again_by_the_link() {
        let (phone, watch) = (store(), store());
        let _ = link(&phone, &watch).await;
        let watch_id = me(&watch, "Watch").node_id.parse().unwrap();
        {
            let mut held = phone.lock().unwrap();
            let edit = lumenna_core::edit::unpair_device(&held.snapshot().0, watch_id).unwrap();
            held.apply_recorded(&edit).unwrap();
        }
        let (left, right) = link(&phone, &watch).await;
        assert!(left.is_err() && right.is_err(), "both stop: {left:?} {right:?}");
        assert_eq!(names(&phone), ["Phone"]);
    }
}
