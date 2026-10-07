//! Everything that touches Win32.

mod a11y;
mod app;
mod block_form;
mod blocks;
mod clock;
mod completion;
mod controls;
mod core;
mod dark;
mod day;
mod detail;
mod dialog;
mod font;
mod menu;
mod prompts;
mod quick_add;
mod pairing;
mod settings;
mod shortcuts;
mod sidebar;
mod system;
mod task_actions;
mod tasks;
mod tray;
mod tree;
mod view;

pub fn run() {
    app::run();
}
