//! Keeping Lumenna running in the background: the one device setting whose value is the
//! platform's, since only the platform can make something start with it.
//!
//! Each platform has its own way, and so its own words: a desktop app opens at sign-in and stays
//! resident, BTSpeak runs a service that syncs and gives reminders, and Emacs, which is not open
//! all the time, turns `lum`'s sync daemon on. The shell says which it has and whether it is on;
//! the app offers the setting among the device's own, and hands the shell what the person chose.

use serde::{Deserialize, Serialize};

use crate::actions::Choice;
use crate::types::{Setting, SettingKind};

/// The setting's key, among the device's own.
pub const KEY: &str = "background";

/// What a platform keeps running in the background, which its words follow.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundKind {
    /// Windows: the app opens at sign-in and stays in the notification area.
    WindowsSignIn,
    /// The Mac: the app opens at login and stays in the menu bar.
    MacLogin,
    /// A Linux desktop: the app opens at sign-in, its window closed, and keeps running.
    DesktopSignIn,
    /// BTSpeak: a service of the app's own keeps the device in sync and gives its reminders.
    Service,
    /// A client that is not always open, Emacs: `lum`'s sync daemon keeps the device in sync.
    SyncDaemon,
}

/// What the setting says, as the platform has it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundWords {
    /// The setting's name.
    pub title: String,
    /// What it does, for under the control.
    pub hint: String,
    /// Said once it is on.
    pub on: String,
    /// Said once it is off.
    pub off: String,
    /// Said, before the platform's reason, when it could not be changed.
    pub failed: String,
}

/// The words for `kind`.
#[must_use]
pub fn background_words(kind: BackgroundKind) -> BackgroundWords {
    let (title, hint, on, off) = match kind {
        BackgroundKind::WindowsSignIn => (
            "Open Lumenna when you sign in to Windows",
            "Lumenna stays running in the notification area when its window is closed, so your devices stay in \
             sync. Exit it from the File menu or the notification area.",
            "Lumenna opens when you sign in",
            "Lumenna no longer opens when you sign in",
        ),
        BackgroundKind::MacLogin => (
            "Open Lumenna at login",
            "Lumenna stays running in the menu bar when its window is closed, so your devices stay in sync. Quit \
             it from the menu bar or the Lumenna menu.",
            "Lumenna opens at login",
            "Lumenna no longer opens at login",
        ),
        BackgroundKind::DesktopSignIn => (
            "Open Lumenna when you sign in, in the background",
            "Lumenna keeps running after its window is closed, so it syncs and reminds you. Start it again, from \
             the menu or with lumenna-gtk, to bring the window back.",
            "Lumenna opens when you sign in",
            "Lumenna no longer opens when you sign in",
        ),
        BackgroundKind::Service => (
            "Background reminders and sync",
            "A service keeps this device in sync with your other devices and gives its reminders, with the \
             notification sound, speech and braille, while Lumenna is closed. It starts whenever the device does.",
            "Background reminders and sync are on. They start whenever the device does",
            "Background reminders and sync are off",
        ),
        BackgroundKind::SyncDaemon => (
            "Sync in the background",
            "Lumenna's sync daemon keeps this device in sync with your other devices while Emacs is closed, and \
             starts when you log in.",
            "The sync daemon is on. It starts when you log in",
            "The sync daemon is off",
        ),
    };
    BackgroundWords {
        title: title.to_owned(),
        hint: hint.to_owned(),
        on: on.to_owned(),
        off: off.to_owned(),
        failed: "That could not be changed".to_owned(),
    }
}

/// The setting, describing itself, as `kind` words it and `on` has it: a toggle of this device's
/// alone.
#[must_use]
pub fn background_setting(kind: BackgroundKind, on: bool) -> Setting {
    let words = background_words(kind);
    let option = |id: &str, title: &str| Choice { id: id.to_owned(), title: title.to_owned(), ..Default::default() };
    Setting {
        key: KEY.to_owned(),
        value: on.to_string(),
        title: words.title,
        kind: SettingKind::Toggle,
        options: vec![option("true", "On"), option("false", "Off")],
        syncs: false,
        hint: words.hint,
    }
}
