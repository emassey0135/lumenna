//! The open store, for the screens not yet drawn from the app, and what runs beside the
//! window: backups. Syncing is the app's network's (`lumenna-ffi`), which serves the command
//! surface while it holds the device's endpoint.
//!
//! What takes longer — a backup — runs on a thread of its own and posts what it has to say
//! back to the window.

use std::sync::Arc;

use lumenna_host::{Lumenna, LumennaError};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::speech;

/// A sentence to say, posted from another thread: a boxed `String` in `lparam`.
pub const WM_SAY: u32 = WM_APP + 2;

/// The window to post to, as a number, since a window handle is not `Send`.
#[derive(Clone, Copy)]
pub struct Poster(isize);

impl Poster {
    pub fn new(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }

    fn post(self, message: u32, lparam: isize) -> bool {
        unsafe { PostMessageW(Some(HWND(self.0 as _)), message, WPARAM(0), LPARAM(lparam)).is_ok() }
    }

    /// Asks the window to say something.
    pub fn say(self, text: String) {
        self.send(WM_SAY, text);
    }

    /// Posts a value to the window, boxed in `lparam`, for it to take back with [`taken`].
    /// Returns whether it was posted; a window that has gone never receives it, and the
    /// value is dropped here instead.
    pub fn send<T>(self, message: u32, value: T) -> bool {
        let boxed = Box::into_raw(Box::new(value));
        let posted = self.post(message, boxed as isize);
        if !posted {
            drop(unsafe { Box::from_raw(boxed) });
        }
        posted
    }
}

/// Takes back a sentence posted with [`Poster::say`].
///
/// # Safety
///
/// `lparam` must be what a `WM_SAY` carried, and taken back once.
pub unsafe fn said(lparam: LPARAM) -> String {
    unsafe { taken(lparam) }
}

/// Takes back a value posted with [`Poster::send`].
///
/// # Safety
///
/// `lparam` must be what that message carried, of type `T`, and taken back once.
pub unsafe fn taken<T>(lparam: LPARAM) -> T {
    *unsafe { Box::from_raw(lparam.0 as *mut T) }
}

/// The open store.
pub struct Core {
    pub lumenna: Arc<Lumenna>,
}

impl Core {
    pub fn open(directory: &std::path::Path) -> Result<Self, LumennaError> {
        Ok(Self { lumenna: Lumenna::open(&directory.display().to_string())? })
    }

    /// Takes a backup if one is due, and says so only if it failed.
    pub fn back_up_if_due(&self, window: Poster) {
        let lumenna = Arc::clone(&self.lumenna);
        std::thread::spawn(move || {
            if let Err(error) = lumenna.back_up_if_due() {
                window.say(format!("The automatic backup failed. {}", sentence(&error)));
            }
        });
    }
}

/// The sentence the core wrote for an error, which is already phrased to be read aloud.
pub fn sentence(error: &LumennaError) -> String {
    speech::sentence(error.message())
}
