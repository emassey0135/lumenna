//! `lum pair`, `lum sync`, `lum sync-daemon` and `lum device` (§7, §8).
//!
//! The protocol is in `lumenna-sync`; what is here is how a person at a terminal drives it,
//! and how processes on one device share it.
//!
//! # One endpoint per device, decided by a lock
//!
//! Every process on a device is the same device — the key lives in the store — but only one of
//! them may run the endpoint at a time, or two processes would answer for one key. The **sync
//! lock**, an advisory lock on `sync.lock` in the profile, decides which (§8). `lum
//! sync-daemon` holds it for as long as it runs; `lum sync` takes it for one round when nothing
//! else has it, and otherwise asks the daemon over its socket to sync now.
//!
//! **Pairing needs no lock.** It runs on an endpoint of its own (`lumenna_sync::invite`), so a
//! person can pair from a terminal while the daemon carries on syncing beside it.

use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anstream::eprint;
use anstream::eprintln;
use lumenna_core::edit;
use lumenna_core::id::NodeId;
use lumenna_core::model::Device;
use lumenna_store::Store;
use lumenna_sync::invite::{Invitation, identity};
use lumenna_sync::node::{Network, Node, PeerResult};
use lumenna_sync::{SharedStore, SyncError};

use lumenna_surface::words::count_line;

use crate::api::{self, Response};
use crate::error::{CliError, Result};
use crate::profile::Profile;

/// How long `lum pair` waits for the other device before giving up.
const PAIR_WAIT: Duration = Duration::from_secs(10 * 60);

/// How often the daemon syncs with everyone even when nothing has changed here, so that an
/// edit made on another device while this one was out of reach still arrives.
const FULL_ROUND: Duration = Duration::from_secs(5 * 60);

impl From<SyncError> for CliError {
    fn from(error: SyncError) -> Self {
        Self::Message(error.to_string())
    }
}

pub(crate) const fn network(local_only: bool) -> Network {
    if local_only { Network::LocalOnly } else { Network::Internet }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Message(format!("could not start the network runtime: {e}")))
}

/// A second connection to the store, shareable between the network's tasks. A process may
/// hold several; that is the multi-process design of §8 working as intended.
fn shared(profile: &Profile) -> Result<SharedStore> {
    Ok(Arc::new(Mutex::new(Store::open(&profile.store_path())?)))
}

fn lock_store(store: &SharedStore) -> Result<std::sync::MutexGuard<'_, Store>> {
    store.lock().map_err(|_| CliError::Message("the store is wedged".to_owned()))
}

/// Takes the sync lock, or `None` if another process holds it.
fn take_lock(profile: &Profile) -> Result<Option<File>> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(profile.directory().join("sync.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

/// Whether a process on this device holds the sync endpoint — the daemon, usually. Taking
/// the lock to find out and letting go at once changes nothing.
pub(crate) fn endpoint_held(profile: &Profile) -> Result<bool> {
    Ok(take_lock(profile)?.is_none())
}

/// What this device is called, and what it runs.
fn this_device(store: &SharedStore) -> Result<(String, String)> {
    let key = lumenna_sync::node::device_key(store)?;
    let me = NodeId::from_bytes(*key.public().as_bytes());
    let existing = lock_store(store)?.snapshot().0.devices.get(&me).map(|d| d.name.clone());
    Ok((existing.unwrap_or_else(default_name), platform().to_owned()))
}

fn default_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = host.strip_suffix(".local").unwrap_or(&host).trim().to_owned();
    if host.is_empty() { "this device".to_owned() } else { host }
}

fn platform() -> &'static str {
    if Path::new("/BTSpeak").exists() { "btspeak" } else { std::env::consts::OS }
}

/// Asks at the terminal whether the words match. On standard error, so that standard output
/// stays the response.
fn ask(words: &[String]) -> bool {
    eprintln!("The words are: {}.", words.join(", "));
    eprint!("Do the same three words show on the other device? Type yes or no: ");
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "yes" | "y")
}

// ---------------------------------------------------------------------------------------
// Pairing
// ---------------------------------------------------------------------------------------

/// `lum pair`: finds or dials the other device, compares words, and syncs.
pub(crate) fn pair(profile: &Profile, code: Option<&str>, local_only: bool) -> Result<Response> {
    let peer = code
        .map(|code| {
            code.trim().parse::<iroh::EndpointId>().map_err(|_| {
                CliError::Message(format!(
                    "'{code}' is not a pairing code; it is the long code `lum pair` prints"
                ))
            })
        })
        .transpose()?;
    let store = shared(profile)?;
    let (name, platform) = this_device(&store)?;
    let me = identity(&store, &name, &platform)?;

    let paired = runtime()?.block_on(async {
        let session = Invitation::open(network(local_only), peer.is_none()).await?;
        match peer {
            None => eprintln!(
                "Waiting to pair, as {name}. On the other device, run `lum pair` too while on \
                 this network. If it is on a different network, run this there instead:\n\
                 lum pair {}",
                session.code()
            ),
            Some(_) => eprintln!("Connecting to the other device..."),
        }
        let met = tokio::time::timeout(PAIR_WAIT, session.meet(peer.map(iroh::EndpointAddr::new)))
            .await
            .map_err(|_| {
                SyncError::NotPaired("no other device turned up in ten minutes".to_owned())
            })?;
        let (conn, role) = met?;
        let result = session
            .pair(&conn, role, &store, me, |words| async move {
                tokio::task::spawn_blocking(move || ask(&words)).await.unwrap_or(false)
            })
            .await;
        session.close().await;
        result
    })?;

    let changed = paired.summary.changed.len();
    Ok(Response::new(api::PairedWith {
        announcement: format!(
            "Paired with {}. Synced {}, {} brought in changes.",
            paired.peer.name,
            count_line(paired.summary.documents, "document"),
            changed
        ),
        notices: Vec::new(),
        name: paired.peer.name,
        platform: paired.peer.platform,
        node_id: paired.peer.node_id,
    }))
}

// ---------------------------------------------------------------------------------------
// Syncing
// ---------------------------------------------------------------------------------------

fn report(results: Vec<PeerResult>) -> Response {
    let peers: Vec<api::PeerSync> = results
        .into_iter()
        .map(|result| match result.outcome {
            Ok(summary) => api::PeerSync {
                name: result.name,
                node_id: result.node_id.to_string(),
                synced: true,
                changed: summary.changed.iter().map(|id| id.name()).collect(),
                error: None,
            },
            Err(error) => api::PeerSync {
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
        "This device is not paired with any other yet; `lum pair` pairs one".to_owned()
    } else {
        format!("Synced with {reached} of {}", count_line(peers.len(), "device"))
    };
    Response::new(api::SyncReport { announcement, notices: Vec::new(), peers })
}

/// `lum sync`: one round with every paired device.
pub(crate) fn sync_once(profile: &Profile, local_only: bool) -> Result<Response> {
    let Some(_lock) = take_lock(profile)? else {
        return ask_daemon_to_sync(profile);
    };
    let store = shared(profile)?;
    let results = runtime()?.block_on(async {
        let node = Node::bind(store, network(local_only)).await?;
        // Answer while dialling, so two devices running `lum sync` at once still meet.
        let serving = node.clone();
        let server = tokio::spawn(async move { serving.serve().await });
        let results = node.sync_all().await;
        node.close().await;
        server.abort();
        Ok::<_, SyncError>(results)
    })?;
    Ok(report(results))
}

/// Another process holds the endpoint — normally the daemon. Asks it to sync now.
fn ask_daemon_to_sync(profile: &Profile) -> Result<Response> {
    #[cfg(unix)]
    {
        use std::io::{BufRead, BufReader, Write};
        if let Ok(mut socket) = std::os::unix::net::UnixStream::connect(profile.socket_path()) {
            socket.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"sync\"}\n")?;
            let mut line = String::new();
            BufReader::new(&socket).read_line(&mut line)?;
            let reply: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| CliError::Message(format!("the daemon's answer did not read: {e}")))?;
            if let Some(error) = reply.pointer("/error/message").and_then(|m| m.as_str()) {
                return Err(CliError::Message(error.to_owned()));
            }
            let result = reply.get("result").cloned().unwrap_or_default();
            let announcement = result
                .get("announcement")
                .and_then(|a| a.as_str())
                .unwrap_or("The sync daemon synced")
                .to_owned();
            let peers = serde_json::from_value(result.get("peers").cloned().unwrap_or_default())
                .unwrap_or_default();
            return Ok(Response::new(api::SyncReport { announcement, notices: Vec::new(), peers }));
        }
    }
    Ok(Response::unchanged(
        "another process is running sync for this profile, so it is already syncing; \
         `lum sync status` says how it is going",
    ))
}

/// `lum sync-daemon`: holds the endpoint and keeps this device in sync until stopped.
pub(crate) fn daemon(profile: &Profile, local_only: bool) -> Result<()> {
    let Some(_lock) = take_lock(profile)? else {
        return Err(CliError::Message(
            "a sync daemon is already running for this profile".to_owned(),
        ));
    };
    let store = shared(profile)?;
    let runtime = runtime()?;
    runtime.block_on(async {
        let node = Node::bind(store.clone(), network(local_only)).await?;
        eprintln!("lum: syncing as {}", node.id());
        let serving = node.clone();
        tokio::spawn(async move { serving.serve().await });

        // `lum sync` and the RPC `sync` method reach the endpoint through this.
        let (requests, mut asked) =
            tokio::sync::mpsc::channel::<tokio::sync::oneshot::Sender<Response>>(8);
        let _socket = serve_socket(profile, requests);

        let arrivals = node.arrivals();
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        let mut dirty = true;
        let mut last_round = Instant::now();
        let mut last_backup_check = Instant::now();
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    // Another process on this device wrote something.
                    if lock_store(&store)?.refresh()? {
                        dirty = true;
                    }
                    if dirty || last_round.elapsed() >= FULL_ROUND {
                        let results = node.sync_all().await;
                        // What arrived from one peer goes on to the others next round; a round
                        // that brings nothing in ends the chain.
                        dirty = results.iter().any(|r| {
                            r.outcome.as_ref().is_ok_and(|s| !s.changed.is_empty())
                        });
                        last_round = Instant::now();
                    }
                    if last_backup_check.elapsed() >= Duration::from_secs(3600) {
                        last_backup_check = Instant::now();
                        crate::durability::back_up_if_due(profile);
                    }
                }
                () = arrivals.notified() => dirty = true,
                Some(reply) = asked.recv() => {
                    let results = node.sync_all().await;
                    last_round = Instant::now();
                    let _ = reply.send(report(results));
                }
                () = stop_requested() => break,
            }
        }
        node.close().await;
        Ok::<(), CliError>(())
    })?;
    #[cfg(unix)]
    let _ = std::fs::remove_file(profile.socket_path());
    Ok(())
}

/// Control-C, or — on Unix — the SIGTERM a service manager sends to stop a daemon.
async fn stop_requested() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

/// Serves the command surface on the profile's socket (§8), one thread per client, each with
/// its own connection to the store. A `sync` request is passed to the daemon's loop.
#[cfg(unix)]
fn serve_socket(
    profile: &Profile,
    requests: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Sender<Response>>,
) -> Option<std::thread::JoinHandle<()>> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let path = profile.socket_path();
    // The lock is held, so any socket file here is a dead daemon's.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("lum: not serving {}: {error}", path.display());
            return None;
        }
    };
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    let directory = profile.directory().to_path_buf();
    let hook: crate::rpc::SyncHook = Arc::new(move || {
        let (reply, answer) = tokio::sync::oneshot::channel();
        requests
            .blocking_send(reply)
            .map_err(|_| CliError::Message("the sync daemon is stopping".to_owned()))?;
        answer
            .blocking_recv()
            .map_err(|_| CliError::Message("the sync daemon stopped mid-sync".to_owned()))
    });
    Some(std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (directory, hook) = (directory.clone(), Arc::clone(&hook));
            std::thread::spawn(move || {
                let Ok(reader) = stream.try_clone() else { return };
                let Ok(profile) = Profile::open(Some(&directory)) else { return };
                let _ = crate::rpc::serve_streams(
                    profile,
                    std::io::BufReader::new(reader),
                    Box::new(stream),
                    Some(hook),
                );
            });
        }
    }))
}

#[cfg(not(unix))]
fn serve_socket(
    _profile: &Profile,
    _requests: tokio::sync::mpsc::Sender<tokio::sync::oneshot::Sender<Response>>,
) -> Option<std::thread::JoinHandle<()>> {
    // Named pipes on Windows are still to come (§8); clients spawn `lum rpc` there.
    None
}

// ---------------------------------------------------------------------------------------
// Status and devices
// ---------------------------------------------------------------------------------------

fn device_views(profile: &Profile) -> Result<(NodeId, Vec<api::DeviceView>)> {
    let store = shared(profile)?;
    let key = lumenna_sync::node::device_key(&store)?;
    let me = NodeId::from_bytes(*key.public().as_bytes());
    let snapshot = profile.store().snapshot().0;
    let peers = profile.store().peers()?;
    let mut views: Vec<api::DeviceView> = snapshot
        .devices
        .values()
        .map(|device| {
            let status = peers.iter().find(|p| p.node_id == device.node_id.to_string());
            api::DeviceView {
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

fn when(millis: i64) -> String {
    jiff::Timestamp::from_millisecond(millis).map_or_else(|_| String::new(), |t| t.to_string())
}

/// `lum sync status`: §9's sync status, as sentences rather than an icon.
pub(crate) fn status(profile: &Profile) -> Result<Response> {
    // The lock being taken means something holds the endpoint: the daemon, or a `lum sync` in
    // progress. Taking it here and letting go at once changes nothing.
    let running = take_lock(profile)?.is_none();
    let (me, devices) = device_views(profile)?;
    let others = devices.iter().filter(|d| !d.this_device).count();
    let announcement = match (running, others) {
        (_, 0) => "Not paired with any other device yet; `lum pair` pairs one".to_owned(),
        (true, n) => format!("Sync is running, with {}", count_line(n, "other device")),
        (false, n) => format!(
            "Sync is not running; {} will catch up at the next `lum sync` or when the daemon starts",
            count_line(n, "other device")
        ),
    };
    Ok(Response::new(api::SyncStatus {
        announcement,
        notices: Vec::new(),
        running,
        this_device: me.to_string(),
        devices,
    }))
}

/// `lum device list`.
pub(crate) fn list_devices(profile: &Profile) -> Result<Response> {
    let (_, devices) = device_views(profile)?;
    let count = devices.len();
    Ok(Response::new(api::DeviceList {
        announcement: count_line(count, "paired device"),
        notices: Vec::new(),
        devices,
    }))
}

fn find_device(profile: &Profile, input: &str) -> Result<Device> {
    let snapshot = profile.store().snapshot().0;
    let lowered = input.to_lowercase();
    let matches: Vec<&Device> = snapshot
        .devices
        .values()
        .filter(|d| d.name.to_lowercase() == lowered || d.node_id.to_string().starts_with(&lowered))
        .collect();
    match matches.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(CliError::Message(format!("no paired device called '{input}'"))),
        _ => Err(CliError::Message(format!(
            "'{input}' matches more than one device; give more of its identifier"
        ))),
    }
}

/// `lum device rename`.
pub(crate) fn rename_device(profile: &Profile, input: &str, name: &str) -> Result<Response> {
    let device = find_device(profile, input)?;
    let change = edit::rename_device(&profile.store().snapshot().0, device.node_id, name)?;
    if change.is_empty() {
        return Ok(Response::unchanged("it already has that name"));
    }
    profile.store().apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

/// `lum device unpair`.
pub(crate) fn unpair_device(profile: &Profile, input: &str) -> Result<Response> {
    let device = find_device(profile, input)?;
    let store = shared(profile)?;
    let me = NodeId::from_bytes(*lumenna_sync::node::device_key(&store)?.public().as_bytes());
    if device.node_id == me {
        return Err(CliError::Message(
            "this device cannot unpair itself; unpair it from one of your other devices".to_owned(),
        ));
    }
    let change = edit::unpair_device(&profile.store().snapshot().0, device.node_id)?;
    profile.store().apply_recorded(&change)?;
    // §7: say plainly what unpairing does not do.
    Ok(Response::changed(&change).note(format!(
        "{} keeps everything it already has. Unpairing is for a device you replaced; if it was \
         lost or stolen, unpairing alone does not take your data back from it",
        device.name
    )))
}
