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
mod rows;
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

/// Every fixed word the app draws, the core's, in Windows' Title Case as a PC calls itself:
/// taken once, so no word is written here.
fn words() -> &'static lumenna_app::words::ShellWords {
    static WORDS: std::sync::OnceLock<lumenna_app::words::ShellWords> = std::sync::OnceLock::new();
    WORDS.get_or_init(|| lumenna_app::words::shell_words(lumenna_app::Capitals::Title, lumenna_app::words::ThisDevice::Pc))
}

/// What the installer's uninstaller starts `lumenna.exe` with, to take away what it wrote of
/// its own that the installer cannot name: every profile's value opening it at sign-in, and the
/// Jump List.
const UNINSTALL: &str = "--uninstall";

pub fn run() {
    // Started by the Widgets board, to draw its widgets: no window.
    if widgets::requested() {
        widgets::serve();
        return;
    }
    if std::env::args().skip(1).any(|argument| argument == UNINSTALL) {
        settings::sign_in::forget_all();
        jump_list::forget();
        return;
    }
    app::run();
}
