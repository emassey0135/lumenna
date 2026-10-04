//! Backups, restore, export and import (§9).
//!
//! Two kinds of output, kept visibly apart as §9 asks: a **backup** is the whole history,
//! deleted tasks included, and an **export** is the present state with nothing from the
//! trash. What each announces says which it is, because confusing them is how someone emails
//! a "task list" that holds everything they ever deleted.

use std::path::{Path, PathBuf};

use jiff::{Timestamp, Zoned};
use lumenna_core::edit;
use lumenna_store::export::{self, Imported as ExportFile};
use lumenna_store::{Store, backup};

use crate::error::{LumennaError, Result};
use crate::types::{
    Announced, BackupDone, ExportFormat, Exported, ImportDone, Imported, RestoreDone,
};
use crate::words::count_line;
use crate::{Lumenna, repaired};

/// The settings that stay on this device (§3.12).
pub const DEVICE_KEYS: &[&str] = &["backup-dir", "backup-keep", "backup-every"];

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// Backs up the whole store now, history included — into the configured directory, or
    /// `to` if given.
    ///
    /// # Errors
    ///
    /// If `to` is inside the profile, or the backup cannot be written.
    pub fn backup(&self, to: Option<String>) -> Result<BackupDone> {
        let mut policy = self.backup_policy()?;
        if let Some(to) = to {
            policy.directory = absolute(Path::new(&to));
            refuse_inside(&self.directory, &policy.directory)?;
        }
        let path = self.store().back_up_to(&policy, Timestamp::now())?;
        let kept = backup::list(&policy.directory)?.len();
        let mut done = BackupDone {
            announcement: format!("Backed up to {}", path.display()),
            notices: Vec::new(),
            path: path.display().to_string(),
            kept: to_u32(kept),
        };
        done.notices.extend(cloud_warning(&policy.directory));
        Ok(done)
    }

    /// Takes a backup if the newest is older than the device's `backup-every`, and says
    /// where it went — or nothing, if none was due.
    ///
    /// §9: every client calls this opportunistically — at launch, before a one-shot command,
    /// hourly in anything resident — because a schedule only exists where something stays
    /// running.
    ///
    /// # Errors
    ///
    /// If a backup was due and could not be written.
    pub fn back_up_if_due(&self) -> Result<Option<String>> {
        let policy = self.backup_policy()?;
        let written = self.store().back_up_if_due(&policy, Timestamp::now())?;
        Ok(written.map(|path| path.display().to_string()))
    }

    /// Merges a backup into the store.
    ///
    /// Nothing in the store is lost: what the backup has that the store lacks comes in, and
    /// the rest is left alone. A task deleted after the backup was taken stays deleted.
    ///
    /// # Errors
    ///
    /// If the file cannot be read or is not a backup.
    pub fn restore(&self, path: &str) -> Result<RestoreDone> {
        let file = Path::new(path);
        let bytes = read(file)?;
        if !backup::is_backup(&bytes) {
            return Err(LumennaError::new(format!(
                "{} is not a Lumenna backup; if it is an export, import it instead",
                file.display()
            )));
        }
        self.with(|store| restore_bytes(store, file, &bytes))
    }

    /// The current state, with no history and nothing from the trash — written to `path`,
    /// or returned as [`Exported::content`] when there is none.
    ///
    /// JSON is complete and is what [`import`](Self::import) reads back. Markdown and org
    /// are task lists for reading; ics is the blocks, for any calendar.
    ///
    /// # Errors
    ///
    /// If `path` exists and `replace` is not set, or it cannot be written.
    pub fn export(
        &self,
        format: ExportFormat,
        path: Option<String>,
        replace: bool,
    ) -> Result<Exported> {
        let now = Zoned::now();
        let content = self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            Ok(match format {
                ExportFormat::Json => export::export_json(&snapshot, now.timestamp()),
                ExportFormat::Markdown => export::markdown(&snapshot, &now),
                ExportFormat::Org => export::org(&snapshot, &now),
                ExportFormat::Ics => export::ics(&snapshot, now.timestamp()),
            })
        })?;
        let word = format.word();
        let Some(path) = path else {
            return Ok(Exported {
                announcement: format!("Exported as {word}"),
                notices: Vec::new(),
                format: word.to_owned(),
                path: None,
                content: Some(content),
            });
        };
        let file = Path::new(&path);
        if file.exists() && !replace {
            return Err(LumennaError::new(format!(
                "{} already exists, and replacing it was not asked for",
                file.display()
            )));
        }
        std::fs::write(file, &content)?;
        Ok(Exported {
            announcement: format!("Exported as {word} to {}", file.display()),
            notices: Vec::new(),
            format: word.to_owned(),
            path: Some(file.display().to_string()),
            content: None,
        })
    }

    /// Reads a JSON export, or a backup, into the store — whichever the file is.
    ///
    /// Records keep their identifiers, so importing the same file twice changes nothing the
    /// second time. Nothing missing from the file is removed.
    ///
    /// # Errors
    ///
    /// If the file cannot be read, or is neither a JSON export nor a backup.
    pub fn import(&self, path: &str) -> Result<Imported> {
        let file = Path::new(path);
        let bytes = read(file)?;
        if backup::is_backup(&bytes) {
            return self
                .with(|store| restore_bytes(store, file, &bytes))
                .map(|done| Imported::Backup { done });
        }
        let text = String::from_utf8(bytes).map_err(|_| {
            LumennaError::new(format!("{} is not a Lumenna export or backup", file.display()))
        })?;
        let imported: ExportFile = export::parse_json(&text).map_err(|error| {
            LumennaError::new(format!(
                "{error}. Importing reads a JSON export or a backup; Markdown, org and \
                 iCalendar exports are for reading, and do not come back in"
            ))
        })?;
        self.with(|store| {
            let report = store.import(&imported)?;
            fold_inbox(store)?;
            let announcement = if imported.is_empty() {
                format!("{} holds nothing to import", file.display())
            } else {
                format!(
                    "Imported {}: {} new, {} updated, {} already the same",
                    file.display(),
                    report.created,
                    report.updated,
                    report.unchanged
                )
            };
            let mut done = ImportDone {
                announcement,
                notices: Vec::new(),
                created: to_u32(report.created),
                updated: to_u32(report.updated),
                unchanged: to_u32(report.unchanged),
                skipped: to_u32(report.skipped),
            };
            if report.skipped > 0 {
                done = done.note(format!(
                    "left out {} whose block is in neither the file nor this store",
                    count_line(report.skipped, "assignment")
                ));
            }
            Ok(Imported::Export { done })
        })
    }
}

fn restore_bytes(store: &mut Store, file: &Path, bytes: &[u8]) -> Result<RestoreDone> {
    let restored = store.restore(bytes)?;
    fold_inbox(store)?;
    let announcement = if restored.changed == 0 {
        format!("{} holds nothing this store does not already have", file.display())
    } else {
        format!(
            "Restored from {}: {} brought in changes",
            file.display(),
            count_line(restored.changed, "document")
        )
    };
    let mut done = RestoreDone {
        announcement,
        notices: Vec::new(),
        documents: to_u32(restored.documents),
        changed: to_u32(restored.changed),
        unknown: restored.unknown.clone(),
    };
    if !restored.unknown.is_empty() {
        done = done.note(format!(
            "left out {} this version does not know ({}); a newer Lumenna can restore them",
            count_line(restored.unknown.len(), "document"),
            restored.unknown.join(", ")
        ));
    }
    Ok(done)
}

/// An export from a store with its own Inbox, or a backup of one, brings a second Inbox in.
/// Folding it is the app's doing, not the person's, so it is not recorded for undo.
fn fold_inbox(store: &mut Store) -> Result<()> {
    let edit = edit::adopt_inbox(&repaired(store));
    if !edit.is_empty() {
        store.apply(&edit)?;
    }
    Ok(())
}

fn read(file: &Path) -> Result<Vec<u8>> {
    std::fs::read(file)
        .map_err(|error| LumennaError::new(format!("cannot read {}: {error}", file.display())))
}

pub(crate) fn absolute(path: &Path) -> PathBuf {
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
pub(crate) fn refuse_inside(profile: &Path, directory: &Path) -> Result<()> {
    if resolved(directory).starts_with(resolved(profile)) {
        return Err(LumennaError::new(format!(
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
pub fn cloud_warning(directory: &Path) -> Option<String> {
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
