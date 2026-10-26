//! Backups, restore, export and import.
//!
//! Two kinds of output, kept visibly apart: a **backup** is the whole history,
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
    Announced, BackupDone, BackupFile, ExportFormat, Exported, ImportDone, Imported, RestoreDone,
};
use crate::words::{count_line, to_u32};
use crate::{Lumenna, repaired};

/// The settings that stay on this device.
pub const DEVICE_KEYS: &[&str] = &["backup-dir", "backup-keep", "backup-every", "clock", "reminders"];

/// Which generation of the store's history this device's automatic backups were last taken
/// of (empty for the first): the store's own record, so every process on the device acts on
/// an erasure once.
const BACKUPS_OF: &str = "backups-of";

impl Lumenna {
    /// After a real erasure, here or taken in from another device, by this process or another:
    /// the automatic backups, which hold what was erased, are deleted, and a fresh one is
    /// taken of the new history when automatic backups are on. Looked at whenever the store's
    /// generation is not the one last seen, so it costs nothing otherwise.
    pub(crate) fn follow_erasure(&self, store: &mut Store) -> Result<()> {
        let generation = store.generation().unwrap_or_default().to_owned();
        let mut seen = self.generation_seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if seen.as_deref() == Some(generation.as_str()) {
            return Ok(());
        }
        let backed_up = store.local_value(BACKUPS_OF)?.map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
        if backed_up.as_deref().unwrap_or_default() != generation {
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            {
                let policy = self.backup_policy()?;
                backup::remove_all(&policy.directory)?;
                if policy.every.is_some() {
                    store.back_up_to(&policy, Timestamp::now())?;
                }
            }
            store.set_local_value(BACKUPS_OF, generation.as_bytes())?;
        }
        *seen = Some(generation);
        Ok(())
    }

    /// Which generation of the store's history this device holds, empty for the first: for a
    /// client that keeps its automatic backups where this process cannot reach (the web's,
    /// in a folder the person chose), to delete them when it moves on after an erasure, as
    /// [`follow_erasure`](Self::follow_erasure) does everywhere else.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn generation(&self) -> Result<String> {
        self.with(|store| Ok(store.generation().unwrap_or_default().to_owned()))
    }

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
        let written = self.store().back_up_to(&policy, Timestamp::now())?;
        let path = written.path;
        let kept = backup::list(&policy.directory)?.len();
        let mut done = BackupDone {
            announcement: format!("Backed up to {}", path.display()),
            notices: Vec::new(),
            path: path.display().to_string(),
            kept: to_u32(kept),
        };
        done.notices.extend(written.pruning_failed);
        done.notices.extend(cloud_warning(&policy.directory));
        Ok(done)
    }

    /// A backup taken now, as a file's contents rather than a file: for a client that saves
    /// it where the person chooses — the browser, as a download. It is a whole backup, history
    /// and trash included, which [`import_bytes`](Self::import_bytes) or
    /// [`restore`](Self::restore) read back.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn backup_file(&self) -> Result<BackupFile> {
        let bytes = self.store().backup()?;
        Ok(BackupFile {
            announcement: "Backed up, the whole history and the trash included".to_owned(),
            notices: Vec::new(),
            name: backup::file_name(Timestamp::now()),
            bytes,
        })
    }

    /// Takes a backup if the newest is older than the device's `backup-every`, and says
    /// where it went — or nothing, if none was due.
    ///
    /// Every client calls this opportunistically — at launch, before a one-shot command,
    /// hourly in anything resident — because a schedule only exists where something stays
    /// running.
    ///
    /// # Errors
    ///
    /// If a backup was due and could not be written.
    pub fn back_up_if_due(&self) -> Result<Option<String>> {
        let policy = self.backup_policy()?;
        let written = self.store().back_up_if_due(&policy, Timestamp::now())?;
        Ok(written.map(|written| written.path.display().to_string()))
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
        self.told(|store| {
            self.merged();
            restore_bytes(store, file, &bytes)
        })
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
                ExportFormat::Json => export::export_json_keeping(&snapshot, now.timestamp(), store.documents()),
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
        self.import_bytes(path, read(Path::new(path))?)
    }

    /// [`import`](Self::import), from a file's contents rather than its path: what a client
    /// has that was handed a file rather than a name for one — the browser's upload. `name`
    /// is what to call the file in what is said.
    ///
    /// # Errors
    ///
    /// If the contents are neither a JSON export nor a backup.
    pub fn import_bytes(&self, name: &str, bytes: Vec<u8>) -> Result<Imported> {
        let file = Path::new(name);
        if backup::is_backup(&bytes) {
            return self
                .with(|store| {
                    self.merged();
                    restore_bytes(store, file, &bytes)
                })
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
            self.merged();
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
            let unplaced = report.skipped - report.erased;
            if unplaced > 0 {
                done = done.note(format!(
                    "left out {} whose block is in neither the file nor this store",
                    count_line(unplaced, "assignment")
                ));
            }
            if report.erased > 0 {
                done = done.note(format!(
                    "left out {} erased for good here, so that importing does not bring them back",
                    count_line(report.erased, "record")
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

/// A backup inside the directory it protects can be lost along with it.
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
/// Never default to a cloud-synced directory, and if the user chooses one, say so plainly
/// rather than silently complying. Recognised by the folder names the common services use;
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

/// For a backup folder the person gave a browser, which this process cannot list itself:
/// whether a backup is due among the files `names` there, by the default policy (a browser has
/// no device settings of its own).
#[must_use]
pub fn folder_backup_due(names: &[String], now: jiff::Timestamp) -> bool {
    lumenna_store::backup::due_among(names, Some(lumenna_store::backup::Policy::DEFAULT_EVERY), now)
}

/// For such a folder: which of the files `names` to remove, the oldest backups beyond the
/// default number kept. Nothing else there is ever named.
#[must_use]
pub fn folder_backup_surplus(names: &[String]) -> Vec<String> {
    lumenna_store::backup::surplus_among(names, lumenna_store::backup::Policy::DEFAULT_KEEP)
}

/// For such a folder, after a real erasure: every backup among the files `names`, each holding
/// what was erased, to delete before one is taken of the new history, as
/// `follow_erasure` deletes a device's own. Nothing else there is ever named.
#[must_use]
pub fn folder_backups(names: &[String]) -> Vec<String> {
    names.iter().filter(|name| backup::taken_from_name(name).is_some()).cloned().collect()
}

/// What a backup folder a browser was given does, in a sentence: when backups are taken there
/// and how many are kept, by the default policy.
#[must_use]
pub fn folder_backup_words() -> String {
    let hours = backup::Policy::DEFAULT_EVERY.as_hours();
    let every = if hours == 24 { "once a day".to_owned() } else { format!("every {}", count_line(usize::try_from(hours).unwrap_or(0), "hour")) };
    format!(
        "While Lumenna is open, it backs up {every} into the folder you choose, and keeps the newest {}.",
        backup::Policy::DEFAULT_KEEP
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_cleared_after_an_erasure_names_every_backup_there_and_nothing_else() {
        let names: Vec<String> =
            ["lumenna-20260110T090000.000Z.lumbak", "notes.txt", "lumenna-20261024T120000.123Z.lumbak", "lumenna-old.lumbak"]
                .map(str::to_owned)
                .into();
        assert_eq!(folder_backups(&names), ["lumenna-20260110T090000.000Z.lumbak", "lumenna-20261024T120000.123Z.lumbak"]);
    }

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
