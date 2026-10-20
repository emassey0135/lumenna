//! What the tray asks of the window from its thread.

use gtk::glib;

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
