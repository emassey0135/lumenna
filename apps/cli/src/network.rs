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

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anstream::eprint;
use anstream::eprintln;
use lumenna_surface::{LumennaError, PairingPrompt, Reach, SyncListener};

use crate::api::Response;
use crate::error::{CliError, Result};
use crate::profile::Profile;

pub(crate) const fn network(local_only: bool) -> Reach {
    if local_only { Reach::LocalOnly } else { Reach::Internet }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Message(format!("could not start the runtime: {e}")))
}

/// Whether a process on this device holds the sync endpoint — the daemon, usually.
pub(crate) fn endpoint_held(profile: &Profile) -> Result<bool> {
    Ok(profile.endpoint_held()?)
}

fn default_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = host.strip_suffix(".local").unwrap_or(&host).trim().to_owned();
    if host.is_empty() { "this device".to_owned() } else { host }
}

fn platform() -> &'static str {
    if Path::new("/BTSpeak").exists() { "btspeak" } else { std::env::consts::OS }
}

/// Pairing at a terminal: the code and the question go to standard error, so that standard
/// output stays the response.
struct TerminalPrompt {
    name: String,
}

impl PairingPrompt for TerminalPrompt {
    fn show_code(&self, code: String) {
        eprintln!(
            "Waiting to pair, as {}. On the other device, run `lum pair` too while on this \
             network. If it is on a different network, run this there instead:\nlum pair {code}",
            self.name
        );
    }

    fn confirm(&self, words: Vec<String>) -> bool {
        eprintln!("The words are: {}.", words.join(", "));
        eprint!("Do the same three words show on the other device? Type yes or no: ");
        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        matches!(answer.trim().to_lowercase().as_str(), "yes" | "y")
    }

    fn is_cancelled(&self) -> bool {
        // Control-C ends the process, which is how a terminal cancels.
        false
    }
}

/// The daemon has nothing to redraw when something arrives.
struct Quietly;

impl SyncListener for Quietly {
    fn changed(&self) {}
}

// ---------------------------------------------------------------------------------------
// Pairing and syncing
// ---------------------------------------------------------------------------------------

/// `lum pair`: finds or dials the other device, compares words, and syncs.
pub(crate) fn pair(profile: &Profile, code: Option<&str>, local_only: bool) -> Result<Response> {
    let name = default_name();
    if code.is_some() {
        eprintln!("Connecting to the other device...");
    }
    let prompt = Arc::new(TerminalPrompt { name: name.clone() });
    Ok(Response::new(profile.pair(
        code.map(ToOwned::to_owned),
        network(local_only),
        name,
        platform().to_owned(),
        prompt,
    )?))
}

/// `lum sync`: one round with every paired device — or, when the daemon holds the endpoint,
/// a request to it to do the round.
pub(crate) fn sync_once(profile: &Profile, local_only: bool) -> Result<Response> {
    match profile.sync_now(network(local_only)) {
        Ok(report) => {
            let response = Response::new(report);
            Ok(if response.announcement().contains("not paired") {
                response.note("`lum pair` pairs one")
            } else {
                response
            })
        }
        Err(LumennaError::SyncElsewhere { .. }) => ask_daemon_to_sync(profile),
        Err(error) => Err(error.into()),
    }
}

/// Another process holds the endpoint — normally the daemon. Asks it to sync now.
// The daemon's socket is Unix-only until Windows named pipes exist (§8).
#[cfg_attr(not(unix), allow(unused_variables))]
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
            let report: lumenna_surface::SyncReport =
                serde_json::from_value(reply.get("result").cloned().unwrap_or_default())
                    .map_err(|e| CliError::Message(format!("the daemon's answer did not read: {e}")))?;
            return Ok(Response::new(report));
        }
    }
    Ok(Response::unchanged(
        "another process is running sync for this profile, so it is already syncing; \
         `lum sync status` says how it is going",
    ))
}

/// `lum sync-daemon`: holds the endpoint and keeps this device in sync until stopped.
pub(crate) fn daemon(profile: &Profile, local_only: bool) -> Result<()> {
    let service = match profile.start_sync(network(local_only), Arc::new(Quietly)) {
        Err(LumennaError::SyncElsewhere { .. }) => {
            return Err(CliError::Message(
                "a sync daemon is already running for this profile".to_owned(),
            ));
        }
        started => started?,
    };
    eprintln!("lum: syncing as {}", profile.sync_status()?.this_device);

    // `lum sync` and the RPC `sync` method reach the running endpoint through this.
    let hook: crate::rpc::SyncHook = {
        let service = Arc::clone(&service);
        Arc::new(move || Ok(Response::new(service.sync_now()?)))
    };
    let _socket = serve_socket(profile, hook);

    runtime()?.block_on(async {
        // §9: a resident process takes a backup when one is due, hourly.
        let mut hourly = tokio::time::interval(Duration::from_secs(60 * 60));
        hourly.tick().await;
        loop {
            tokio::select! {
                _ = hourly.tick() => crate::durability::back_up_if_due(profile),
                () = stop_requested() => break,
            }
        }
    });
    service.stop();
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
    hook: crate::rpc::SyncHook,
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
    _hook: crate::rpc::SyncHook,
) -> Option<std::thread::JoinHandle<()>> {
    // Named pipes on Windows are still to come (§8); clients spawn `lum rpc` there.
    None
}

// ---------------------------------------------------------------------------------------
// Status and devices
// ---------------------------------------------------------------------------------------

/// `lum sync status`: §9's sync status, as sentences rather than an icon.
pub(crate) fn status(profile: &Profile) -> Result<Response> {
    let response = Response::new(profile.sync_status()?);
    Ok(if response.announcement().starts_with("Not paired") {
        response.note("`lum pair` pairs one")
    } else {
        response
    })
}

/// `lum device list`.
pub(crate) fn list_devices(profile: &Profile) -> Result<Response> {
    Ok(Response::new(profile.devices()?))
}

/// `lum device rename`.
pub(crate) fn rename_device(profile: &Profile, input: &str, name: &str) -> Result<Response> {
    Ok(Response::new(profile.rename_device(input, name)?))
}

/// `lum device unpair`.
pub(crate) fn unpair_device(profile: &Profile, input: &str) -> Result<Response> {
    Ok(Response::new(profile.unpair_device(input)?))
}
