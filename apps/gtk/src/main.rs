//! Lumenna for Linux: GTK 4 through `gtk4-rs`, drawing the Crux app (`lumenna-ffi`, linked as
//! Rust: no FFI, no JSON) as the Windows app draws it.
//!
//! **Stock widgets, annotated where GTK under-reports.** Every list is a `GtkListView` that
//! tells Orca it is a tree, with each row's level, position and expansion (`tree`); forms are
//! entries, drop-downs and buttons named by their labels; dialogs are GTK's own. What is
//! decided rather than drawn — what is in hand, what is folded, the question open, how a row is
//! worded, how flat rows become a tree — is the app's (`lumenna_app`), as on every platform.
//!
//! One instance per profile: GApplication registers the app's identifier on the session bus,
//! and starting it again activates the running one, which shows its window. A launch shortcut
//! (`--launch <id>`, a desktop entry's action) goes to the running one as its `launch` action.

mod actions;
mod appimage;
mod asking;
mod autostart;
mod block_form;
mod clock;
mod completion;
mod core;
mod day;
mod detail;
mod help;
mod model;
mod pairing;
mod pickers;
mod prompts;
mod proposal;
mod quick_add;
mod reminders;
mod settings;
mod shortcuts;
mod sidebar;
mod tasks;
mod tray;
mod tree;
mod window;

use std::path::Path;

use gtk::{gio, glib};
use gtk::prelude::*;
use lumenna_ffi::profile;

/// The application identifier, which is also the name it owns on the session bus.
pub const ID: &str = "dev.emassey0135.Lumenna";

/// Whether the app runs in a sandbox, a Flatpak's: it opens at sign-in through the Background
/// portal (`autostart.rs`), and its tray owns no well-known name (`tray.rs`). Its store is the
/// sandbox's own, `~/.var/app/<id>/data/lumenna`, by the XDG directories the sandbox sets.
pub fn sandboxed() -> bool {
    ashpd::is_sandboxed()
}

/// Where the app's own icon is in this checkout, for a build run without `install.sh`, which
/// puts it in the user's hicolor theme.
pub fn icons_here() -> Option<&'static str> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/icons");
    Path::new(path).is_dir().then_some(path)
}

/// The identifier for a profile: the usual one plain, any other tagged by its path, so each
/// profile has one instance of its own and none answers for another's.
fn application_id(directory: &Path) -> String {
    if profile::default_directory().as_deref() == Some(directory) {
        return ID.to_owned();
    }
    // FNV-1a, which is stable from one build to the next as a standard hasher is not.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in directory.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{ID}.Profile{hash:016x}")
}

fn main() -> glib::ExitCode {
    let arguments = profile::arguments(std::env::args().skip(1));
    let Some(directory) = profile::directory(arguments.profile) else {
        eprintln!("Lumenna cannot find a folder for its data. Set LUMENNA_PROFILE to one.");
        return glib::ExitCode::FAILURE;
    };
    let application = gtk::Application::builder().application_id(application_id(&directory)).build();
    // libadwaita for its alert dialogs and the like, which need it set up once GTK is.
    application.connect_startup(|_| {
        if let Err(error) = adw::init() {
            eprintln!("libadwaita did not start: {error}");
        }
        // The app's own icon, on every window and the About dialog.
        if let (Some(display), Some(icons)) = (gtk::gdk::Display::default(), icons_here()) {
            gtk::IconTheme::for_display(&display).add_search_path(icons);
        }
        gtk::Window::set_default_icon_name(ID);
        tree::swatches();
    });
    // A launch shortcut chosen while the app runs, from another start of it.
    let launch = gio::SimpleAction::new("launch", Some(glib::VariantTy::STRING));
    launch.connect_activate(|_, id| {
        if let Some(id) = id.and_then(glib::Variant::str) {
            window::launch(id);
        }
    });
    application.add_action(&launch);
    let (background, shortcuts) = (arguments.background, !arguments.no_shortcuts);
    // The first start does its launch shortcut once the window is up; a later activation, from
    // another start, has its own through the action.
    let first = std::cell::Cell::new(arguments.launch.clone());
    application.connect_activate(move |application| {
        window::activate(application, &directory, background, shortcuts);
        if let Some(id) = first.take() {
            window::launch(&id);
        }
    });
    if let Some(id) = &arguments.launch {
        // Started again with a shortcut: the copy already running does it. Registering here
        // tells which this is; run() below then activates that copy and waits for both to go.
        if application.register(None::<&gio::Cancellable>).is_ok() && application.is_remote() {
            application.activate_action("launch", Some(&id.to_variant()));
        }
    }
    // GTK is given no arguments: they were ours, and were read above.
    let program = std::env::args().next().unwrap_or_else(|| "lumenna-gtk".to_owned());
    application.run_with_args(&[program])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usual_profile_is_the_plain_identifier() {
        if let Some(usual) = profile::default_directory() {
            assert_eq!(application_id(&usual), ID);
        }
    }

    #[test]
    fn another_profile_has_an_identifier_of_its_own() {
        let one = application_id(Path::new("/tmp/one"));
        let two = application_id(Path::new("/tmp/two"));
        assert_ne!(one, two);
        assert!(one.starts_with(&format!("{ID}.Profile")));
        assert!(gtk::gio::Application::id_is_valid(&one));
    }
}
