//! Opening at sign-in, in the background: the app's background setting
//! (`BackgroundKind::DesktopSignIn`), which this shell carries out with an XDG autostart entry,
//! or, in a sandbox (a Flatpak), through the Background portal.
//!
//! Its words, and what is said once it changes, are the core's. The shell says whether it is on,
//! and turns it on or off when a turn asks (`Turn::background`), answering with what it now is or
//! why it could not.
//!
//! A sandbox cannot reach the desktop's autostart folder, so there the portal writes the entry,
//! after asking the person, and the portal never says whether one is there: the app keeps the
//! choice it last had granted in the sandbox's own configuration, and asks again at each start
//! while it is on, which asks nothing once allowed and keeps the portal's command current.

use std::path::{Path, PathBuf};

use gtk::glib;
use lumenna_app::BackgroundEvent;
use lumenna_ffi::ShellEvent;
use lumenna_records::background::{BackgroundKind, background_words};

/// This profile's sign-in entry, or in a sandbox the record of the choice the portal granted.
pub struct Autostart {
    file: PathBuf,
    /// The profile's folder, passed to the app the entry starts when it is not the usual one.
    directory: PathBuf,
}

impl Autostart {
    /// The entry for the application `id` over the profile in `directory`: in the XDG autostart
    /// directory, named after the identifier, so each profile has its own. In a sandbox, the
    /// record of it in the sandbox's own configuration.
    pub fn new(id: &str, directory: &Path) -> Self {
        let file = if crate::sandboxed() {
            glib::user_config_dir().join("lumenna").join(format!("{id}.autostart"))
        } else {
            glib::user_config_dir().join("autostart").join(format!("{id}.desktop"))
        };
        Self { file, directory: directory.to_path_buf() }
    }

    /// What the app is told: that this desktop opens Lumenna at sign-in, and whether it does.
    pub fn offered(&self) -> ShellEvent {
        ShellEvent::Background(BackgroundEvent::Offered { kind: BackgroundKind::DesktopSignIn, on: self.file.exists() })
    }

    /// Makes or removes the entry as the app asked, and what to tell it of how that went.
    /// Outside a sandbox only: in one, [`Autostart::ask`].
    pub fn keep(&self, on: bool) -> ShellEvent {
        match self.set(on) {
            Ok(()) => self.offered(),
            Err(error) => failed(error.to_string()),
        }
    }

    /// In a sandbox, has the Background portal open the app at sign-in or stop, asking the person
    /// if it has not yet been allowed, over `window`; what to tell the app of how that went.
    pub async fn ask(&self, on: bool, window: Option<&gtk::ApplicationWindow>) -> ShellEvent {
        match self.request(on, window).await {
            Ok(granted) if granted == on => match self.record(on) {
                Ok(()) => self.offered(),
                Err(error) => failed(error.to_string()),
            },
            Ok(_) => failed("it was not allowed".to_owned()),
            Err(error) => failed(error.to_string()),
        }
    }

    /// At start, in a sandbox, while it is on: asks the portal again, which asks the person
    /// nothing once allowed and keeps its entry's command this build's. Its answer is the one
    /// reading of whether it still opens at sign-in: turned off since, in the desktop's own
    /// settings, the record is forgotten and the app told it is off (what this returns), so the
    /// setting says so and nothing is asked again. A portal that cannot be reached changes nothing.
    pub async fn renew(&self) -> Option<ShellEvent> {
        if !crate::sandboxed() || !self.file.exists() {
            return None;
        }
        match self.request(true, None).await {
            Ok(false) => {
                let _ = remove(&self.file);
                Some(self.offered())
            }
            _ => None,
        }
    }

    /// The portal's answer to opening at sign-in (`on`) or not: whether it now does.
    async fn request(&self, on: bool, window: Option<&gtk::ApplicationWindow>) -> ashpd::Result<bool> {
        let identifier = match window {
            Some(window) => ashpd::WindowIdentifier::from_native(window).await,
            None => None,
        };
        let mut command = vec!["lumenna-gtk".to_owned(), "--background".to_owned()];
        if lumenna_ffi::profile::default_directory().as_deref() != Some(self.directory.as_path()) {
            command.push("--profile".to_owned());
            command.push(self.directory.display().to_string());
        }
        let asking = background_words(BackgroundKind::DesktopSignIn).asking;
        let answer = ashpd::desktop::background::Background::request()
            .identifier(identifier)
            .reason(asking.as_str())
            .auto_start(on)
            .command(command)
            .dbus_activatable(false)
            .send()
            .await?
            .response()?;
        Ok(answer.auto_start())
    }

    /// Keeps, or forgets, that the portal opens the app at sign-in.
    fn record(&self, on: bool) -> std::io::Result<()> {
        if !on {
            return remove(&self.file);
        }
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.file, "Lumenna opens at sign-in, through the Background portal.\n")
    }

    fn set(&self, on: bool) -> std::io::Result<()> {
        if !on {
            return remove(&self.file);
        }
        // An AppImage's own path, not its program's, which is inside a mount gone once it stops.
        let program = crate::appimage::program()?;
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
                "[Desktop Entry]\nType=Application\nName=Lumenna\nComment={}\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
                lumenna_records::words::DESCRIPTION
            ),
        )
    }
}

fn failed(reason: String) -> ShellEvent {
    ShellEvent::Background(BackgroundEvent::Failed { reason })
}

fn remove(file: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(file) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// A path as a desktop entry's Exec line takes it.
fn quoted(path: &Path) -> String {
    crate::appimage::exec_argument(path)
}
