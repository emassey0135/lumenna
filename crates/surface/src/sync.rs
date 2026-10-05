//! Pairing and syncing (§7, §8): what the command line, the daemon and the apps share.
//!
//! The protocol is `lumenna-sync`'s. What is here is how a client drives it: pairing with a
//! person confirming the words, a round with every paired device, the device list, and a
//! [`SyncService`] that keeps a device in sync for as long as something stays running — the
//! daemon on a desktop, the app while it is open on a phone.
//!
//! # One endpoint per device
//!
//! Every process on a device is the same device — the key lives in the store — but only one
//! may answer for that key at a time. An advisory lock on `sync.lock` in the profile decides
//! which (§8). A [`SyncService`] holds it while it runs; [`Lumenna::sync_now`] takes it for one
//! round, or hands the round to the service, or says another process holds it
//! ([`LumennaError::SyncElsewhere`]) so that a client which can reach that process — the CLI,
//! over the daemon's socket — can ask it instead.
//!
//! **Pairing needs no lock.** It runs on an endpoint of its own with a key minted for it, so a
//! person can pair while the service carries on syncing.

use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use lumenna_core::edit;
use lumenna_core::id::NodeId;
use lumenna_core::model::Device;
use lumenna_store::ChangeHash;
use lumenna_sync::invite::{Invitation, identity};
use lumenna_sync::node::{Network, Node, PeerResult};
use lumenna_sync::{SharedStore, SyncError};

use crate::error::{LumennaError, Result};
use crate::tasks::record_or;
use crate::types::{
    Announced, Change, DeviceList, DeviceView, PairedWith, PeerSync, Reach, SyncReport,
    SyncStatus,
};
use crate::words::count_line;
use crate::{Lumenna, repaired};

/// How long a pairing waits for the other device.
const PAIR_WAIT: Duration = Duration::from_secs(10 * 60);

/// How often a running service syncs with everyone even when nothing changed here, so that
/// an edit made on another device while this one was out of reach still arrives.
const FULL_ROUND: Duration = Duration::from_secs(5 * 60);

/// How often a running service looks for local changes to send on.
const TICK: Duration = Duration::from_secs(1);

impl From<SyncError> for LumennaError {
    fn from(error: SyncError) -> Self {
        Self::new(error.to_string())
    }
}

impl From<Reach> for Network {
    fn from(reach: Reach) -> Self {
        match reach {
            Reach::Internet => Self::Internet,
            Reach::LocalOnly => Self::LocalOnly,
        }
    }
}

/// What a pairing asks the person, through whatever the client has — a terminal, a dialog.
#[cfg_attr(feature = "uniffi", uniffi::export(with_foreign))]
pub trait PairingPrompt: Send + Sync {
    /// The code to give the other device, when this one is the one waiting to be found.
    fn show_code(&self, code: String);

    /// Whether the same words show on the other device. Called off the main thread, and may
    /// block until the person answers.
    fn confirm(&self, words: Vec<String>) -> bool;

    /// Whether the person has given up, which ends the wait for the other device.
    fn is_cancelled(&self) -> bool;
}

/// What a running [`SyncService`] tells its client.
#[cfg_attr(feature = "uniffi", uniffi::export(with_foreign))]
pub trait SyncListener: Send + Sync {
    /// Something arrived from another device; whatever shows the store should read it again.
    fn changed(&self);
}

/// The sync state of one open store: the running service, if there is one.
#[derive(Default)]
pub(crate) struct SyncState {
    service: Mutex<Weak<SyncService>>,
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| LumennaError::new(format!("could not start the network runtime: {e}")))
}

/// Takes the sync lock, or `None` if another process holds it.
fn take_lock(profile: &Path) -> Result<Option<File>> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(profile.join("sync.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

fn this_node(store: &SharedStore) -> Result<NodeId> {
    let key = lumenna_sync::node::device_key(store)?;
    Ok(NodeId::from_bytes(*key.public().as_bytes()))
}

/// Says how a round went, device by device.
fn report(results: Vec<PeerResult>) -> SyncReport {
    let peers: Vec<PeerSync> = results
        .into_iter()
        .map(|result| match result.outcome {
            Ok(summary) => PeerSync {
                name: result.name,
                node_id: result.node_id.to_string(),
                synced: true,
                changed: summary.changed.iter().map(|id| id.name()).collect(),
                error: None,
            },
            Err(error) => PeerSync {
                name: result.name,
                node_id: result.node_id.to_string(),
                synced: false,
                changed: Vec::new(),
                error: Some(error.to_string()),
            },
        })
        .collect();
    let reached = peers.iter().filter(|p| p.synced).count();
    let announcement = if peers.is_empty() {
        "This device is not paired with any other yet".to_owned()
    } else {
        format!("Synced with {reached} of {}", count_line(peers.len(), "device"))
    };
    SyncReport { announcement, notices: Vec::new(), peers }
}

fn brought_anything(report: &SyncReport) -> bool {
    report.peers.iter().any(|peer| !peer.changed.is_empty())
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Pairs this device with another of the person's (§7).
    ///
    /// Without a `code`, this device waits to be found — on the local network, or by the code
    /// it hands to [`PairingPrompt::show_code`]. With one, it dials the device that printed
    /// it. Both devices show the same three words, and only if the person says they match on
    /// both are the devices written into each other's lists. A first sync follows over the
    /// same connection, so the new device has everything before this returns.
    ///
    /// `name` and `platform` describe this device to the other, unless it already has a name
    /// from an earlier pairing. Blocks for as long as the pairing takes, up to ten minutes.
    ///
    /// # Errors
    ///
    /// If the code does not read, nobody turns up, either person says no, or the network or
    /// store fails.
    pub fn pair(
        &self,
        code: Option<String>,
        reach: Reach,
        name: String,
        platform: String,
        prompt: Arc<dyn PairingPrompt>,
    ) -> Result<PairedWith> {
        let peer = code
            .map(|code| {
                code.trim().parse::<iroh::EndpointId>().map_err(|_| {
                    LumennaError::new(format!(
                        "'{code}' is not a pairing code; it is the long code the other \
                         device shows while it waits"
                    ))
                })
            })
            .transpose()?;
        let store = self.shared();
        let me = this_node(&store)?;
        let known = repaired(&self.store()).devices.get(&me).map(|d| d.name.clone());
        let me = identity(&store, &known.unwrap_or(name), &platform)?;

        let paired = runtime()?.block_on(async {
            let session = Invitation::open(reach.into(), peer.is_none()).await?;
            if peer.is_none() {
                prompt.show_code(session.code());
            }
            let cancelled = {
                let prompt = Arc::clone(&prompt);
                async move {
                    while !prompt.is_cancelled() {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
            };
            let met = tokio::select! {
                met = tokio::time::timeout(PAIR_WAIT, session.meet(peer.map(iroh::EndpointAddr::new))) => met,
                () = cancelled => {
                    session.close().await;
                    return Err(SyncError::NotPaired("pairing was cancelled".to_owned()));
                }
            };
            let met = met.map_err(|_| {
                SyncError::NotPaired("no other device turned up in ten minutes".to_owned())
            });
            let (conn, role) = match met {
                Ok(Ok(met)) => met,
                Ok(Err(error)) | Err(error) => {
                    session.close().await;
                    return Err(error);
                }
            };
            let asking = Arc::clone(&prompt);
            let result = session
                .pair(&conn, role, &store, me, |words| async move {
                    tokio::task::spawn_blocking(move || asking.confirm(words))
                        .await
                        .unwrap_or(false)
                })
                .await;
            session.close().await;
            result
        })?;

        Ok(PairedWith {
            announcement: format!(
                "Paired with {}. Synced {}, {} brought in changes.",
                paired.peer.name,
                count_line(paired.summary.documents, "document"),
                paired.summary.changed.len()
            ),
            notices: Vec::new(),
            name: paired.peer.name,
            platform: paired.peer.platform,
            node_id: paired.peer.node_id,
        })
    }

    /// Syncs with every paired device now.
    ///
    /// The round runs on this store's [`SyncService`] if one is running, and otherwise on an
    /// endpoint opened for it and closed after.
    ///
    /// # Errors
    ///
    /// [`LumennaError::SyncElsewhere`] if another process holds this device's endpoint;
    /// otherwise if the endpoint cannot be opened. A device that cannot be reached is not an
    /// error: the report says so.
    pub fn sync_now(&self, reach: Reach) -> Result<SyncReport> {
        if let Some(service) = self.running_service() {
            return service.sync_now();
        }
        let Some(_lock) = take_lock(&self.directory)? else {
            return Err(LumennaError::SyncElsewhere {
                reason: "another process is syncing this device already".to_owned(),
            });
        };
        let store = self.shared();
        let results = runtime()?.block_on(async {
            let node = Node::bind(store, reach.into()).await?;
            // Answer while dialling, so two devices syncing at once still meet.
            let serving = node.clone();
            let server = tokio::spawn(async move { serving.serve().await });
            let results = node.sync_all().await;
            node.close().await;
            server.abort();
            Ok::<_, SyncError>(results)
        })?;
        Ok(report(results))
    }

    /// How syncing is going, device by device — §9's status in words rather than an icon.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn sync_status(&self) -> Result<SyncStatus> {
        let running = self.endpoint_held()?;
        let (me, devices) = self.device_views()?;
        let others = devices.iter().filter(|d| !d.this_device).count();
        let announcement = match (running, others) {
            (_, 0) => "Not paired with any other device yet".to_owned(),
            (true, n) => format!("Sync is running, with {}", count_line(n, "other device")),
            (false, n) => format!(
                "Sync is not running; {} will catch up at the next sync",
                count_line(n, "other device")
            ),
        };
        Ok(SyncStatus {
            announcement,
            notices: Vec::new(),
            running,
            this_device: me.to_string(),
            devices,
        })
    }

    /// The paired devices, this one first.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn devices(&self) -> Result<DeviceList> {
        let (_, devices) = self.device_views()?;
        Ok(DeviceList {
            announcement: count_line(devices.len(), "paired device"),
            notices: Vec::new(),
            devices,
        })
    }

    /// Renames a device, found by name or the start of its identifier.
    ///
    /// # Errors
    ///
    /// If no device, or more than one, matches.
    pub fn rename_device(&self, device: &str, name: &str) -> Result<Change> {
        let found = self.find_device(device)?;
        self.with(|store| {
            let change = edit::rename_device(&repaired(store), found.node_id, name)?;
            record_or(store, &change, "it already has that name")
        })
    }

    /// Stops syncing with a device. It keeps what it already has.
    ///
    /// # Errors
    ///
    /// If no device, or more than one, matches; or it is this device.
    pub fn unpair_device(&self, device: &str) -> Result<Change> {
        let found = self.find_device(device)?;
        if found.node_id == this_node(&self.shared())? {
            return Err(LumennaError::new(
                "this device cannot unpair itself; unpair it from one of your other devices",
            ));
        }
        let change = self.with(|store| {
            let change = edit::unpair_device(&repaired(store), found.node_id)?;
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })?;
        // §7: say plainly what unpairing does not do.
        Ok(change.note(format!(
            "{} keeps everything it already has. Unpairing is for a device you replaced; if it \
             was lost or stolen, unpairing alone does not take your data back from it",
            found.name
        )))
    }

    /// Starts keeping this device in sync, for as long as the returned service lives.
    ///
    /// The service holds this device's endpoint: it answers other devices, sends local
    /// changes on within a second or so, and syncs with everyone every few minutes regardless.
    /// What arrives is announced through `listener`.
    ///
    /// # Errors
    ///
    /// [`LumennaError::SyncElsewhere`] if another process holds the endpoint; otherwise if it
    /// cannot be opened.
    pub fn start_sync(
        &self,
        reach: Reach,
        listener: Arc<dyn SyncListener>,
    ) -> Result<Arc<SyncService>> {
        if let Some(service) = self.running_service() {
            return Ok(service);
        }
        let Some(lock) = take_lock(&self.directory)? else {
            return Err(LumennaError::SyncElsewhere {
                reason: "another process is syncing this device already".to_owned(),
            });
        };
        let service = SyncService::start(self.shared(), reach, listener, lock)?;
        *self.sync.service.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
            Arc::downgrade(&service);
        Ok(service)
    }
}

impl Lumenna {
    /// Whether anything on this device holds the sync endpoint — this store's own service,
    /// or another process. Taking the lock to find out and letting go at once changes
    /// nothing.
    ///
    /// # Errors
    ///
    /// If the lock file cannot be opened.
    pub fn endpoint_held(&self) -> Result<bool> {
        Ok(self.running_service().is_some() || take_lock(&self.directory)?.is_none())
    }

    fn running_service(&self) -> Option<Arc<SyncService>> {
        self.sync
            .service
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .upgrade()
            .filter(|service| service.is_running())
    }

    fn device_views(&self) -> Result<(NodeId, Vec<DeviceView>)> {
        let me = this_node(&self.shared())?;
        let (snapshot, peers) = self.with(|store| Ok((repaired(store), store.peers()?)))?;
        let when = |millis: i64| {
            jiff::Timestamp::from_millisecond(millis).map_or_else(|_| String::new(), |t| t.to_string())
        };
        let mut views: Vec<DeviceView> = snapshot
            .devices
            .values()
            .map(|device| {
                let status = peers.iter().find(|p| p.node_id == device.node_id.to_string());
                DeviceView {
                    name: device.name.clone(),
                    platform: device.platform.clone(),
                    node_id: device.node_id.to_string(),
                    this_device: device.node_id == me,
                    paired_at: device.paired_at.to_string(),
                    last_attempt: status.and_then(|s| s.last_attempt).map(when),
                    last_success: status.and_then(|s| s.last_success).map(when),
                    last_error: status.and_then(|s| s.last_error.clone()),
                }
            })
            .collect();
        views.sort_by(|a, b| b.this_device.cmp(&a.this_device).then(a.name.cmp(&b.name)));
        Ok((me, views))
    }

    fn find_device(&self, input: &str) -> Result<Device> {
        let snapshot = self.with(|store| Ok(repaired(store)))?;
        let lowered = input.to_lowercase();
        let matches: Vec<&Device> = snapshot
            .devices
            .values()
            .filter(|d| {
                d.name.to_lowercase() == lowered || d.node_id.to_string().starts_with(&lowered)
            })
            .collect();
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(LumennaError::new(format!("no paired device called '{input}'"))),
            _ => Err(LumennaError::new(format!(
                "'{input}' matches more than one device; give more of its identifier"
            ))),
        }
    }
}

/// Keeps a device in sync while it runs: the daemon's loop, and the app's while it is open.
#[cfg_attr(feature = "uniffi", derive(uniffi::Object))]
pub struct SyncService {
    requests: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Sender<SyncReport>>,
    stop: tokio::sync::watch::Sender<bool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl SyncService {
    /// Syncs with every paired device now, on the running endpoint.
    ///
    /// # Errors
    ///
    /// If the service has stopped.
    pub fn sync_now(&self) -> Result<SyncReport> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.requests
            .blocking_send(reply)
            .map_err(|_| LumennaError::new("syncing has stopped"))?;
        answer.blocking_recv().map_err(|_| LumennaError::new("syncing stopped mid-round"))
    }

    /// Stops: closes the endpoint and lets go of the lock. Waits for that to finish, so a
    /// service started straight afterwards can take the lock.
    pub fn stop(&self) {
        let _ = self.stop.send(true);
        let thread = self.thread.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }

    /// Whether it is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !*self.stop.borrow()
            && self
                .thread
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|thread| !thread.is_finished())
    }
}

impl Drop for SyncService {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

impl SyncService {
    fn start(
        store: SharedStore,
        reach: Reach,
        listener: Arc<dyn SyncListener>,
        lock: File,
    ) -> Result<Arc<Self>> {
        let (requests, asked) = tokio::sync::mpsc::channel(8);
        let (stop, stopping) = tokio::sync::watch::channel(false);
        let (bound, started) = std::sync::mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("lumenna-sync".to_owned())
            .spawn(move || {
                // Held for as long as the endpoint is open, and dropped with it.
                let _lock = lock;
                let runtime = match runtime() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = bound.send(Err(error));
                        return;
                    }
                };
                runtime.block_on(run(store, reach, listener, asked, stopping, bound));
            })
            .map_err(|e| LumennaError::new(format!("could not start syncing: {e}")))?;
        // Opening the endpoint is the part that can fail; wait for it, so the caller hears.
        started
            .recv()
            .map_err(|_| LumennaError::new("syncing stopped before it started"))??;
        Ok(Arc::new(Self { requests, stop, thread: Mutex::new(Some(thread)) }))
    }
}

/// The loop: answer, send local changes on, catch up periodically, and do rounds on request.
async fn run(
    store: SharedStore,
    reach: Reach,
    listener: Arc<dyn SyncListener>,
    mut asked: tokio::sync::mpsc::Receiver<tokio::sync::oneshot::Sender<SyncReport>>,
    mut stopping: tokio::sync::watch::Receiver<bool>,
    bound: std::sync::mpsc::Sender<Result<()>>,
) {
    let node = match Node::bind(Arc::clone(&store), reach.into()).await {
        Ok(node) => {
            let _ = bound.send(Ok(()));
            node
        }
        Err(error) => {
            let _ = bound.send(Err(error.into()));
            return;
        }
    };
    let serving = node.clone();
    let server = tokio::spawn(async move { serving.serve().await });
    let arrivals = node.arrivals();

    // What the documents held after the last round. Anything different — written here, by
    // this app or another process — is sent on at the next tick.
    let version = |store: &SharedStore| -> Option<Vec<ChangeHash>> {
        let mut store = store.lock().ok()?;
        // Takes in what other processes wrote; this connection's own writes are already in.
        store.refresh().ok()?;
        Some(store.version())
    };
    let mut synced = None;
    let mut dirty = true;
    let mut last_round = Instant::now();
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let now = version(&store);
                if dirty || now != synced || last_round.elapsed() >= FULL_ROUND {
                    let round = report(node.sync_all().await);
                    // What arrived from one peer goes on to the others next round; a round
                    // that brings nothing in ends the chain.
                    dirty = brought_anything(&round);
                    if dirty {
                        listener.changed();
                    }
                    synced = version(&store);
                    last_round = Instant::now();
                }
            }
            () = arrivals.notified() => {
                dirty = true;
                listener.changed();
            }
            Some(reply) = asked.recv() => {
                let round = report(node.sync_all().await);
                if brought_anything(&round) {
                    listener.changed();
                }
                synced = version(&store);
                last_round = Instant::now();
                let _ = reply.send(round);
            }
            _ = stopping.changed() => break,
        }
    }
    node.close().await;
    server.abort();
}
