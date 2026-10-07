//! Serving the surface at the profile's address while this process holds the endpoint.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::Lumenna;
use crate::endpoint::{self, Connection};

use super::server::{Host, serve_streams};

/// How often a process not holding the endpoint looks again, and a listener checks whether
/// it should stop.
const STEP: Duration = Duration::from_millis(250);

/// Serves the command surface at the profile's address for as long as this process holds the
/// sync endpoint, and gives the address up when it stops holding it — so whichever process
/// syncs is the one clients reach, and two never claim one address.
pub struct Endpoint {
    stopping: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Endpoint {
    /// Starts serving `directory`'s profile while `holding` says this process holds its
    /// endpoint: a `SyncService`'s `is_running`, which is false while it waits its turn.
    #[must_use]
    pub fn serve(directory: PathBuf, host: Host, holding: impl Fn() -> bool + Send + 'static) -> Self {
        let stopping = Arc::new(AtomicBool::new(false));
        let thread = {
            let stopping = Arc::clone(&stopping);
            std::thread::Builder::new()
                .name("lumenna-rpc".to_owned())
                .spawn(move || {
                    while !stopping.load(Ordering::Relaxed) {
                        if holding() {
                            listen(&directory, &host, &holding, &stopping);
                        }
                        std::thread::sleep(STEP);
                    }
                })
                .ok()
        };
        Self { stopping, thread }
    }

    /// Stops serving, and waits until the address is given up.
    pub fn stop(mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
    }
}

/// Serves one client: a store connection of its own, so what it writes reaches the process
/// serving it as another connection's write, which that process already redraws for.
fn serve_client(directory: &Path, host: &Host, reader: Connection, writer: Connection) {
    let Ok(lumenna) = Lumenna::open_at(directory) else { return };
    let _ = serve_streams(Arc::new(lumenna), BufReader::new(reader), Box::new(writer), host.clone());
}

/// Accepts clients at the address until this process stops holding the endpoint or is told
/// to stop.
#[cfg(unix)]
fn listen(directory: &Path, host: &Host, holding: &impl Fn() -> bool, stopping: &AtomicBool) {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let path = endpoint::address(directory);
    // This process holds the endpoint, so whatever is at the address is a dead holder's.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("lumenna: not serving {}: {error}", path.display());
            return;
        }
    };
    // Yours alone: anything that can reach it can read and change the whole store.
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    while holding() && !stopping.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let (Ok(reader), writer) = (stream.try_clone(), stream) else { continue };
                let (directory, host) = (directory.to_path_buf(), host.clone());
                std::thread::spawn(move || {
                    serve_client(&directory, &host, Connection::Socket(reader), Connection::Socket(writer));
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(STEP),
            Err(_) => break,
        }
    }
    let _ = std::fs::remove_file(&path);
}

/// Named pipes: see the Windows app's session, which writes and tests this.
#[cfg(windows)]
fn listen(directory: &Path, host: &Host, holding: &impl Fn() -> bool, stopping: &AtomicBool) {
    let _ = (directory, host, holding, stopping, serve_client as fn(&Path, &Host, Connection, Connection));
}

/// Relays stdin and stdout to whatever holds the profile's endpoint, until either side ends,
/// and returns whether anything was there to relay to.
///
/// What `lum rpc` does first: a client that starts a server of its own reaches a running app
/// or daemon this way — Emacs on Windows, above all, which cannot open a local socket or a
/// named pipe itself.
///
/// # Errors
///
/// If the connection fails once made.
pub fn relay(directory: &Path) -> std::io::Result<bool> {
    let Some(connection) = endpoint::connect(directory) else { return Ok(false) };
    let mut from_holder = connection.try_clone()?;
    let mut to_holder = connection;
    let out = std::thread::spawn(move || {
        let mut stdout = std::io::stdout();
        let _ = std::io::copy(&mut from_holder, &mut stdout);
    });
    let _ = std::io::copy(&mut std::io::stdin(), &mut to_holder);
    drop(to_holder);
    let _ = out.join();
    Ok(true)
}
