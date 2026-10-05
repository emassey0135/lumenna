//! The open store, and what runs beside the window: syncing and backups.
//!
//! Operations are milliseconds against local SQLite, so they run on the window's thread and a
//! view never shows a state the store has moved past — as on the Mac. What takes longer —
//! starting the sync endpoint, a sync round, a backup — runs on a thread of its own and posts
//! what it has to say back to the window.

use std::sync::{Arc, Mutex, PoisonError};

use lumenna_surface::{Lumenna, LumennaError, Reach, SyncListener, SyncService};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};

use crate::speech;

/// Something arrived from another device, or another process wrote to the store.
pub const WM_STORE_CHANGED: u32 = WM_APP + 1;
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

    pub fn changed(self) {
        self.post(WM_STORE_CHANGED, 0);
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
    sync: Arc<Mutex<Option<Arc<SyncService>>>>,
}

impl Core {
    pub fn open(directory: &std::path::Path) -> Result<Self, LumennaError> {
        let lumenna = Lumenna::open(&directory.display().to_string())?;
        Ok(Self { lumenna, sync: Arc::new(Mutex::new(None)) })
    }

    /// Starts keeping this device in sync, for as long as the app runs (§16.2): the resident
    /// app is the device's sync process, with no service to set up.
    ///
    /// If another process already holds the endpoint — `lum daemon` — that one carries on
    /// and this does nothing: either way the device is in sync.
    pub fn start_syncing(&self, window: Poster) {
        let lumenna = Arc::clone(&self.lumenna);
        let sync = Arc::clone(&self.sync);
        std::thread::spawn(move || {
            let listener: Arc<dyn SyncListener> = Arc::new(Arrivals(window));
            match lumenna.start_sync(Reach::Internet, listener) {
                Ok(service) => *sync.lock().unwrap_or_else(PoisonError::into_inner) = Some(service),
                Err(LumennaError::SyncElsewhere { .. }) => {}
                Err(error) => window.say(format!("Syncing could not start. {}", sentence(&error))),
            }
        });
    }

    /// Stops syncing and lets go of the endpoint, before returning.
    pub fn stop_syncing(&self) {
        let service = self.sync.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(service) = service {
            service.stop();
        }
    }

    /// Syncs with every paired device now, on this app's endpoint or one opened for the
    /// round, and says how it went.
    pub fn sync_now(&self, window: Poster) {
        let lumenna = Arc::clone(&self.lumenna);
        let sync = Arc::clone(&self.sync);
        std::thread::spawn(move || {
            let service = sync.lock().unwrap_or_else(PoisonError::into_inner).clone();
            let result = match service {
                Some(service) => service.sync_now(),
                None => lumenna.sync_now(Reach::Internet),
            };
            match result {
                Ok(report) => {
                    let failures: Vec<String> = report
                        .peers
                        .iter()
                        .filter_map(|peer| peer.error.as_ref().map(|e| format!("{}: {e}", peer.name)))
                        .collect();
                    window.say(speech::announcement(&report.announcement, &failures));
                    window.changed();
                }
                Err(error) => window.say(sentence(&error)),
            }
        });
    }

    /// Takes a backup if one is due (§9), and says so only if it failed.
    pub fn back_up_if_due(&self, window: Poster) {
        let lumenna = Arc::clone(&self.lumenna);
        std::thread::spawn(move || {
            if let Err(error) = lumenna.back_up_if_due() {
                window.say(format!("The automatic backup failed. {}", sentence(&error)));
            }
        });
    }
}

/// What a sync brought in: every view reads the store again.
struct Arrivals(Poster);

impl SyncListener for Arrivals {
    fn changed(&self) {
        self.0.changed();
    }
}

/// The sentence the core wrote for an error, which is already phrased to be read aloud.
pub fn sentence(error: &LumennaError) -> String {
    speech::sentence(error.message())
}
