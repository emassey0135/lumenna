//! The store's own connection, for the screens not yet drawn from the app — a reminder list,
//! quick add, completion in the filter — and what the tray asks of the window from its thread.

use std::sync::Arc;

use gtk::glib;
use lumenna_desktop::speech;
use lumenna_host::{Lumenna, LumennaError};

/// Something from another thread for the window: the tray's.
#[derive(Debug)]
pub enum Event {
    /// Show the window.
    ShowWindow,
    /// Quick add, over whatever is in front.
    QuickAdd,
    /// Sync now.
    SyncNow,
    /// Quit.
    Quit,
}

/// Hands an event to the main thread, which gives it to the window.
pub fn post(event: Event) {
    glib::MainContext::default().invoke(move || crate::window::receive(event));
}

/// The open store.
pub struct Core {
    pub lumenna: Arc<Lumenna>,
}

impl Core {
    pub fn open(directory: &std::path::Path) -> Result<Self, LumennaError> {
        Ok(Self { lumenna: Lumenna::open(&directory.display().to_string())? })
    }
}

/// The sentence the core wrote for an error, which is already phrased to be read aloud.
pub fn sentence(error: &LumennaError) -> String {
    speech::sentence(error.message())
}
