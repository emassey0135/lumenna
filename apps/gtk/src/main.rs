//! Lumenna for Linux: GTK 4 through `gtk4-rs`, linking the command surface directly —
//! no FFI, no JSON, the same `Lumenna` object the command line calls.
//!
//! **Stock widgets, annotated where GTK under-reports.** Every list is a `GtkListView` that
//! tells Orca it is a tree, with each row's level, position and expansion (`tree`); forms are
//! entries, drop-downs and buttons named by their labels; dialogs are GTK's own. What is
//! decided rather than drawn — how a row is worded, how flat rows become a tree — is
//! `lumenna-desktop`'s, shared with the Windows app, and what every client shares, such as
//! which places the sidebar holds or what a task form sends back, is the surface's.
//!
//! One instance per profile: GApplication registers the app's identifier on the session bus,
//! and starting it again activates the running one, which shows its window.

mod actions;
mod block_form;
mod blocks;
mod clock;
mod completion;
mod core;
mod day;
mod detail;
mod help;
mod pairing;
mod prompts;
mod quick_add;
mod settings;
mod shortcuts;
mod sidebar;
mod tasks;
mod tray;
mod tree;
mod window;

use std::path::Path;

use gtk::glib;
use gtk::prelude::*;
use lumenna_desktop::profile;

/// The application identifier, which is also the name it owns on the session bus.
pub const ID: &str = "io.github.emassey0135.Lumenna";

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
    });
    let (background, shortcuts) = (arguments.background, !arguments.no_shortcuts);
    application.connect_activate(move |application| window::activate(application, &directory, background, shortcuts));
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
