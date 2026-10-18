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
    /// A named pipe.
    #[cfg(windows)]
    Pipe(Pipe),
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
            Self::Pipe(pipe) => pipe.try_clone().map(Self::Pipe),
        }
    }

    /// Waits no longer than `limit` for an answer.
    ///
    /// # Errors
    ///
    /// If the system refuses the timeout.
    pub fn set_read_timeout(&mut self, limit: Option<std::time::Duration>) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.set_read_timeout(limit),
            #[cfg(windows)]
            Self::Pipe(pipe) => {
                pipe.timeout = limit;
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
            Self::Pipe(pipe) => pipe.read(buf),
        }
    }
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.write(buf),
            #[cfg(windows)]
            Self::Pipe(pipe) => pipe.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Socket(stream) => stream.flush(),
            #[cfg(windows)]
            Self::Pipe(pipe) => pipe.flush(),
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
        use std::os::windows::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OVERLAPPED)
            .open(address(directory))
            .ok()?;
        Some(Connection::Pipe(Pipe::new(file.into())))
    }
}

/// One end of a named pipe, opened for overlapped I/O.
///
/// Not a `std::fs::File`: Windows serialises synchronous I/O on one file object, so a
/// thread blocked reading a pipe would hold up another writing to it — the relay reading
/// replies while it sends requests, a server pushing a notification while it waits for the
/// next request — and both would wait for ever. Overlapped I/O lets the two run at once, each
/// waiting on an event of its own, and gives a read its timeout.
#[cfg(windows)]
pub struct Pipe {
    handle: std::os::windows::io::OwnedHandle,
    timeout: Option<std::time::Duration>,
}

#[cfg(windows)]
mod overlapped {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, ERROR_PIPE_CONNECTED,
        ERROR_PIPE_NOT_CONNECTED, GetLastError, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
    use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
    use windows_sys::Win32::System::Pipes::ConnectNamedPipe;
    use windows_sys::Win32::System::Threading::{CreateEventW, INFINITE, WaitForSingleObject};

    use super::Pipe;

    /// An event for one operation, closed with it.
    struct Event(HANDLE);

    impl Event {
        fn new() -> io::Result<Self> {
            // Manual reset, as overlapped I/O requires.
            let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if event.is_null() { Err(io::Error::last_os_error()) } else { Ok(Self(event)) }
        }
    }

    impl Drop for Event {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// How an operation ended.
    enum Ended {
        Done(u32),
        TimedOut,
        Failed(u32),
    }

    impl Pipe {
        fn raw(&self) -> HANDLE {
            self.handle.as_raw_handle()
        }

        /// Starts an operation with `start` and waits for it, no longer than `limit`;
        /// `keep_waiting` is asked each time the limit passes, and ends the wait when it says
        /// no. A wait given up cancels the operation before returning, so nothing is left
        /// writing into memory this frame no longer owns.
        fn run(
            &self,
            limit: Option<Duration>,
            keep_waiting: &dyn Fn() -> bool,
            start: impl FnOnce(HANDLE, *mut OVERLAPPED) -> i32,
        ) -> io::Result<Ended> {
            let event = Event::new()?;
            let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
            overlapped.hEvent = event.0;
            if start(self.raw(), &raw mut overlapped) == 0 {
                let error = unsafe { GetLastError() };
                if error != ERROR_IO_PENDING {
                    return Ok(Ended::Failed(error));
                }
                let millis = limit.map_or(INFINITE, |limit| u32::try_from(limit.as_millis()).unwrap_or(INFINITE - 1));
                loop {
                    match unsafe { WaitForSingleObject(event.0, millis) } {
                        WAIT_OBJECT_0 => break,
                        WAIT_TIMEOUT if keep_waiting() => {}
                        WAIT_TIMEOUT => {
                            unsafe { CancelIoEx(self.raw(), &raw const overlapped) };
                            let mut moved = 0;
                            unsafe { GetOverlappedResult(self.raw(), &raw const overlapped, &raw mut moved, 1) };
                            return Ok(Ended::TimedOut);
                        }
                        _ => return Err(io::Error::last_os_error()),
                    }
                }
            }
            let mut moved = 0;
            if unsafe { GetOverlappedResult(self.raw(), &raw const overlapped, &raw mut moved, 0) } == 0 {
                return Ok(Ended::Failed(unsafe { GetLastError() }));
            }
            Ok(Ended::Done(moved))
        }

        /// Waits for a client to connect to this server end, asking `keep_waiting` every
        /// `step` whether to go on: true once one has, false if the wait was given up.
        // Only the RPC server listens; a build without it never calls this.
        #[cfg_attr(not(feature = "rpc"), allow(dead_code))]
        pub(crate) fn accept(&self, step: Duration, keep_waiting: &dyn Fn() -> bool) -> io::Result<bool> {
            match self.run(Some(step), keep_waiting, |handle, overlapped| unsafe { ConnectNamedPipe(handle, overlapped) })? {
                Ended::Done(_) => Ok(true),
                // A client that connected between the pipe's making and the wait.
                Ended::Failed(ERROR_PIPE_CONNECTED) => Ok(true),
                Ended::TimedOut | Ended::Failed(ERROR_OPERATION_ABORTED) => Ok(false),
                Ended::Failed(error) => Err(io::Error::from_raw_os_error(error as i32)),
            }
        }
    }

    impl io::Read for Pipe {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let length = u32::try_from(buf.len()).unwrap_or(u32::MAX);
            let ended = self.run(self.timeout, &|| false, |handle, overlapped| unsafe {
                ReadFile(handle, buf.as_mut_ptr(), length, std::ptr::null_mut(), overlapped)
            })?;
            match ended {
                Ended::Done(read) => Ok(read as usize),
                // The other end has gone: the end of what it sent.
                Ended::Failed(ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) => Ok(0),
                Ended::Failed(error) => Err(io::Error::from_raw_os_error(error as i32)),
                Ended::TimedOut => Err(io::Error::new(io::ErrorKind::TimedOut, "no answer in time")),
            }
        }
    }

    impl io::Write for Pipe {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let length = u32::try_from(buf.len()).unwrap_or(u32::MAX);
            let ended = self.run(None, &|| true, |handle, overlapped| unsafe {
                WriteFile(handle, buf.as_ptr(), length, std::ptr::null_mut(), overlapped)
            })?;
            match ended {
                Ended::Done(written) => Ok(written as usize),
                Ended::Failed(error) => Err(io::Error::from_raw_os_error(error as i32)),
                Ended::TimedOut => Err(io::Error::new(io::ErrorKind::TimedOut, "the pipe took nothing")),
            }
        }

        /// Nothing to do: what is written is in the pipe, for the other end to read. Waiting
        /// for it to be read (`FlushFileBuffers`) would hang on a reader that has stopped.
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(windows)]
impl Pipe {
    /// Takes an end opened with `FILE_FLAG_OVERLAPPED`.
    pub(crate) fn new(handle: std::os::windows::io::OwnedHandle) -> Self {
        Self { handle, timeout: None }
    }

    /// A second handle on the same end, for reading on one thread while writing on another.
    ///
    /// # Errors
    ///
    /// If the system will not duplicate it.
    pub fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self { handle: self.handle.try_clone()?, timeout: self.timeout })
    }
}

