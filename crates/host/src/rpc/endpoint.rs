//! Serving the surface at the profile's address while this process holds the endpoint.

use std::io::{BufReader, Write};
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

/// Accepts clients at the profile's named pipe until this process stops holding the endpoint
/// or is told to stop.
///
/// Each client gets an instance of its own, and the next is made as soon as one connects. The
/// first instance is made with `FILE_FLAG_FIRST_PIPE_INSTANCE`, so a pipe someone else made
/// under this name first is refused rather than joined: clients would reach theirs. Waiting
/// for a client wakes every [`STEP`] to look whether to give the pipe up.
#[cfg(windows)]
fn listen(directory: &Path, host: &Host, holding: &impl Fn() -> bool, stopping: &AtomicBool) {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{FromRawHandle, OwnedHandle};

    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX};
    use windows_sys::Win32::System::Pipes::{
        CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
        PIPE_WAIT,
    };

    use crate::endpoint::Pipe;

    let path = endpoint::address(directory);
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let security = match owner_only::Security::new() {
        Ok(security) => security,
        Err(error) => {
            eprintln!("lumenna: not serving {}: {error}", path.display());
            return;
        }
    };
    let mut first = true;
    let keep_waiting = || holding() && !stopping.load(Ordering::Relaxed);
    while keep_waiting() {
        let mut open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
        if first {
            open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
        }
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                security.attributes(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            eprintln!("lumenna: not serving {}: {}", path.display(), std::io::Error::last_os_error());
            return;
        }
        first = false;
        let pipe = Pipe::new(unsafe { OwnedHandle::from_raw_handle(handle) });
        match pipe.accept(STEP, &keep_waiting) {
            Ok(true) => {
                let Ok(reader) = pipe.try_clone() else { continue };
                let (directory, host) = (directory.to_path_buf(), host.clone());
                std::thread::spawn(move || {
                    serve_client(&directory, &host, Connection::Pipe(reader), Connection::Pipe(pipe));
                });
            }
            // Told to stop, or no longer holding the endpoint: the waiting instance closes.
            Ok(false) => break,
            // A client that came and went before it was accepted: make the next instance.
            Err(_) => {}
        }
    }
}

/// A security descriptor letting only this process's user open the pipe: anything that can
/// reach it can read and change the whole store, as with the Unix socket's 0600.
///
/// The user's own SID, rather than `OW` (owner rights): an elevated process's objects are
/// owned by the Administrators group, which would let every administrator in.
#[cfg(windows)]
mod owner_only {
    use std::io;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{
        GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    pub(super) struct Security {
        descriptor: PSECURITY_DESCRIPTOR,
        attributes: SECURITY_ATTRIBUTES,
    }

    impl Security {
        pub(super) fn new() -> io::Result<Self> {
            let sid = user_sid()?;
            let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})").encode_utf16().chain(Some(0)).collect();
            let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            let made = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &raw mut descriptor,
                    std::ptr::null_mut(),
                )
            };
            if made == 0 {
                return Err(io::Error::last_os_error());
            }
            let attributes = SECURITY_ATTRIBUTES {
                nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
                lpSecurityDescriptor: descriptor,
                bInheritHandle: 0,
            };
            Ok(Self { descriptor, attributes })
        }

        pub(super) fn attributes(&self) -> *const SECURITY_ATTRIBUTES {
            &raw const self.attributes
        }
    }

    impl Drop for Security {
        fn drop(&mut self) {
            unsafe { LocalFree(self.descriptor) };
        }
    }

    /// This process's user, as a SID string: `S-1-5-21-…`.
    pub(super) fn user_sid() -> io::Result<String> {
        let mut token: HANDLE = std::ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = (|| {
            let mut length = 0;
            unsafe { GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &raw mut length) };
            // In u64s, so the buffer is aligned for the TOKEN_USER read from its start.
            let mut buffer = vec![0u64; (length as usize).div_ceil(8)];
            if unsafe { GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), length, &raw mut length) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
            let mut text: *mut u16 = std::ptr::null_mut();
            if unsafe { ConvertSidToStringSidW(user.User.Sid, &raw mut text) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let length = (0..).take_while(|&i| unsafe { *text.add(i) } != 0).count();
            let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) });
            unsafe { LocalFree(text.cast()) };
            Ok(sid)
        })();
        unsafe { CloseHandle(token) };
        result
    }
}

/// This process's user, as a SID string: what the pipe lets in, for tests to check against.
#[cfg(windows)]
#[doc(hidden)]
pub fn pipe_user() -> std::io::Result<String> {
    owner_only::user_sid()
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
    // The client has finished. Closing this handle would not say so — the reading one is a
    // duplicate of the same connection, and a pipe has no half-close — so the holder is told:
    // a `shutdown` without an id ends this client's session, with no reply, once everything
    // sent before it is answered. Then the holder closes, and the copy above ends.
    let _ = to_holder.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"shutdown\"}\n");
    drop(to_holder);
    let _ = out.join();
    Ok(true)
}
