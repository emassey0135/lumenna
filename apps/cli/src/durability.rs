//! `lum backup`, `restore`, `export` and `import` (§9).
//!
//! The work is in `store` — every client backs up, not just this one — and what is here is the
//! policy a terminal user sees: where backups go, what is said about them, and which file
//! format a word on the command line means.
//!
//! Two kinds of output, kept visibly apart as §9 asks: a **backup** is the whole history,
//! deleted tasks included, and an **export** is the present state with nothing from the
//! trash. The commands, their help, and what they announce all say which is which, because
//! confusing them is how someone emails a "task list" that holds everything they ever deleted.

use std::path::{Path, PathBuf};

use jiff::{Timestamp, Zoned};
use lumenna_core::edit;
use lumenna_store::backup;
use lumenna_store::export::{self, Imported};

use crate::api::{self, Outcome, Response};
use crate::error::{CliError, Result};
use crate::profile::Profile;
use crate::{render, state};

/// What `lum export` writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ExportFormat {
    /// Everything current, structured — the one `lum import` reads back.
    #[default]
    Json,
    /// Tasks as a checklist, for reading.
    #[value(alias = "md")]
    Markdown,
    /// Tasks as an org outline, for Emacs.
    Org,
    /// Blocks as an iCalendar file, for any calendar.
    #[value(alias = "ical", alias = "icalendar")]
    Ics,
}

impl ExportFormat {
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "markdown",
            Self::Org => "org",
            Self::Ics => "ics",
        }
    }

    /// The word a client sent, for `lum rpc`.
    pub(crate) fn from_word(word: &str) -> Option<Self> {
        match word.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "markdown" | "md" => Some(Self::Markdown),
            "org" => Some(Self::Org),
            "ics" | "ical" | "icalendar" => Some(Self::Ics),
            _ => None,
        }
    }
}

/// `lum backup`: one now, into the configured directory or the one given.
pub(crate) fn backup(profile: &mut Profile, to: Option<&Path>) -> Result<Response> {
    let mut policy = profile.backup_policy()?;
    if let Some(to) = to {
        policy.directory = absolute(to);
        refuse_inside_profile(profile, &policy.directory)?;
    }
    let path = profile.store.back_up_to(&policy, Timestamp::now())?;
    let kept = backup::list(&policy.directory)?.len();
    let mut response = Response::new(
        format!("Backed up to {}", path.display()),
        Outcome::Backup(api::BackupDone { path: path.display().to_string(), kept }),
    );
    if let Some(warning) = cloud_warning(&policy.directory) {
        response = response.note(warning);
    }
    Ok(response)
}

/// `lum restore`: merges a backup in.
pub(crate) fn restore(profile: &mut Profile, file: &Path) -> Result<Response> {
    let bytes = read(file)?;
    if !backup::is_backup(&bytes) {
        return Err(CliError::Message(format!(
            "{} is not a Lumenna backup; if it is an export, use `lum import`",
            file.display()
        )));
    }
    restore_bytes(profile, file, &bytes)
}

fn restore_bytes(profile: &mut Profile, file: &Path, bytes: &[u8]) -> Result<Response> {
    let restored = profile.store.restore(bytes)?;
    fold_inbox(profile)?;
    let announcement = if restored.changed == 0 {
        format!("{} holds nothing this store does not already have", file.display())
    } else {
        format!(
            "Restored from {}: {} brought in changes",
            file.display(),
            render::count_line(restored.changed, "document")
        )
    };
    let mut response = Response::new(
        announcement,
        Outcome::Restore(api::RestoreDone {
            documents: restored.documents,
            changed: restored.changed,
            unknown: restored.unknown.clone(),
        }),
    );
    if !restored.unknown.is_empty() {
        response = response.note(format!(
            "left out {} this version does not know ({}); a newer Lumenna can restore them",
            render::count_line(restored.unknown.len(), "document"),
            restored.unknown.join(", ")
        ));
    }
    Ok(response)
}

/// `lum export`: the present state, to standard output or a file.
pub(crate) fn export(
    profile: &mut Profile,
    format: ExportFormat,
    output: Option<&Path>,
    force: bool,
    now: &Zoned,
) -> Result<Response> {
    profile.store.load_all_years()?;
    let snapshot = state(profile);
    let content = match format {
        ExportFormat::Json => export::export_json(&snapshot, now.timestamp()),
        ExportFormat::Markdown => export::markdown(&snapshot, now),
        ExportFormat::Org => export::org(&snapshot, now),
        ExportFormat::Ics => export::ics(&snapshot, now.timestamp()),
    };
    let Some(output) = output else {
        return Ok(Response::new(
            format!("Exported as {}", format.word()),
            Outcome::Export(api::Exported {
                format: format.word(),
                path: None,
                content: Some(content),
            }),
        ));
    };
    if output.exists() && !force {
        return Err(CliError::Message(format!(
            "{} already exists; pass --force to replace it",
            output.display()
        )));
    }
    std::fs::write(output, &content)?;
    Ok(Response::new(
        format!("Exported as {} to {}", format.word(), output.display()),
        Outcome::Export(api::Exported {
            format: format.word(),
            path: Some(output.display().to_string()),
            content: None,
        }),
    ))
}

/// `lum import`: a JSON export, or a backup — whichever the file is.
pub(crate) fn import(profile: &mut Profile, file: &Path) -> Result<Response> {
    let bytes = read(file)?;
    if backup::is_backup(&bytes) {
        return restore_bytes(profile, file, &bytes);
    }
    let text = String::from_utf8(bytes).map_err(|_| unreadable(file))?;
    let imported: Imported = export::parse_json(&text).map_err(|error| {
        CliError::Message(format!(
            "{error}. `lum import` reads a JSON export or a backup; Markdown, org and \
             iCalendar exports are for reading, and do not come back in"
        ))
    })?;
    let report = profile.store.import(&imported)?;
    fold_inbox(profile)?;
    let mut announcement = format!(
        "Imported {}: {} new, {} updated, {} already the same",
        file.display(),
        report.created,
        report.updated,
        report.unchanged
    );
    if imported.is_empty() {
        announcement = format!("{} holds nothing to import", file.display());
    }
    let mut response = Response::new(
        announcement,
        Outcome::Import(api::ImportDone {
            created: report.created,
            updated: report.updated,
            unchanged: report.unchanged,
            skipped: report.skipped,
        }),
    );
    if report.skipped > 0 {
        response = response.note(format!(
            "left out {} whose block is in neither the file nor this store",
            render::count_line(report.skipped, "assignment")
        ));
    }
    Ok(response)
}

/// An export from a store with its own Inbox, or a backup of one, brings a second Inbox in.
fn fold_inbox(profile: &mut Profile) -> Result<()> {
    let edit = edit::adopt_inbox(&state(profile));
    if !edit.is_empty() {
        profile.store.apply(&edit)?;
    }
    Ok(())
}

fn read(file: &Path) -> Result<Vec<u8>> {
    std::fs::read(file)
        .map_err(|error| CliError::Message(format!("cannot read {}: {error}", file.display())))
}

fn unreadable(file: &Path) -> CliError {
    CliError::Message(format!("{} is not a Lumenna export or backup", file.display()))
}

/// Takes a backup if one is due, before a command runs.
///
/// §9: every client does this opportunistically, because a schedule only exists where
/// something stays running. Before the command rather than after, so that a command about to
/// do damage is preceded by the backup that undoes it. Quiet when it works — a daily backup
/// announced on every first command of the day is noise — and said once when it does not.
pub(crate) fn back_up_if_due(profile: &mut Profile) {
    let result = profile
        .backup_policy()
        .and_then(|policy| Ok(profile.store.back_up_if_due(&policy, Timestamp::now())?));
    if let Err(error) = result {
        anstream::eprintln!("lum: the automatic backup failed: {error}");
    }
}

/// The device settings `lum config` handles, which stay on this device (§3.12).
pub(crate) const DEVICE_KEYS: &[&str] = &["backup-dir", "backup-keep", "backup-every"];

/// Their current values, for `lum config get`.
pub(crate) fn device_settings(profile: &Profile) -> Result<Vec<api::Setting>> {
    let policy = profile.backup_policy()?;
    Ok(vec![
        api::Setting { key: "backup-dir".to_owned(), value: policy.directory.display().to_string() },
        api::Setting { key: "backup-keep".to_owned(), value: policy.keep.to_string() },
        api::Setting {
            key: "backup-every".to_owned(),
            value: policy.every.map_or_else(
                || "off".to_owned(),
                |every| {
                    let hours = every.as_hours();
                    if hours % 24 == 0 { format!("{}d", hours / 24) } else { format!("{hours}h") }
                },
            ),
        },
    ])
}

/// `lum config set` for a device setting.
pub(crate) fn set_device_setting(profile: &Profile, key: &str, value: &str) -> Result<Response> {
    let mut notices = Vec::new();
    match key {
        "backup-dir" => {
            if value.eq_ignore_ascii_case("default") {
                profile.set_device_setting(key, None)?;
            } else {
                let directory = absolute(Path::new(value));
                refuse_inside_profile(profile, &directory)?;
                profile.set_device_setting(key, Some(&directory.display().to_string()))?;
                // §9: say this where the location is chosen, not in documentation.
                notices.push(
                    "backups hold your whole history, including every task you have deleted; \
                     keep them somewhere only you can read"
                        .to_owned(),
                );
                notices.extend(cloud_warning(&directory));
            }
        }
        "backup-keep" => {
            let keep = crate::profile::parse_keep(value)?;
            profile.set_device_setting(key, Some(&keep.to_string()))?;
        }
        "backup-every" => {
            crate::profile::parse_every(value)?;
            profile.set_device_setting(key, Some(&value.trim().to_lowercase()))?;
        }
        other => return Err(CliError::Message(format!("no setting called '{other}'"))),
    }
    let effective = device_settings(profile)?
        .into_iter()
        .find(|setting| setting.key == key)
        .map_or_else(|| value.to_owned(), |setting| setting.value);
    let mut response = Response::touched(
        format!("{key} is now {effective}, on this device only"),
        api::Affected::default(),
    );
    for notice in notices {
        response = response.note(notice);
    }
    Ok(response)
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    }
}

/// A path with symbolic links resolved, as far as it exists.
///
/// A backup directory is usually chosen before it exists, so only its nearest existing
/// ancestor can be resolved; the rest is carried over as written. Without that, a link along
/// the way — macOS reaches its temporary and home directories through them — makes a path
/// inside the profile look outside it.
fn resolved(path: &Path) -> PathBuf {
    let path = absolute(path);
    let mut existing = path.as_path();
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = existing.canonicalize() {
            return rest.iter().rev().fold(real, |acc: PathBuf, part| acc.join(part));
        }
        match (existing.parent(), existing.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                existing = parent;
            }
            _ => return path,
        }
    }
}

/// §9: a backup inside the directory it protects can be lost along with it.
fn refuse_inside_profile(profile: &Profile, directory: &Path) -> Result<()> {
    if resolved(directory).starts_with(resolved(profile.directory())) {
        return Err(CliError::Message(format!(
            "{} is inside the profile directory; a backup there can be lost along with the \
             store it is meant to protect",
            directory.display()
        )));
    }
    Ok(())
}

/// A sentence saying a directory is probably synced to someone else's server, if it is.
///
/// §9: never default to a cloud-synced directory, and if the user chooses one, *say so plainly
/// rather than silently complying*. Recognised by the folder names the common services use;
/// on macOS, Desktop and Documents too, which iCloud syncs when that option is on.
#[must_use]
pub(crate) fn cloud_warning(directory: &Path) -> Option<String> {
    let service = cloud_service(directory)?;
    Some(format!(
        "{} looks like it is synced by {service}, so backups there leave this device for \
         someone else's server — with your whole history in them. That is your call to make; \
         it is not where Lumenna puts them by default",
        directory.display()
    ))
}

fn cloud_service(directory: &Path) -> Option<&'static str> {
    const SERVICES: &[(&str, &str)] = &[
        ("mobile documents", "iCloud"),
        ("icloud drive", "iCloud"),
        ("icloud", "iCloud"),
        ("cloudstorage", "a cloud storage service"),
        ("dropbox", "Dropbox"),
        ("onedrive", "OneDrive"),
        ("google drive", "Google Drive"),
        ("googledrive", "Google Drive"),
        ("my drive", "Google Drive"),
        ("nextcloud", "Nextcloud"),
        ("owncloud", "ownCloud"),
        ("pcloud", "pCloud"),
        ("box sync", "Box"),
        ("mega", "MEGA"),
    ];
    for component in directory.components() {
        let name = component.as_os_str().to_string_lossy().to_lowercase();
        // The folder name itself, or it followed by anything that is not part of a word:
        // "OneDrive - Work", "Dropbox (Personal)".
        let names = |folder: &str| {
            name.strip_prefix(folder)
                .is_some_and(|rest| rest.chars().next().is_none_or(|c| !c.is_alphanumeric()))
        };
        if let Some((_, service)) = SERVICES.iter().find(|(folder, _)| names(folder)) {
            return Some(service);
        }
    }
    if cfg!(target_os = "macos")
        && let Some(home) = std::env::var_os("HOME").map(PathBuf::from)
        && ["Desktop", "Documents"].iter().any(|d| directory.starts_with(home.join(d)))
    {
        return Some("iCloud, if Desktop and Documents syncing is on");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_common_sync_folders_are_recognised() {
        assert_eq!(cloud_service(Path::new("/Users/me/Dropbox/backups")), Some("Dropbox"));
        assert_eq!(
            cloud_service(Path::new("/Users/me/Library/Mobile Documents/com~apple~CloudDocs")),
            Some("iCloud")
        );
        assert_eq!(cloud_service(Path::new("C:/Users/me/OneDrive - Work/x")), Some("OneDrive"));
        assert_eq!(cloud_service(Path::new("/home/me/.local/share/lumenna-backups")), None);
    }
}
