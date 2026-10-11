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
            ("default-task-reminders", phrases(&settings.default_task_reminders)),
            ("default-block-reminders", phrases(&settings.default_block_reminders)),
            (
                "task-order",
                match settings.task_order {
                    lumenna_core::filter::Sort::Urgency => "urgency".to_owned(),
                    lumenna_core::filter::Sort::Added => "added".to_owned(),
                },
            ),
            ("urgency-weights", weights_phrase(&settings.urgency)),
        ]
        .into_iter()
        .map(|(key, value)| described(key, value))
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

    /// Changes a setting. Most sync to every device; `backup-dir`, `backup-keep`,
    /// `backup-every` and `clock` are this device's alone, and say so.
    ///
    /// # Errors
    ///
    /// If there is no such setting, or the value cannot be read.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<Change> {
        if DEVICE_KEYS.contains(&key) {
            return self.set_device_setting(key, value);
        }
        self.told(|store| {
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
                "default-task-reminders" => after.default_task_reminders = triggers(value, false)?,
                "default-block-reminders" => after.default_block_reminders = triggers(value, true)?,
                "urgency-weights" => after.urgency = weights(value)?,
                "task-order" => {
                    after.task_order = match value.to_lowercase().as_str() {
                        "urgency" => lumenna_core::filter::Sort::Urgency,
                        "added" => lumenna_core::filter::Sort::Added,
                        _ => return Err(LumennaError::new("task order is urgency or added")),
                    };
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
        self.told(|store| {
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
    pub(crate) fn device_setting(&self, key: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.device_settings_path()).ok()?;
        text.lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_owned())
    }

    /// Writes a device setting; `None` puts it back to its default.
    pub(crate) fn write_device_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
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
            described("backup-dir", policy.directory.display().to_string()),
            described("backup-keep", policy.keep.to_string()),
            described(
                "backup-every",
                policy.every.map_or_else(
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
            ),
            // For a client with no clock setting of its own to follow, as a terminal has none.
            described("clock", self.device_setting("clock").unwrap_or_else(|| "24-hour".to_owned())),
            described("reminders", self.device_setting("reminders").unwrap_or_else(|| "true".to_owned())),
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
            "reminders" => {
                let on = resolve::yes_or_no(value)?;
                self.write_device_setting(key, if on { None } else { Some("false") })?;
            }
            "clock" => {
                let clock = match value.trim().to_lowercase().as_str() {
                    "24-hour" | "24" => "24-hour",
                    "12-hour" | "12" => "12-hour",
                    _ => return Err(LumennaError::new("clock is 24-hour or 12-hour")),
                };
                self.write_device_setting(key, Some(clock))?;
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

/// A setting with what every settings screen says of it: its name, its control, what it
/// can be, whether it syncs. Nine clients each had their own copy of this table.
/// What a time setting takes.
const TIME: &str = "A time, such as 8:00 or 8am.";

fn described(key: &str, value: String) -> Setting {
    use crate::types::SettingKind as K;
    let options = |pairs: &[(&str, &str)]| -> Vec<crate::actions::Choice> {
        pairs
            .iter()
            .map(|(id, title)| crate::actions::Choice { id: (*id).to_owned(), title: (*title).to_owned(), ..Default::default() })
            .collect()
    };
    let weekdays = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];
    let (title, kind, options, hint) = match key {
        "cascade-complete-subtasks" => (
            "Completing a task completes its subtasks",
            K::Toggle,
            options(&[("true", "On"), ("false", "Off")]),
            "",
        ),
        "verbosity" => ("Announcements", K::Choice, options(&[("full", "Full sentences"), ("terse", "Terse")]), ""),
        "all-day-reminder-hour" => ("All-day reminders at", K::Time, Vec::new(), TIME),
        "day-start" => ("Day starts", K::Time, Vec::new(), TIME),
        "day-end" => ("Day ends", K::Time, Vec::new(), TIME),
        "week-start" => (
            "Week starts on",
            K::Choice,
            weekdays
                .iter()
                .map(|day| crate::actions::Choice {
                    id: (*day).to_owned(),
                    title: format!("{}{}", day[..1].to_uppercase(), &day[1..]),
                    ..Default::default()
                })
                .collect(),
            "",
        ),
        "backup-every" => (
            "Automatic backups",
            K::Choice,
            options(&[("12h", "Every 12 hours"), ("1d", "Every day"), ("7d", "Every week"), ("off", "Off")]),
            "A backup holds your whole history, including every task you deleted. It stays on this device.",
        ),
        "backup-keep" => ("Backups kept", K::Number, Vec::new(), ""),
        "backup-dir" => ("Backups go to", K::Folder, Vec::new(), ""),
        "reminders" => (
            "Reminders on this device",
            K::Toggle,
            options(&[("true", "On"), ("false", "Off")]),
            "Off silences them here alone; every other device still gives them.",
        ),
        "default-task-reminders" => (
            "Reminders for a task with a due date",
            K::Line,
            Vec::new(),
            "Such as 30m before, or at due; several separated by commas. Empty for none. A task with reminders of its own takes those instead.",
        ),
        "default-block-reminders" => (
            "Reminders for a block",
            K::Line,
            Vec::new(),
            "Such as 5m before start, or at end; several separated by commas. Empty for none. A block with reminders of its own takes those instead.",
        ),
        "task-order" => (
            "Task lists are in",
            K::Choice,
            options(&[("urgency", "Most urgent first"), ("added", "The order added")]),
            "A subtask stays under its task either way. A list's query can end in sort: urgency or sort: added to choose for itself.",
        ),
        "urgency-weights" => (
            "How urgency is weighed",
            K::Line,
            Vec::new(),
            "What each part adds to a task's urgency, which orders suggestions: due (the most, at a week overdue), priority (p1 to p4), age (the most, at a year), started, blocking, blocked, in a block today. A part left out, or the whole line empty, takes its default.",
        ),
        "clock" => ("Clock", K::Choice, options(&[("12-hour", "12-hour"), ("24-hour", "24-hour")]), ""),
        _ => ("", K::Choice, Vec::new(), ""),
    };
    Setting {
        key: key.to_owned(),
        value,
        title: title.to_owned(),
        kind,
        options,
        syncs: !DEVICE_KEYS.contains(&key),
        hint: hint.to_owned(),
    }
}

/// Triggers as the defaults' setting shows them: "5m before start, at end".
fn phrases(triggers: &[lumenna_core::model::Trigger]) -> String {
    triggers.iter().map(crate::reminders::trigger_phrase).collect::<Vec<_>>().join(", ")
}

/// The defaults' setting read back: phrases separated by commas, or empty for none.
fn triggers(text: &str, block: bool) -> Result<Vec<lumenna_core::model::Trigger>> {
    text.split(',')
        .map(str::trim)
        .filter(|phrase| !phrase.is_empty() && !phrase.eq_ignore_ascii_case("none"))
        .map(|phrase| crate::reminders::parse_trigger(phrase, block))
        .collect()
}

/// The urgency weights as the setting says them: "due 12, priority 6 3.9 1.8 0, age 2, …".
fn weights_phrase(c: &lumenna_core::urgency::Coefficients) -> String {
    let [p1, p2, p3, p4] = c.priority;
    format!(
        "due {}, priority {p1} {p2} {p3} {p4}, age {}, started {}, blocking {}, blocked {}, in a block today {}",
        c.due, c.age, c.started, c.blocking, c.blocked, c.assigned_today
    )
}

/// Reads the urgency weights' line: parts separated by commas, each a name and its number
/// (priority four, p1 first). A part left out takes its default.
fn weights(value: &str) -> Result<lumenna_core::urgency::Coefficients> {
    let mut c = lumenna_core::urgency::Coefficients::default();
    for part in value.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let split = part.find(|ch: char| ch == '-' || ch == '+' || ch.is_ascii_digit() || ch == '.').unwrap_or(part.len());
        let (name, numbers) = part.split_at(split);
        let numbers: Vec<f32> = numbers
            .split_whitespace()
            .map(|n| n.parse::<f32>().ok().filter(|n| n.is_finite()))
            .collect::<Option<_>>()
            .ok_or_else(|| LumennaError::new(format!("'{part}' needs a number after its name, such as due 12")))?;
        let one = || match numbers.as_slice() {
            [n] => Ok(*n),
            _ => Err(LumennaError::new(format!("'{part}' takes one number"))),
        };
        match name.trim().to_lowercase().as_str() {
            "due" => c.due = one()?,
            "age" => c.age = one()?,
            "started" => c.started = one()?,
            "blocking" => c.blocking = one()?,
            "blocked" => c.blocked = one()?,
            "in a block today" => c.assigned_today = one()?,
            "priority" => {
                c.priority = <[f32; 4]>::try_from(numbers.as_slice())
                    .map_err(|_| LumennaError::new("priority takes four numbers, p1 to p4, such as priority 6 3.9 1.8 0"))?;
            }
            other => {
                return Err(LumennaError::new(format!(
                    "'{other}' is not part of urgency: due, priority, age, started, blocking, blocked, or in a block today"
                )));
            }
        }
    }
    Ok(c)
}

#[cfg(test)]
mod weight_tests {
    use super::*;

    #[test]
    fn the_weights_read_back_as_they_are_said() {
        let said = weights_phrase(&lumenna_core::urgency::Coefficients::default());
        assert_eq!(said, "due 12, priority 6 3.9 1.8 0, age 2, started 4, blocking 8, blocked -5, in a block today -1");
        assert_eq!(weights(&said).unwrap(), lumenna_core::urgency::Coefficients::default());
        assert_eq!(weights("").unwrap(), lumenna_core::urgency::Coefficients::default());
        let changed = weights("due 20, blocked -8").unwrap();
        assert_eq!((changed.due, changed.blocked, changed.age), (20.0, -8.0, 2.0));
        assert!(weights("priority 1 2").is_err());
        assert!(weights("deadline 3").is_err());
        assert!(weights("due soon").is_err());
    }
}
