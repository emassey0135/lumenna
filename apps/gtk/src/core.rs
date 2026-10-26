//! What the tray asks of the window from its thread.

use gtk::glib;

/// Something from another thread for the window: the tray's.
#[derive(Debug)]
pub enum Event {
    /// A command of the core's tray menu (`keys::tray`), by its id.
    Command(String),
    /// The timer's control pressed.
    ToggleTimer,
}

/// Hands an event to the main thread, which gives it to the window.
pub fn post(event: Event) {
    glib::MainContext::default().invoke(move || crate::window::receive(event));
}
