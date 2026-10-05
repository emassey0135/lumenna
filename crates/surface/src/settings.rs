//! Settings, the device's own settings, and undo.

use std::path::{Path, PathBuf};

use lumenna_core::edit;
use lumenna_core::model::Verbosity;
use lumenna_store::backup::Policy;
use lumenna_store::undo::Step;

use crate::durability::{absolute, refuse_inside};
use crate::error::{LumennaError, Result};
use crate::resolve;
use crate::types::{Announced, Change, Setting, SettingList};
use crate::words::{count_line, time_text};
use crate::{DEVICE_KEYS, Lumenna, cloud_warning, repaired};

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Every setting, in a fixed order — or the one named by `key`. The device's own settings
    /// come last.
    ///
    /// # Errors
    ///
    /// If there is no setting called `key`.
    pub fn settings(&self, key: Option<String>) -> Result<SettingList> {
        let settings = self.with(|store| Ok(repaired(store).settings))?;
        let all: Vec<Setting> = [
            ("cascade-complete-subtasks", settings.cascade_complete_subtasks.to_string()),
            (
                "verbosity",
                match settings.verbosity {
                    Verbosity::Terse => "terse".to_owned(),
                    Verbosity::Full => "full".to_owned(),
                },
            ),
            // `HH:MM`, as every time is shown and typed; jiff's own form carries seconds,
            // which nothing here has.
            ("all-day-reminder-hour", time_text(settings.all_day_reminder_hour)),
            ("day-start", time_text(settings.day_window.0)),
            ("day-end", time_text(settings.day_window.1)),
            ("week-start", format!("{:?}", settings.week_start).to_lowercase()),
        ]
        .into_iter()
        .map(|(key, value)| Setting { key: key.to_owned(), value })
        .chain(self.device_settings()?)
        .collect();

        match key {
            Some(key) => {
                let found = all
                    .into_iter()
                    .find(|setting| setting.key == key)
                    .ok_or_else(|| LumennaError::new(format!("no setting called '{key}'")))?;
                Ok(SettingList {
                    announcement: found.value.clone(),
                    notices: Vec::new(),
                    settings: vec![found],
                })
            }
            None => Ok(SettingList {
                announcement: count_line(all.len(), "setting"),
                notices: Vec::new(),
                settings: all,
            }),
        }
    }

    /// Changes a setting. Most sync to every device; `backup-dir`, `backup-keep` and
    /// `backup-every` are this device's alone, and say so.
    ///
    /// # Errors
    ///
    /// If there is no such setting, or the value cannot be read.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<Change> {
        if DEVICE_KEYS.contains(&key) {
            return self.set_device_setting(key, value);
        }
        self.with(|store| {
            let before = repaired(store).settings;
            let mut after = before.clone();
            match key {
                "cascade-complete-subtasks" => {
                    after.cascade_complete_subtasks = resolve::yes_or_no(value)?;
                }
                "verbosity" => {
                    after.verbosity = match value.to_lowercase().as_str() {
                        "terse" => Verbosity::Terse,
                        "full" => Verbosity::Full,
                        _ => return Err(LumennaError::new("verbosity is terse or full")),
                    };
                }
                "all-day-reminder-hour" => after.all_day_reminder_hour = resolve::time(value)?,
                "day-start" => after.day_window.0 = resolve::time(value)?,
                "day-end" => after.day_window.1 = resolve::time(value)?,
                "week-start" => {
                    after.week_start = lumenna_parse::date::weekday_of(&value.to_lowercase())
                        .ok_or_else(|| {
                            LumennaError::new(format!("'{value}' is not a day of the week"))
                        })?;
                }
                other => return Err(LumennaError::new(format!("no setting called '{other}'"))),
            }
            let change = edit::update_settings(before, after);
            if change.is_empty() {
                return Ok(Change::unchanged("nothing changed"));
            }
            store.apply_recorded(&change)?;
            Ok(Change::announced(format!("{key} is now {value}"), &change))
        })
    }

    /// Undoes the last change made on this device.
    ///
    /// The history is this device's alone and survives restarts, so this reverses what the
    /// last command did whichever client did it. Anything changed since by something else is
    /// kept, and each such field is a notice. Always announced: without a visual channel, an
    /// undo that did less than asked can go unnoticed for minutes.
    ///
    /// # Errors
    ///
    /// If the history or the store cannot be read.
    pub fn undo(&self) -> Result<Change> {
        self.step(false)
    }

    /// Redoes the change most recently undone.
    ///
    /// # Errors
    ///
    /// If the history or the store cannot be read.
    pub fn redo(&self) -> Result<Change> {
        self.step(true)
    }
}

impl Lumenna {
    fn step(&self, redo: bool) -> Result<Change> {
        self.with(|store| {
            let taken = if redo { store.redo()? } else { store.undo()? };
            Ok(match taken {
                Step::Nothing => {
                    Change::unchanged(if redo { "nothing to redo" } else { "nothing to undo" })
                }
                Step::Unreadable => Change::unchanged(
                    "the last change was saved by a different version of Lumenna and cannot \
                     be reversed by this one; it has been set aside, and the one before it is \
                     next",
                ),
                Step::Done(reverted) => {
                    let verb = if redo { "Redid" } else { "Undid" };
                    let mut change = Change::announced(
                        format!("{verb}: {}", reverted.description),
                        &reverted.applied,
                    );
                    if reverted.applied.is_empty() && reverted.kept.is_empty() {
                        change = change.note("it was already that way");
                    }
                    for kept in reverted.kept {
                        change = change.note(kept);
                    }
                    change
                }
            })
        })
    }

    /// Where this device's own settings live: a file in the profile directory, never in a
    /// document, because they are about this machine and must not sync. Where this
    /// laptop keeps its backups means nothing on a phone.
    fn device_settings_path(&self) -> PathBuf {
        self.directory.join("device-settings")
    }

    /// A setting that belongs to this device alone.
    fn device_setting(&self, key: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.device_settings_path()).ok()?;
        text.lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_owned())
    }

    /// Writes a device setting; `None` puts it back to its default.
    fn write_device_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
        let text = std::fs::read_to_string(self.device_settings_path()).unwrap_or_default();
        let mut lines: Vec<String> = text
            .lines()
            .filter(|line| line.split_once('\t').is_some_and(|(k, _)| k != key))
            .map(ToOwned::to_owned)
            .collect();
        if let Some(value) = value {
            lines.push(format!("{key}\t{value}"));
        }
        let mut body = lines.join("\n");
        if !body.is_empty() {
            body.push('\n');
        }
        std::fs::write(self.device_settings_path(), body)?;
        Ok(())
    }

    /// Where backups go when nothing says otherwise: beside the profile directory, never
    /// inside it.
    ///
    /// Backups belong outside the live database's directory, so that a bug that corrupts
    /// one cannot take both, and inside the platform data directory rather than anywhere a
    /// cloud service syncs. A sibling of the profile is both. `LUMENNA_BACKUP_DIR` overrides
    /// it, which is mostly for tests and for anyone running a second profile.
    #[must_use]
    pub fn default_backup_directory(&self) -> PathBuf {
        if let Some(explicit) = std::env::var_os("LUMENNA_BACKUP_DIR") {
            return PathBuf::from(explicit);
        }
        let name = self
            .directory
            .file_name()
            .map_or_else(|| "lumenna".to_owned(), |n| n.to_string_lossy().into_owned());
        self.directory.parent().map_or_else(
            || self.directory.join("..").join(format!("{name}-backups")),
            |parent| parent.join(format!("{name}-backups")),
        )
    }

    /// The backup policy in force on this device.
    ///
    /// # Errors
    ///
    /// If a stored device setting cannot be read — which only a hand-edited file produces.
    pub fn backup_policy(&self) -> Result<Policy> {
        let directory = self
            .device_setting("backup-dir")
            .map_or_else(|| self.default_backup_directory(), PathBuf::from);
        let keep = match self.device_setting("backup-keep") {
            Some(text) => parse_keep(&text)?,
            None => Policy::DEFAULT_KEEP,
        };
        let every = match self.device_setting("backup-every") {
            Some(text) => parse_every(&text)?,
            None => Some(Policy::DEFAULT_EVERY),
        };
        Ok(Policy { directory, keep, every })
    }

    /// The device settings' current values.
    fn device_settings(&self) -> Result<Vec<Setting>> {
        let policy = self.backup_policy()?;
        Ok(vec![
            Setting { key: "backup-dir".to_owned(), value: policy.directory.display().to_string() },
            Setting { key: "backup-keep".to_owned(), value: policy.keep.to_string() },
            Setting {
                key: "backup-every".to_owned(),
                value: policy.every.map_or_else(
                    || "off".to_owned(),
                    |every| {
                        let hours = every.as_hours();
                        if hours % 24 == 0 {
                            format!("{}d", hours / 24)
                        } else {
                            format!("{hours}h")
                        }
                    },
                ),
            },
        ])
    }

    fn set_device_setting(&self, key: &str, value: &str) -> Result<Change> {
        let mut notices = Vec::new();
        match key {
            "backup-dir" => {
                if value.eq_ignore_ascii_case("default") {
                    self.write_device_setting(key, None)?;
                } else {
                    let directory = absolute(Path::new(value));
                    refuse_inside(&self.directory, &directory)?;
                    self.write_device_setting(key, Some(&directory.display().to_string()))?;
                    // Say this where the location is chosen, not in documentation.
                    notices.push(
                        "backups hold your whole history, including every task you have \
                         deleted; keep them somewhere only you can read"
                            .to_owned(),
                    );
                    notices.extend(cloud_warning(&directory));
                }
            }
            "backup-keep" => {
                let keep = parse_keep(value)?;
                self.write_device_setting(key, Some(&keep.to_string()))?;
            }
            "backup-every" => {
                parse_every(value)?;
                self.write_device_setting(key, Some(&value.trim().to_lowercase()))?;
            }
            other => return Err(LumennaError::new(format!("no setting called '{other}'"))),
        }
        let effective = self
            .device_settings()?
            .into_iter()
            .find(|setting| setting.key == key)
            .map_or_else(|| value.to_owned(), |setting| setting.value);
        let mut change = Change::local(format!("{key} is now {effective}, on this device only"));
        change.notices = notices;
        Ok(change)
    }
}

/// How many backups to keep: a whole number, at least one.
///
/// # Errors
///
/// If it is not.
pub fn parse_keep(text: &str) -> Result<usize> {
    text.trim()
        .parse::<usize>()
        .ok()
        .filter(|n| *n >= 1)
        .ok_or_else(|| LumennaError::new(format!("'{text}' is not a number of backups to keep")))
}

/// How often a backup is due: `24h`, `7d`, a bare number of hours, or `off`.
///
/// # Errors
///
/// If it is none of those.
pub fn parse_every(text: &str) -> Result<Option<jiff::SignedDuration>> {
    let text = text.trim().to_lowercase();
    if matches!(text.as_str(), "off" | "never" | "no") {
        return Ok(None);
    }
    let (number, hours_per) = match text.strip_suffix('d') {
        Some(days) => (days, 24),
        None => (text.strip_suffix('h').unwrap_or(&text), 1),
    };
    number
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .map(|n| Some(jiff::SignedDuration::from_hours(n * hours_per)))
        .ok_or_else(|| {
            LumennaError::new(format!(
                "'{text}' is not an interval; say something like 24h, 7d, or off"
            ))
        })
}
