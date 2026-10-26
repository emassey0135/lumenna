//! Lumenna for Windows: Win32 through `windows-rs`, drawing the Crux app (`lumenna-ffi`,
//! linked as Rust: no FFI, no JSON) as the Mac and Android draw it. What is in hand, what is
//! folded, the question open and what to say are the app's; the window draws its view and
//! sends what the person did.
//!
//! **Standard common controls only.** Every list is a `SysTreeView32`, whose level, position,
//! size, expansion and checked state NVDA, JAWS and Narrator have read for decades; the
//! moment anything is custom-drawn, its UI Automation provider is ours to write. Forms are
//! edit controls, combo boxes and buttons, and dialogs are the dialog manager's own.
//!
//! How every line reads, how flat rows become a tree, the menus and the shortcuts from anywhere
//! are the app's (`lumenna_app`), built and tested on every platform. The rest is in `win`,
//! and exists only on Windows, but for the widget's cards (`widget`).

#![cfg_attr(windows, windows_subsystem = "windows")]
// Elsewhere the neutral modules are built for their tests alone, with nothing calling them.
#![cfg_attr(not(windows), allow(dead_code))]

mod widget;

// How every line reads, the keys and the tree are the app's, shared with every other.
#[cfg_attr(not(windows), allow(unused_imports))]
use lumenna_app::{devices, keys, outline, speech};
#[cfg_attr(not(windows), allow(unused_imports))]
use lumenna_ffi::profile;
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
