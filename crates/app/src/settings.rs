//! Settings: each applied as it is made (a line when it is left), saying so; a line refused stays
//! as typed, beside what was wrong with it. Backups, exports and imports are here too: the host
//! makes the file, and the shell saves or picks it.

use std::collections::BTreeMap;

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::{LumennaError, Offered};
use lumenna_records::background::{BackgroundKind, KEY as BACKGROUND, background_words};
use lumenna_records::types::{ExportFormat, Setting};
use serde::{Deserialize, Serialize};

use crate::host::{Arrival, HostRequest};
use crate::model::Screen;
use crate::{Effect, Event, Lumenna, host, say, write};

/// What the person does in Settings.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettingsEvent {
    /// Settings open.
    Open,
    /// Settings close.
    Close,
    /// A line typed into a setting, not yet applied.
    Type {
        /// The setting.
        key: String,
        /// What is typed.
        value: String,
    },
    /// Apply a setting: a toggle or a choice as it is made, a line when it is left.
    Set {
        /// The setting.
        key: String,
        /// Its new value.
        value: String,
    },
    /// Take a backup now, for the shell to save.
    Backup,
    /// Export what is there now, in this format, for the shell to save.
    Export(ExportFormat),
    /// Read a JSON export or a backup the person chose.
    Import {
        /// The file's name, which says which it is.
        name: String,
        /// Its contents.
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
}

/// What the shell says of keeping Lumenna running in the background, the one device setting
/// whose value is the platform's (`lumenna_records::background`).
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundEvent {
    /// What this platform keeps running and whether it is on: sent once at start, and again
    /// whenever it changes, the app's asking included.
    Offered {
        /// What it keeps running, which the setting's words follow.
        kind: BackgroundKind,
        /// Whether it is on.
        on: bool,
    },
    /// Turning it on or off, as the app asked, failed: why, in the platform's words.
    Failed {
        /// Why.
        reason: String,
    },
}

/// What the person chose for keeping Lumenna running in the background, for the shell to do:
/// it answers with `BackgroundEvent::Offered` once done, or `Failed`.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Background {
    /// On or off.
    pub on: bool,
    /// The screen that asked, where the shell answers.
    pub screen: u32,
}

impl crux_core::capability::Operation for Background {
    type Output = ();
}

/// Settings, open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    /// Every setting, once the host has said.
    pub list: Option<Vec<Setting>>,
    /// Lines typed and not yet applied, by key.
    pub typed: BTreeMap<String, String>,
    /// What was wrong with a line applied, by key.
    pub problems: BTreeMap<String, String>,
    /// The setting on its way to the host, and the write it went out as.
    pub setting: Option<(String, u64)>,
}

/// One setting as shown.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingView {
    /// The setting, describing itself.
    pub setting: Setting,
    /// What its field shows: as typed, or its value.
    pub typed: String,
    /// What was wrong with the line last applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

/// An export Settings offers.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportChoice {
    /// What it writes.
    pub format: ExportFormat,
    /// Its button.
    pub label: String,
    /// The file's name, taken today.
    pub name: String,
}

/// Settings as shown.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsView {
    /// Every setting, once known.
    pub settings: Vec<SettingView>,
    /// The exports offered.
    pub exports: Vec<ExportChoice>,
    /// What is done to the store as a whole, a button each: Erase Everything Deleted. A
    /// shell hands one back with `Event::ActOn(ActingOn::Settings, offered)`, and the app
    /// asks its question as any row's.
    pub offered: Vec<Offered>,
    /// What to say under them, as a caption: the core's, so every app says the same.
    pub offered_caption: String,
}

/// A file for the shell to save: a backup or an export.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveFile {
    /// What to call it.
    pub name: String,
    /// What it holds.
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
    /// Its media type.
    pub kind: String,
    /// The screen that asked for it.
    pub screen: u32,
}

impl crux_core::capability::Operation for SaveFile {
    type Output = ();
}

/// Settings as shown, the exports named for `today`, with keeping Lumenna running in the
/// background where the shell offers it (`background`, as the shell last said).
#[must_use]
pub fn view(settings: &Settings, background: Option<(BackgroundKind, bool)>, today: jiff::civil::Date) -> SettingsView {
    let offered = background.map(|(kind, on)| lumenna_records::background::background_setting(kind, on));
    let rows = settings
        .list
        .iter()
        .flatten()
        .chain(settings.list.is_some().then_some(offered.as_ref()).flatten())
        .map(|setting| SettingView {
            typed: settings.typed.get(&setting.key).cloned().unwrap_or_else(|| setting.value.clone()),
            problem: settings.problems.get(&setting.key).cloned(),
            setting: setting.clone(),
        })
        .collect();
    let exports = lumenna_records::exports::EXPORTS
        .iter()
        .map(|export| ExportChoice {
            format: export.format,
            label: export.label.trim_end_matches("...").to_owned(),
            name: lumenna_records::exports::export_name(export.format, today),
        })
        .collect();
    let offered = lumenna_records::actions::store_actions().iter().map(Offered::of).collect();
    SettingsView { settings: rows, exports, offered, offered_caption: lumenna_records::actions::STORE_ACTIONS.to_owned() }
}

impl Lumenna {
    pub(crate) fn settings(&self, event: SettingsEvent, model: &mut Screen) -> Command<Effect, Event> {
        match event {
            SettingsEvent::Open => {
                model.settings = Some(Settings::default());
                render().and(host(HostRequest::Settings))
            }
            SettingsEvent::Close => {
                model.settings = None;
                model.devices = None;
                render()
            }
            SettingsEvent::Type { key, value } => {
                if let Some(settings) = &mut model.settings {
                    settings.typed.insert(key, value);
                }
                render()
            }
            SettingsEvent::Set { key, value } if key == BACKGROUND => {
                // The platform's to do: handed to the shell, which says how it went.
                let Some((kind, _)) = model.background else { return Command::done() };
                let on = value == "true";
                if let Some(settings) = &mut model.settings {
                    settings.typed.insert(key, value);
                }
                model.background_asked = Some((kind, on));
                let ask: Command<Effect, Event> = Command::notify_shell(Background { on, screen: 0 }).into();
                render().and(ask)
            }
            SettingsEvent::Set { key, value } => {
                let Some(settings) = &mut model.settings else { return Command::done() };
                settings.typed.insert(key.clone(), value.clone());
                let (number, sent) = write(model, HostRequest::SetSetting { key: key.clone(), value }, None);
                if let Some(settings) = &mut model.settings {
                    settings.setting = Some((key, number));
                }
                sent
            }
            SettingsEvent::Backup => host(HostRequest::Backup),
            SettingsEvent::Export(format) => host(HostRequest::Export(format)),
            SettingsEvent::Import { name, bytes } => host(HostRequest::Import { name, bytes }),
        }
    }

    /// What the shell says of keeping Lumenna running in the background: what it offers, or
    /// that what the app asked failed, each said where it was asked.
    pub(crate) fn background(&self, event: BackgroundEvent, model: &mut Screen) -> Command<Effect, Event> {
        if let Some(settings) = &mut model.settings {
            settings.typed.remove(BACKGROUND);
        }
        match event {
            BackgroundEvent::Offered { kind, on } => {
                model.background = Some((kind, on));
                let asked = model.background_asked.take();
                match asked {
                    Some((_, wanted)) if wanted == on => {
                        let words = background_words(kind);
                        render().and(say(if on { words.on } else { words.off }))
                    }
                    _ => render(),
                }
            }
            BackgroundEvent::Failed { reason } => {
                let kind = model.background_asked.take().map(|(kind, _)| kind).or(model.background.map(|(kind, _)| kind));
                let failed = kind.map_or_else(|| "That could not be changed".to_owned(), |kind| background_words(kind).failed);
                let reason = reason.trim();
                render().and(say(if reason.is_empty() { failed } else { format!("{failed}: {reason}") }))
            }
        }
    }

    pub(crate) fn listed(&self,result: Result<lumenna_records::types::SettingList, LumennaError>, model: &mut Screen) -> Command<Effect, Event> {
        let Some(settings) = &mut model.settings else { return Command::done() };
        match result {
            Ok(list) => {
                settings.list = Some(list.settings);
                render()
            }
            Err(error) => say(error.message()),
        }
    }

    pub(crate) fn saved(&self, result: Result<(SaveFile, String), LumennaError>) -> Command<Effect, Event> {
        match result {
        Ok((file, said)) => { let save: Command<Effect, Event> = Command::notify_shell(file).into(); save.and(say(said)) }
        Err(error) => say(error.message()),
    }
    }

    pub(crate) fn imported(&self, result: Result<String, LumennaError>, store: Option<Arrival>, model: &mut Screen) -> Command<Effect, Event> {
        if let Some(store) = store {
            model.store = Some(store);
            model.settle();
        }
        match result {
            Ok(said) => render().and(say(said)),
            Err(error) => render().and(say(error.message())),
        }
    }
}
