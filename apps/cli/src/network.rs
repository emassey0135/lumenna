//! `lum pair`, `lum sync`, `lum sync-daemon` and `lum device`.
//!
//! The protocol is in `lumenna-sync`; what is here is how a person at a terminal drives it,
//! and how processes on one device share it.
//!
//! # One endpoint per device, decided by a lock
//!
//! Every process on a device is the same device — the key lives in the store — but only one of
//! them may run the endpoint at a time, or two processes would answer for one key. The **sync
//! lock**, an advisory lock on `sync.lock` in the profile, decides which. `lum
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

pub(crate) fn default_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = host.strip_suffix(".local").unwrap_or(&host).trim().to_owned();
    if host.is_empty() { "this device".to_owned() } else { host }
}

pub(crate) fn platform() -> &'static str {
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

/// `lum sync`: one round with every paired device — on the endpoint the daemon or an app
/// holds, when one does.
pub(crate) fn sync_once(profile: &Profile, local_only: bool) -> Result<Response> {
    match profile.sync_now(network(local_only)) {
        Ok(report) => {
            let response = Response::new(report);
            Ok(if profile.at_terminal() && response.announcement().contains("not paired") {
                response.note("`lum pair` pairs one")
            } else {
                response
            })
        }
        // Held, and the holder could not be asked.
        Err(LumennaError::SyncElsewhere { .. }) => Ok(Response::unchanged(
            "another process is running sync for this profile, so it is already syncing; \
             `lum sync status` says how it is going",
        )),
        Err(error) => Err(error.into()),
    }
}

/// What is answering at the profile's address, by what it says it is: `daemon`, an app's
/// name, or nothing.
fn answering(profile: &Profile) -> Option<String> {
    use std::io::{BufRead, BufReader, Write};
    let mut connection = lumenna_surface::endpoint::connect(profile.directory())?;
    connection.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    connection.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n").ok()?;
    let mut line = String::new();
    BufReader::new(connection).read_line(&mut line).ok()?;
    let reply: serde_json::Value = serde_json::from_str(&line).ok()?;
    reply.pointer("/result/process").and_then(|p| p.as_str()).map(ToOwned::to_owned)
}

/// `lum sync-daemon`: holds the endpoint and keeps this device in sync until stopped,
/// serving the command surface at the profile's address while it does.
pub(crate) fn daemon(profile: &Profile, local_only: bool) -> Result<()> {
    // A second daemon is refused. An app holding the endpoint is waited for instead, and
    // taken over from when it quits.
    if answering(profile).as_deref() == Some("daemon") {
        return Err(CliError::Message("a sync daemon is already running for this profile".to_owned()));
    }
    let service = profile.start_sync(network(local_only), Arc::new(Quietly))?;
    if service.is_running() {
        eprintln!("lum: syncing as {}", profile.sync_status()?.this_device);
    } else {
        eprintln!("lum: another process is syncing this profile; this daemon takes over when it stops");
    }

    let host = lumenna_surface::rpc::Host {
        process: "daemon".to_owned(),
        device_name: default_name(),
        platform: platform().to_owned(),
        // A round asked of the daemon runs on the endpoint it holds.
        sync: Some({
            let service = Arc::clone(&service);
            Arc::new(move || service.sync_now())
        }),
        // The daemon takes them itself, below.
        backups: false,
    };
    let holding = Arc::clone(&service);
    let endpoint = lumenna_surface::rpc::Endpoint::serve(profile.directory().to_path_buf(), host, move || holding.is_running());

    runtime()?.block_on(async {
        // A resident process takes a backup when one is due, checking hourly.
        let mut hourly = tokio::time::interval(Duration::from_secs(60 * 60));
        hourly.tick().await;
        loop {
            tokio::select! {
                _ = hourly.tick() => crate::durability::back_up_if_due(profile),
                () = stop_requested() => break,
            }
        }
    });
    endpoint.stop();
    service.stop();
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

/// `lum rpc`: the command surface on stdio, for one client. When the daemon or an app holds
/// this profile's endpoint, it is relayed there, so the client reaches the running process
/// as a client that can open its address does; otherwise this serves it.
pub(crate) fn serve_rpc(profile: &Profile) -> Result<()> {
    if lumenna_surface::rpc::relay(profile.directory())? {
        return Ok(());
    }
    let host = lumenna_surface::rpc::Host {
        process: "lum rpc".to_owned(),
        device_name: default_name(),
        platform: platform().to_owned(),
        sync: None,
        // Something that stays running, so it takes the backup when one is due.
        backups: true,
    };
    lumenna_surface::rpc::serve_streams(
        profile.shared(),
        std::io::BufReader::new(std::io::stdin()),
        Box::new(std::io::stdout()),
        host,
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Status and devices
// ---------------------------------------------------------------------------------------

/// `lum sync status`: how sync is going, as sentences rather than an icon.
pub(crate) fn status(profile: &Profile) -> Result<Response> {
    let response = Response::new(profile.sync_status()?);
    Ok(if profile.at_terminal() && response.announcement().starts_with("Not paired") {
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
