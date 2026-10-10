//! Lumenna for Windows: Win32 through `windows-rs`, linking the command surface
//! directly — no FFI, no JSON, the same `Lumenna` object the command line calls.
//!
//! **Standard common controls only.** Every list is a `SysTreeView32`, whose level, position,
//! size, expansion and checked state NVDA, JAWS and Narrator have read for decades; the
//! moment anything is custom-drawn, its UI Automation provider is ours to write. Forms are
//! edit controls, combo boxes and buttons, and dialogs are the dialog manager's own.
//!
//! What is decided here rather than in Win32 code — how a row is worded, how flat rows become
//! a tree — is in modules that build and are tested on every platform. What every client
//! shares, such as what a task form sends back and which places the sidebar holds, is the
//! surface's (`lumenna_surface::task_edit`, `places`). The rest is in `win`, and exists only
//! on Windows.

#![cfg_attr(windows, windows_subsystem = "windows")]
// Elsewhere the neutral modules are built for their tests alone, with nothing calling them.
#![cfg_attr(not(windows), allow(dead_code))]

mod shortcut;

// Wording and the tree are shared with the GTK app.
#[cfg_attr(not(windows), allow(unused_imports))]
use lumenna_desktop::{devices, keys, outline, profile, speech};
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    win::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Lumenna for Windows runs on Windows. Elsewhere, use `lum` or this platform's app.");
    std::process::exit(1);
}
