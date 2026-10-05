//! Everything that touches Win32.

mod a11y;
mod app;
mod block_form;
mod blocks;
mod clock;
mod completion;
mod controls;
mod core;
mod day;
mod detail;
mod dialog;
mod menu;
mod prompts;
mod quick_add;
mod sidebar;
mod tasks;
mod tray;
mod tree;
mod view;

pub fn run() {
    app::run();
}
