//! Where the process holding this device's sync endpoint answers the command surface, and
//! reaching it.
//!
//! One address per profile: a Unix socket in the profile directory, or on Windows a named
//! pipe named from the profile's path. Whichever process holds the endpoint — `lum`'s
//! daemon, or a desktop app — serves the whole JSON-RPC surface there ([`crate::rpc`]), so
//! Emacs and the BTSpeak app reach a running app as they reach the daemon, and a round asked
//! for anywhere else on the device runs on the holder's endpoint.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Where the holder of this profile's endpoint answers.
#[cfg(unix)]
#[must_use]
pub fn address(directory: &Path) -> PathBuf {
    directory.join("lumenna.sock")
}

/// Where the holder of this profile's endpoint answers: a named pipe, named from the
/// profile's path so that two profiles on one machine have two. Windows' paths are not case
/// sensitive, so neither is the name.
#[cfg(windows)]
#[must_use]
pub fn address(directory: &Path) -> PathBuf {
    let path = directory.canonicalize().unwrap_or_else(|_| directory.to_path_buf());
    let text = path.display().to_string().to_lowercase();
    // FNV-1a: stable across builds and processes, which `DefaultHasher` does not promise.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    PathBuf::from(format!(r"\\.\pipe\lumenna-{hash:016x}"))
}

/// A connection to the holder, as a client.
pub enum Connection {
    /// A Unix socket.
    #[cfg(unix)]
    Socket(std::os::unix::net::UnixStream),
    /// A named pipe, which a client opens as a file.
    #[cfg(windows)]
    Pipe(std::fs::File),
}

impl Connection {
    /// A second handle on the same connection, for reading on one thread while writing on
    /// another.
    ///
    /// # Errors
    ///
    /// If the system will not duplicate it.
    pub fn try_clone(&self) -> std::io::Result<Self> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.try_clone().map(Self::Socket),
            #[cfg(windows)]
            Self::Pipe(file) => file.try_clone().map(Self::Pipe),
        }
    }

    /// Waits no longer than `limit` for an answer.
    ///
    /// # Errors
    ///
    /// If the system refuses the timeout.
    pub fn set_read_timeout(&self, limit: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.set_read_timeout(limit),
            // A pipe opened as a file has no timeout; the holder answers or closes.
            #[cfg(windows)]
            Self::Pipe(_) => {
                let _ = limit;
                Ok(())
            }
        }
    }
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.read(buf),
            #[cfg(windows)]
            Self::Pipe(file) => file.read(buf),
        }
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.write(buf),
            #[cfg(windows)]
            Self::Pipe(file) => file.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.flush(),
            #[cfg(windows)]
            Self::Pipe(file) => file.flush(),
        }
    }
}

/// Connects to whatever holds this profile's endpoint, or `None` if nothing is answering.
#[must_use]
pub fn connect(directory: &Path) -> Option<Connection> {
    #[cfg(unix)]
    {
        std::os::unix::net::UnixStream::connect(address(directory)).ok().map(Connection::Socket)
    }
    #[cfg(windows)]
    {
        std::fs::OpenOptions::new().read(true).write(true).open(address(directory)).ok().map(Connection::Pipe)
    }
}
