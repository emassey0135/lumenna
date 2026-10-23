//! Opening at sign-in, in the background: the app's background setting
//! (`BackgroundKind::DesktopSignIn`), which this shell carries out with an XDG autostart entry.
//!
//! Its words, and what is said once it changes, are the core's. The shell says whether the
//! entry is there, and makes or removes it when a turn asks (`Turn::background`), answering
//! with what it now is or why it could not.

use std::path::{Path, PathBuf};

use gtk::glib;
use lumenna_app::BackgroundEvent;
use lumenna_ffi::ShellEvent;
use lumenna_records::background::BackgroundKind;

/// This profile's sign-in entry.
pub struct Autostart {
    file: PathBuf,
    /// The profile's folder, passed to the app the entry starts when it is not the usual one.
    directory: PathBuf,
}

impl Autostart {
    /// The entry for the application `id` over the profile in `directory`: in the XDG autostart
    /// directory, named after the identifier, so each profile has its own.
    pub fn new(id: &str, directory: &Path) -> Self {
        Self { file: glib::user_config_dir().join("autostart").join(format!("{id}.desktop")), directory: directory.to_path_buf() }
    }

    /// What the app is told: that this desktop opens Lumenna at sign-in, and whether it does.
    pub fn offered(&self) -> ShellEvent {
        ShellEvent::Background(BackgroundEvent::Offered { kind: BackgroundKind::DesktopSignIn, on: self.file.exists() })
    }

    /// Makes or removes the entry as the app asked, and what to tell it of how that went.
    pub fn keep(&self, on: bool) -> ShellEvent {
        match self.set(on) {
            Ok(()) => self.offered(),
            Err(error) => ShellEvent::Background(BackgroundEvent::Failed { reason: error.to_string() }),
        }
    }

    fn set(&self, on: bool) -> std::io::Result<()> {
        if !on {
            return match std::fs::remove_file(&self.file) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        let program = std::env::current_exe()?;
        let mut exec = format!("{} --background", quoted(&program));
        if lumenna_ffi::profile::default_directory().as_deref() != Some(self.directory.as_path()) {
            exec.push_str(&format!(" --profile {}", quoted(&self.directory)));
        }
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &self.file,
            format!(
                "[Desktop Entry]\nType=Application\nName=Lumenna\nComment=Tasks and time blocks\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
            ),
        )
    }
}

/// A path as a desktop entry's Exec line takes it.
fn quoted(path: &Path) -> String {
    let text = path.display().to_string();
    if text.chars().any(|c| c.is_whitespace() || "\"'\\$`".contains(c)) {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
    } else {
        text
    }
}
