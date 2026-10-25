//! Everything that touches Win32.

mod a11y;
mod actions;
mod app;
mod asking;
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
mod help;
mod jump_list;
mod menu;
mod prompts;
mod quick_add;
mod package;
mod pairing;
mod picker;
mod proposal;
mod reminders;
mod settings;
mod shell;
mod shortcuts;
mod sidebar;
mod swatch;
mod system;
mod tasks;
mod toast;
mod tray;
mod tree;
mod view;
mod widgets;

pub fn run() {
    // Started by the Widgets board, to draw its widgets: no window.
    if widgets::requested() {
        widgets::serve();
        return;
    }
    app::run();
}
