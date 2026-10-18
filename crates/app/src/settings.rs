//! Settings: each applied as it is made (a line when it is left), saying so; a line refused stays
//! as typed, beside what was wrong with it. Backups, exports and imports are here too: the host
//! makes the file, and the shell saves or picks it.

use std::collections::BTreeMap;

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::LumennaError;
use lumenna_records::types::{ExportFormat, Setting};
use serde::{Deserialize, Serialize};

use crate::host::{Arrival, HostRequest};
use crate::model::Model;
use crate::{Effect, Event, Lumenna, host, say};

/// What the person does in Settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
        #[cfg_attr(feature = "wasm", tsify(type = "Uint8Array"))]
        bytes: Vec<u8>,
    },
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
    /// The setting on its way to the host.
    pub setting: Option<String>,
}

/// One setting as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct ExportChoice {
    /// What it writes.
    pub format: ExportFormat,
    /// Its button.
    pub label: String,
    /// The file's name, taken today.
    pub name: String,
}

/// Settings as shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SettingsView {
    /// Every setting, once known.
    pub settings: Vec<SettingView>,
    /// The exports offered.
    pub exports: Vec<ExportChoice>,
}

/// A file for the shell to save: a backup or an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct SaveFile {
    /// What to call it.
    pub name: String,
    /// What it holds.
    #[serde(with = "serde_bytes")]
    #[cfg_attr(feature = "wasm", tsify(type = "Uint8Array"))]
    pub bytes: Vec<u8>,
    /// Its media type.
    pub kind: String,
}

impl crux_core::capability::Operation for SaveFile {
    type Output = ();
}

/// Settings as shown, the exports named for `today`.
#[must_use]
pub fn view(settings: &Settings, today: jiff::civil::Date) -> SettingsView {
    let rows = settings
        .list
        .iter()
        .flatten()
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
    SettingsView { settings: rows, exports }
}

impl Lumenna {
    pub(crate) fn settings(&self, event: SettingsEvent, model: &mut Model) -> Command<Effect, Event> {
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
            SettingsEvent::Set { key, value } => {
                let Some(settings) = &mut model.settings else { return Command::done() };
                settings.typed.insert(key.clone(), value.clone());
                settings.setting = Some(key.clone());
                host(HostRequest::SetSetting { key, value })
            }
            SettingsEvent::Backup => host(HostRequest::Backup),
            SettingsEvent::Export(format) => host(HostRequest::Export(format)),
            SettingsEvent::Import { name, bytes } => host(HostRequest::Import { name, bytes }),
        }
    }

    pub(crate) fn listed(&self, result: Result<lumenna_records::types::SettingList, LumennaError>, model: &mut Model) -> Command<Effect, Event> {
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

    pub(crate) fn imported(&self, result: Result<String, LumennaError>, store: Option<Arrival>, model: &mut Model) -> Command<Effect, Event> {
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
