//! The Crux app (crates/app, run by lumenna-ffi as the Mac and Android run it) as this window
//! reaches it: every turn the app leaves is posted to the window's thread, which draws it and
//! then says what it says.
//!
//! What is in hand, what is folded, the question open and what to say are the app's. A screen
//! drawn from it shows `view` with `lines` and sends what the person did as a `ShellEvent`.

use lumenna_ffi::{Shell, Turn};

use super::core::Poster;

/// A turn the app left: a boxed [`Turn`] in `lparam`.
pub const WM_TURN: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 7;

/// The app's turns, from its thread onto the window's, in order.
pub struct Relay(pub Poster);

impl Shell for Relay {
    fn turn(&self, turn: Turn) {
        self.0.send(WM_TURN, turn);
    }
}
