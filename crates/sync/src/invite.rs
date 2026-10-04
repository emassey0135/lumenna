//! The short-lived endpoint a pairing runs on (§7).
//!
//! **A pairing never uses the device key.** It runs on a fresh endpoint with a key minted for
//! this one pairing and thrown away after, for two reasons:
//!
//! - **It needs no daemon.** The device key's endpoint is held by whichever process has the
//!   sync lock — on a BTSpeak, the always-running daemon, which has no screen to show words on
//!   and no keyboard to answer. A pairing on its own endpoint runs from whatever terminal the
//!   person is at, beside a daemon that carries on syncing.
//! - **It changes nothing about trust.** The words authenticate this connection, and only once
//!   they have been confirmed on both sides does each device say which device key it is. A
//!   key learned that way is one a person vouched for; a key merely seen on the network never
//!   is.
//!
//! Finding each other is the *where* half of §7. On one network, **both people run
//! `lum pair`** and the two sessions find each other by mDNS, under a service name only
//! pairing sessions use: nothing to type, no outside service. Off the network, one side gives
//! the other its pairing code to dial.

use iroh::endpoint::Connection;
use iroh::{Endpoint, EndpointAddr, SecretKey};
use crate::discovery::LocalLookup;
use lumenna_core::edit;
use lumenna_core::model::Device;

use crate::error::{Result, SyncError};
use crate::node::{Network, finish};
use crate::pairing::{self, Identity, Role};
use crate::session::{self, Summary};
use crate::{ALPN_PAIR, SharedStore, lock};

/// The mDNS service name pairing sessions advertise under. Distinct from paired devices',
/// so a device going about its business never shows up as someone waiting to pair.
const PAIRING_SERVICE: &str = "lumenna-pair";

/// How long to listen on the local network for a pairing code before dialling it anyway.
const LOCATE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// How long a dial to a session heard on the local network gets. Plenty on one network, and
/// short enough that a stale announcement costs seconds rather than the connection timeout.
const HEARD_DIAL_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// The label for the TLS exporter secret the words come from.
const EXPORTER_LABEL: &[u8] = b"lumenna pairing v0";

/// What a pairing achieved.
#[derive(Debug)]
pub struct Paired {
    /// The device now paired with this one.
    pub peer: Identity,
    /// The first sync, run straight after over the same connection.
    pub summary: Summary,
}

/// A pairing session, open and findable.
#[derive(Debug)]
pub struct Invitation {
    endpoint: Endpoint,
    local: LocalLookup,
}

impl Invitation {
    /// Opens a pairing session on a key of its own.
    ///
    /// `findable` is whether it announces itself on the local network. A session waiting to
    /// be found does; one about to dial a code it was given does not, so that the session it
    /// is dialling never sees it and dials back — two connections between one pair of sessions
    /// would each wait for the other to speak first. It still listens, to locate that code.
    ///
    /// # Errors
    ///
    /// If the endpoint cannot bind.
    pub async fn open(network: Network, findable: bool) -> Result<Self> {
        let (endpoint, local) = crate::node::bind(
            SecretKey::generate(),
            ALPN_PAIR,
            network,
            PAIRING_SERVICE,
            findable,
        )
        .await?;
        Ok(Self { endpoint, local })
    }

    /// The pairing code: this session's key, for the other device to dial when the two are not
    /// on one network. Good for this session only.
    #[must_use]
    pub fn code(&self) -> String {
        self.endpoint.id().to_string()
    }

    /// Where this session can be reached right now, for tests without a lookup service.
    #[must_use]
    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    /// Meets the other device: dials `peer` if given, and otherwise waits to be dialled or
    /// finds another pairing session on the local network.
    ///
    /// When two sessions find each other, the one with the smaller key dials and the other
    /// waits, so they meet once rather than twice.
    ///
    /// # Errors
    ///
    /// If the peer cannot be reached, or the endpoint closes first.
    pub async fn meet(&self, peer: Option<EndpointAddr>) -> Result<(Connection, Role)> {
        if let Some(peer) = peer {
            let peer = self.locate(peer).await;
            return Ok((self.dial(peer).await?, Role::Dialled));
        }
        let me = self.endpoint.id();
        let (already, mut found) = self.local.subscribe();
        // Sessions heard before this one started listening, then the ones heard after.
        let mut waiting: std::collections::VecDeque<_> = already.into();
        loop {
            if let Some(them) = waiting.pop_front() {
                let addr = EndpointAddr::from_parts(
                    them.id,
                    them.addrs.into_iter().map(iroh::TransportAddr::Ip),
                );
                // A session that has gone without saying so — a closed laptop, a phone off
                // the network — can stay announced for a while. Each such dial gives up
                // quickly, rather than holding up the meeting for a full connection timeout.
                if me < them.id
                    && let Ok(Ok(conn)) = tokio::time::timeout(HEARD_DIAL_WAIT, self.dial(addr)).await
                {
                    return Ok((conn, Role::Dialled));
                }
                continue;
            }
            tokio::select! {
                incoming = self.endpoint.accept() => {
                    let incoming = incoming.ok_or_else(|| {
                        SyncError::Network("the pairing session closed".to_owned())
                    })?;
                    let Ok(accepting) = incoming.accept() else { continue };
                    if let Ok(conn) = accepting.await {
                        return Ok((conn, Role::Answered));
                    }
                }
                heard = found.recv() => match heard {
                    Ok(them) => waiting.push_back(them),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    // The lookup is gone; carry on waiting to be dialled.
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return self.wait().await;
                    }
                },
            }
        }
    }

    /// Waits to be dialled, without looking for anyone. For when the other side was given
    /// this session's code.
    ///
    /// # Errors
    ///
    /// If the endpoint closes first.
    pub async fn wait(&self) -> Result<(Connection, Role)> {
        loop {
            let incoming = self
                .endpoint
                .accept()
                .await
                .ok_or_else(|| SyncError::Network("the pairing session closed".to_owned()))?;
            let Ok(accepting) = incoming.accept() else { continue };
            if let Ok(conn) = accepting.await {
                return Ok((conn, Role::Answered));
            }
        }
    }

    /// Fills in where a pairing code is on the local network, when mDNS can say.
    ///
    /// A code alone is a key with no address. With relays and the lookup service that is
    /// enough, but on the local network only mDNS knows, and it takes a moment to hear from
    /// a session that has just opened — longer than a dial waits. So this listens for that
    /// one key for a few seconds first, and dials with whatever it heard. If nothing answers
    /// the dial still goes ahead, and the lookup services get their turn.
    async fn locate(&self, peer: EndpointAddr) -> EndpointAddr {
        if !peer.addrs.is_empty() {
            return peer;
        }
        let id = peer.id;
        let addr = move |addrs: Vec<std::net::SocketAddr>| {
            EndpointAddr::from_parts(id, addrs.into_iter().map(iroh::TransportAddr::Ip))
        };
        // Heard already, or heard from now on — taken together, so a session heard in between
        // is in one or the other rather than neither.
        let (already, mut events) = self.local.subscribe();
        if let Some(found) = already.into_iter().find(|found| found.id == id) {
            return addr(found.addrs);
        }
        let heard = tokio::time::timeout(LOCATE_WAIT, async {
            loop {
                match events.recv().await {
                    Ok(found) if found.id == id => return Some(found.addrs),
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        })
        .await;
        heard.ok().flatten().map_or(peer, addr)
    }

    async fn dial(&self, peer: EndpointAddr) -> Result<Connection> {
        self.endpoint
            .connect(peer, ALPN_PAIR)
            .await
            .map_err(|e| SyncError::Network(format!("could not reach the other device: {e}")))
    }

    /// Runs a pairing over a connection [`meet`](Self::meet) returned.
    ///
    /// Shows the words through `confirm`, which returns whether the person says they match;
    /// exchanges decisions; and if both said yes, writes both devices into `devices` and runs
    /// the first sync over the same connection, so the new device has everything before this
    /// returns.
    ///
    /// # Errors
    ///
    /// [`SyncError::NotPaired`] if either side said no; otherwise a network, protocol or
    /// store failure.
    pub async fn pair<F, Fut>(
        &self,
        conn: &Connection,
        role: Role,
        store: &SharedStore,
        me: Identity,
        confirm: F,
    ) -> Result<Paired>
    where
        F: FnOnce(Vec<String>) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let (mut send, mut recv) = match role {
            Role::Dialled => conn.open_bi().await,
            Role::Answered => conn.accept_bi().await,
        }
        .map_err(|e| SyncError::Network(e.to_string()))?;

        let mut secret = [0u8; 32];
        conn.export_keying_material(&mut secret, EXPORTER_LABEL, b"")
            .map_err(|_| SyncError::Protocol("the connection gave no secret to compare".to_owned()))?;
        let words = pairing::compare(role, &mut recv, &mut send, &secret).await?;
        let accepted = confirm(words).await;
        let peer = match pairing::decide(&mut recv, &mut send, accepted, &me).await {
            Ok(peer) => peer,
            Err(refused @ SyncError::NotPaired(_)) => {
                // Both decisions have been exchanged; end the connection in order, so the
                // other device reads this one's answer instead of a dropped connection.
                let _ = finish(&mut send, &mut recv).await;
                close(conn, role).await;
                return Err(refused);
            }
            Err(other) => return Err(other),
        };
        if peer.node_id == me.node_id {
            return Err(SyncError::NotPaired(
                "that is this device — the same store opened twice — so there is nothing to pair"
                    .to_owned(),
            ));
        }

        enroll(store, &me, &peer)?;
        let summary = session::run(store, &mut recv, &mut send).await?;
        finish(&mut send, &mut recv).await?;
        close(conn, role).await;
        Ok(Paired { peer, summary })
    }

    /// Closes the session.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

/// The dialler closes once it has everything; the answerer waits for that, so neither tears
/// down data the other has not read yet.
async fn close(conn: &Connection, role: Role) {
    match role {
        Role::Dialled => conn.close(0u32.into(), b"done"),
        Role::Answered => {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(5), conn.closed()).await;
        }
    }
}

/// Writes both devices into `devices`, through the undo history: unpairing by mistake, or
/// pairing by mistake, is the kind of thing a person wants back.
fn enroll(store: &SharedStore, me: &Identity, peer: &Identity) -> Result<()> {
    let now = lumenna_core::time::now();
    let device = |identity: &Identity| -> Result<Device> {
        Ok(Device {
            node_id: identity.node_id.parse().map_err(|_| {
                SyncError::Protocol(format!("'{}' is not a device key", identity.node_id))
            })?,
            name: identity.name.clone(),
            platform: identity.platform.clone(),
            paired_at: now,
            last_seen: now,
        })
    };
    let devices = [device(me)?, device(peer)?];
    let mut store = lock(store)?;
    let edit = edit::enroll_devices(&store.snapshot().0, &devices);
    if !edit.is_empty() {
        store.apply_recorded(&edit)?;
    }
    Ok(())
}

/// This device's identity for a pairing: its device key, and what to call it.
///
/// # Errors
///
/// If the device key cannot be read or minted.
pub fn identity(store: &SharedStore, name: &str, platform: &str) -> Result<Identity> {
    let key = crate::node::device_key(store)?;
    Ok(Identity {
        node_id: key.public().to_string(),
        name: name.to_owned(),
        platform: platform.to_owned(),
    })
}
