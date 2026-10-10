//! This device on the network: dialling its paired devices, and answering them.
//!
//! The endpoint's key is the device key, kept in the store (`Store::device_secret`), so
//! every process using the store is the same device. Only one of them runs the endpoint at a
//! time — whichever holds the sync lock — and the rest read and write the store as usual.
//!
//! **Trust is the `devices` document**. A connection from a key not listed there is
//! closed before a byte of data moves, and this device never dials one either.

use std::time::Duration;

use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId, SecretKey};
use crate::discovery::LocalLookup;
use lumenna_core::id::NodeId;

use crate::error::{Result, SyncError};
use crate::session::{self, Summary};
use crate::{ALPN_SYNC, SharedStore, lock};

/// Which servers a device may lean on to find and reach its peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    /// Iroh's relays and its address lookup service, plus the local network. Devices reach
    /// each other wherever they are; the relays forward traffic they cannot read.
    Internet,
    /// The local network only: no relay, no lookup service, nothing outside the building.
    /// Peers are found by mDNS or given an address. Also what tests use, offline.
    LocalOnly,
}

/// The mDNS service name paired devices advertise under, so they find each other on a local
/// network without any outside service.
const DEVICE_SERVICE: &str = "lumenna";

/// How long one peer gets before a sync with it is given up for this round.
pub const PEER_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn bind(
    secret: SecretKey,
    alpn: &[u8],
    network: Network,
    service: &str,
    advertise: bool,
) -> Result<(Endpoint, LocalLookup)> {
    let builder = match network {
        Network::Internet => Endpoint::builder(iroh::endpoint::presets::N0),
        Network::LocalOnly => Endpoint::builder(iroh::endpoint::presets::Minimal)
            .relay_mode(iroh::RelayMode::Disabled),
    };
    let endpoint = builder
        .secret_key(secret)
        .alpns(vec![alpn.to_vec()])
        .bind()
        .await
        .map_err(|e| SyncError::Network(format!("could not open the network endpoint: {e}")))?;
    // The local network is the path that must always work, with no outside service.
    // Standard DNS-SD, so every platform's mDNS hears it; a network that forbids multicast
    // leaves it hearing nothing rather than refusing everything else.
    let local = LocalLookup::start(endpoint.id(), service, advertise);
    if let Ok(lookup) = endpoint.address_lookup() {
        lookup.add(local.clone());
    }
    Ok((endpoint, local))
}

/// The device key's public half, as the model writes it.
pub(crate) fn node_id(id: EndpointId) -> NodeId {
    NodeId::from_bytes(*id.as_bytes())
}

/// The other way round.
pub(crate) fn endpoint_id(id: NodeId) -> Result<EndpointId> {
    EndpointId::from_bytes(id.as_bytes())
        .map_err(|_| SyncError::Protocol(format!("{id} is not a valid device key")))
}

/// This device's key, minted into the store the first time anything asks.
///
/// # Errors
///
/// If the store cannot be read or written.
pub fn device_key(store: &SharedStore) -> Result<SecretKey> {
    let bytes = lock(store)?.device_secret(|| SecretKey::generate().to_bytes())?;
    Ok(SecretKey::from_bytes(&bytes))
}

/// How one peer went in a [`Node::sync_all`].
#[derive(Debug)]
pub struct PeerResult {
    /// The peer.
    pub node_id: NodeId,
    /// What its device record calls it.
    pub name: String,
    /// The session's summary, or why there was none.
    pub outcome: Result<Summary>,
}

/// This device's endpoint.
#[derive(Clone)]
pub struct Node {
    endpoint: Endpoint,
    store: SharedStore,
    _local: LocalLookup,
    arrived: std::sync::Arc<tokio::sync::Notify>,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node").field("id", &self.endpoint.id()).finish_non_exhaustive()
    }
}

impl Node {
    /// Opens the endpoint under this device's key.
    ///
    /// # Errors
    ///
    /// If the key cannot be read or the endpoint cannot bind.
    pub async fn bind(store: SharedStore, network: Network) -> Result<Self> {
        let secret = device_key(&store)?;
        let (endpoint, local) = bind(secret, ALPN_SYNC, network, DEVICE_SERVICE, true).await?;
        Ok(Self { endpoint, store, _local: local, arrived: std::sync::Arc::default() })
    }

    /// This device's identifier.
    #[must_use]
    pub fn id(&self) -> NodeId {
        node_id(self.endpoint.id())
    }

    /// Where this endpoint can be reached right now — for tests, which have no lookup service.
    #[must_use]
    pub fn addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    /// Syncs every document with one paired device.
    ///
    /// # Errors
    ///
    /// If the peer is not in `devices`, cannot be reached, or the session fails.
    pub async fn sync_with(&self, peer: impl Into<EndpointAddr>) -> Result<Summary> {
        let addr = peer.into();
        self.require_trusted(addr.id)?;
        let conn = self
            .endpoint
            .connect(addr, ALPN_SYNC)
            .await
            // The common reasons are the ordinary ones — the device is off, asleep, or not
            // running sync — so say that, and keep Iroh's own words for whoever is debugging.
            .map_err(|e| {
                SyncError::Network(format!(
                    "could not reach it; it may be off, asleep, or not running sync ({e})"
                ))
            })?;
        let (mut send, mut recv) = conn
            .open_bi()
            .await
            .map_err(|e| SyncError::Network(e.to_string()))?;
        let summary = session::run(&self.store, &mut recv, &mut send).await?;
        // Said as soon as the changes are in the store, not after the goodbyes below, which can
        // take seconds — a view showing the store should redraw when the data is there.
        if !summary.changed.is_empty() {
            self.arrived.notify_one();
        }
        finish(&mut send, &mut recv).await?;
        conn.close(0u32.into(), b"done");
        Ok(summary)
    }

    /// Syncs with every paired device that answers, each with its own time limit, and records
    /// how each went for `lum sync status`.
    pub async fn sync_all(&self) -> Vec<PeerResult> {
        // A device the direct link (a watch's to its phone) synced, with nothing changed here
        // since, has nothing to be sent: it counts as synced without a connection.
        let (peers, linked): (Vec<Peer>, Vec<Peer>) = match lock(&self.store) {
            Ok(mut store) => {
                let devices: Vec<Peer> = store
                    .snapshot()
                    .0
                    .devices
                    .values()
                    .filter(|d| d.node_id != self.id())
                    .map(|d| (d.node_id, d.name.clone()))
                    .collect();
                devices.into_iter().partition(|(id, _)| !store.linked_and_unchanged(&id.to_string()))
            }
            Err(_) => (Vec::new(), Vec::new()),
        };
        let mut results: Vec<PeerResult> = linked
            .into_iter()
            .map(|(node_id, name)| PeerResult { node_id, name, outcome: Ok(Summary::default()) })
            .collect();
        let mut tasks = Vec::new();
        for (node_id, name) in peers {
            let node = self.clone();
            tasks.push(n0_future::task::spawn(async move {
                let outcome = match endpoint_id(node_id) {
                    Ok(id) => n0_future::time::timeout(PEER_TIMEOUT, node.sync_with(id))
                        .await
                        .unwrap_or_else(|_| {
                            Err(SyncError::Network("it did not answer in time".to_owned()))
                        }),
                    Err(error) => Err(error),
                };
                node.record(node_id, &outcome);
                PeerResult { node_id, name, outcome }
            }));
        }
        for task in tasks {
            if let Ok(result) = task.await {
                results.push(result);
            }
        }
        results
    }

    /// Answers paired devices until the endpoint closes. Each connection is handled on its own
    /// task, so a slow peer holds up nobody else.
    pub async fn serve(&self) {
        while let Some(incoming) = self.endpoint.accept().await {
            let node = self.clone();
            n0_future::task::spawn(async move {
                let Ok(accepting) = incoming.accept() else { return };
                let Ok(conn) = accepting.await else { return };
                let peer = node_id(conn.remote_id());
                let outcome = node.answer(&conn).await;
                if !matches!(outcome, Err(SyncError::Refused(_))) {
                    node.record(peer, &outcome);
                }
            });
        }
    }

    async fn answer(&self, conn: &Connection) -> Result<Summary> {
        if let Err(refused) = self.require_trusted(conn.remote_id()) {
            conn.close(1u32.into(), b"not paired");
            return Err(refused);
        }
        let (mut send, mut recv) =
            conn.accept_bi().await.map_err(|e| SyncError::Network(e.to_string()))?;
        let summary = session::run(&self.store, &mut recv, &mut send).await?;
        // Said as soon as the changes are in the store, not after the goodbyes below, which can
        // take seconds — a view showing the store should redraw when the data is there.
        if !summary.changed.is_empty() {
            self.arrived.notify_one();
        }
        finish(&mut send, &mut recv).await?;
        // The dialler closes once it has everything; waiting for that keeps this side from
        // tearing down data still in flight.
        let _ = n0_future::time::timeout(Duration::from_secs(5), conn.closed()).await;
        Ok(summary)
    }

    fn require_trusted(&self, peer: EndpointId) -> Result<()> {
        let peer = node_id(peer);
        let store = lock(&self.store)?;
        if peer != self.id() && store.snapshot().0.devices.contains_key(&peer) {
            Ok(())
        } else {
            Err(SyncError::Refused(format!(
                "{peer} is not one of your paired devices, so nothing was synced with it"
            )))
        }
    }

    fn record(&self, peer: NodeId, outcome: &Result<Summary>) {
        if let Ok(mut store) = lock(&self.store) {
            let error = outcome.as_ref().err().map(ToString::to_string);
            let _ = store.record_peer(&peer.to_string(), error.as_deref());
        }
    }

    /// Signalled whenever a peer that dialled in brought changes, so whatever is driving this
    /// node can pass them on to the other devices — this is how an edit made on the phone
    /// reaches the laptop through the Pi without either being awake at the same time.
    #[must_use]
    pub fn arrivals(&self) -> std::sync::Arc<tokio::sync::Notify> {
        std::sync::Arc::clone(&self.arrived)
    }

    /// Closes the endpoint, letting peers know rather than leaving them to time out.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

/// Ends a session cleanly on either side: finish sending, then read the other side to its
/// end, so nothing either side wrote is discarded by a close that comes too soon.
pub(crate) async fn finish(send: &mut SendStream, recv: &mut RecvStream) -> Result<()> {
    send.finish().map_err(|e| SyncError::Network(e.to_string()))?;
    recv.read_to_end(1024).await.map_err(|e| SyncError::Network(e.to_string()))?;
    // Having the peer's end does not mean the peer has ours: it may still be in flight, and
    // closing the connection now would drop it, so the peer reads a lost connection where it
    // should read a finished one. Over a relay that gap is long enough to lose every time.
    // `stopped` resolves once the peer has read this stream to its end.
    let _ = n0_future::time::timeout(Duration::from_secs(5), send.stopped()).await;
    Ok(())
}

/// A paired device, by its key and what it is called.
type Peer = (NodeId, String);
