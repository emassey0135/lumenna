//! What every view in the middle pane does, and the measurements views are laid out by.

use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Controls::NMHDR;

use super::app::App;

/// Sizes in pixels at the window's DPI, from the system's message font.
#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    /// Pixels per 96 DPI pixel.
    pub scale: f32,
    /// A line of text.
    pub line: i32,
    /// A single-line field or combo box.
    pub field: i32,
    /// A push button.
    pub button: i32,
}

impl Metrics {
    /// `pixels` at 96 DPI, at this DPI.
    pub fn px(&self, pixels: i32) -> i32 {
        (pixels as f32 * self.scale).round() as i32
    }

    /// The space around and between things.
    pub fn gap(&self) -> i32 {
        self.px(8)
    }
}

/// A view in the middle pane: a task list, the day, the blocks.
///
/// Each answers for its own controls; the main window routes to it whatever comes from them,
/// and asks it what to focus when F6 reaches the pane.
pub trait View {
    /// Where focus goes when the pane is reached.
    fn focus_target(&self) -> HWND;

    /// Lays the view out in its pane.
    fn layout(&self, width: i32, height: i32, metrics: Metrics);

    /// Reads the store again, keeping the selection where it was.
    fn reload(&self, app: &App);

    /// A command from one of its controls. Returns whether it was the view's.
    fn command(&self, _app: &App, _control: HWND, _id: u16, _code: u16) -> bool {
        false
    }

    /// A notification from one of its controls, and what to return for it.
    fn notify(&self, _app: &App, _header: &NMHDR, _lparam: LPARAM) -> Option<isize> {
        None
    }

    /// A context menu asked for on one of its controls: at a point on screen with the mouse,
    /// or from the keyboard. Returns whether it was the view's.
    fn context_menu(&self, _app: &App, _control: HWND, _point: Option<POINT>) -> bool {
        false
    }

    /// Enter, with focus in the view. Returns whether it did anything.
    fn enter(&self, _app: &App, _focus: HWND) -> bool {
        false
    }

    /// Escape, with focus in the view.
    fn escape(&self, _app: &App, _focus: HWND) -> bool {
        false
    }

    /// Its windows, to destroy when another view takes the pane.
    fn windows(&self) -> Vec<HWND>;

    /// Called once a minute, for a view whose content follows the clock.
    fn minute(&self, _app: &App) {}
}
