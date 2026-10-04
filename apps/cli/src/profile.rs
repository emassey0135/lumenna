//! Finding the store, and remembering the last listing.

use std::path::{Path, PathBuf};

use lumenna_store::Store;

use crate::error::{CliError, Result};

/// The command that lists a given kind, for pointing at in an error.
fn listing_command(kind: &str) -> &'static str {
    match kind {
        "task" => "lum task list",
        "assignment" => "lum plan",
        "block" => "lum block list",
        "project" => "lum project list",
        "label" => "lum label list",
        _ => "lum task list",
    }
}

/// Where the store lives and what the last listing showed.
pub struct Profile {
    /// The open store.
    pub store: Store,
    directory: PathBuf,
    rows_addressable: bool,
}

impl Profile {
    /// Opens the profile, creating it if this is the first run.
    ///
    /// Order of precedence: an explicit `--profile`, then `LUMENNA_PROFILE`, then the
    /// platform's data directory. The environment variable is what makes a scripted test or
    /// a second profile possible without a flag on every invocation.
    pub fn open(explicit: Option<&Path>) -> Result<Self> {
        let directory = match explicit {
            Some(path) => path.to_path_buf(),
            None => match std::env::var_os("LUMENNA_PROFILE") {
                Some(value) => PathBuf::from(value),
                None => directories::ProjectDirs::from("", "", "lumenna")
                    .ok_or_else(|| {
                        CliError::Message(
                            "cannot find a data directory; set LUMENNA_PROFILE".to_owned(),
                        )
                    })?
                    // The *local* data directory. On Linux and macOS it is the same place as
                    // `data_dir`; on Windows `data_dir` is Roaming AppData, which a domain
                    // with roaming profiles copies to a server at every logon and logoff —
                    // carrying the store, and its backups beside it, off the machine.
                    .data_local_dir()
                    .to_path_buf(),
            },
        };
        std::fs::create_dir_all(&directory)?;
        let store = Store::open(&directory.join("lumenna.sqlite"))?;
        Ok(Self { store, directory, rows_addressable: true })
    }

    /// Stops row numbers meaning anything, for a client that is not the terminal.
    ///
    /// The last listing is one file per profile, so a resident `lum rpc` and a person typing
    /// `lum task list` in a shell would overwrite each other's numbering — and `1` would
    /// then silently name whichever listing landed last. Row numbers are a terminal
    /// affordance (§15); every other client holds identifiers, which never go stale.
    pub const fn detach_rows(&mut self) {
        self.rows_addressable = false;
    }

    /// The store's file.
    #[must_use]
    pub fn store_path(&self) -> PathBuf {
        self.directory.join("lumenna.sqlite")
    }

    /// The socket `lum sync-daemon` serves the command surface on (§8), which the BTSpeak app
    /// tries before spawning a server of its own.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        self.directory.join("lumenna.sock")
    }

    /// The profile directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Where this device's own settings live: a file in the profile directory, never in a
    /// document, because they are about this machine and must not sync (§3.12). Where this
    /// laptop keeps its backups means nothing on a phone.
    fn device_settings_path(&self) -> PathBuf {
        self.directory.join("device-settings")
    }

    /// A setting that belongs to this device alone.
    #[must_use]
    pub fn device_setting(&self, key: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.device_settings_path()).ok()?;
        text.lines()
            .filter_map(|line| line.split_once('\t'))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_owned())
    }

    /// Changes a device setting; `None` puts it back to its default.
    ///
    /// # Errors
    ///
    /// If the file cannot be written.
    pub fn set_device_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
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
    /// §9 wants backups outside the live database's directory, so that a bug that corrupts
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
        self.directory
            .parent()
            .map_or_else(|| self.directory.join("..").join(format!("{name}-backups")), |parent| {
                parent.join(format!("{name}-backups"))
            })
    }

    /// The backup policy in force on this device.
    ///
    /// # Errors
    ///
    /// If a stored device setting cannot be read — which only a hand-edited file produces.
    pub fn backup_policy(&self) -> Result<lumenna_store::backup::Policy> {
        use lumenna_store::backup::Policy;
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

    fn listing_path(&self) -> PathBuf {
        self.directory.join("last-listing")
    }

    /// Records what a listing showed, so `lum task done 3` can mean the third row.
    ///
    /// §15: a UUID is thirty-six characters, miserable to type and worse to dictate. Small
    /// integers against the last listing are what Taskwarrior does and what makes a terminal
    /// usable at all.
    ///
    /// The **kind** is stored alongside each identifier, and row numbers are counted within
    /// a kind. Otherwise `lum block list` followed by `lum assign 2 --block 1` would resolve
    /// `2` against blocks and be right, and `--block 1` against whatever came first and be
    /// silently wrong. Counting per kind makes both halves mean what they look like.
    ///
    /// Failing to write this is not worth failing a command over — the listing already
    /// printed, and the next one will overwrite it — so the error is dropped deliberately.
    pub fn remember(&self, rows: &[(&str, String)]) {
        if !self.rows_addressable {
            return;
        }
        let lines: Vec<String> =
            rows.iter().map(|(kind, id)| format!("{kind}\t{id}")).collect();
        let _ = std::fs::write(self.listing_path(), lines.join("\n"));
    }


    /// What the last listing showed, as `(kind, identifier)` pairs.
    pub fn recall(&self) -> Vec<(String, String)> {
        std::fs::read_to_string(self.listing_path())
            .map(|text| {
                text.lines()
                    .filter_map(|line| line.split_once('\t'))
                    .map(|(kind, id)| (kind.to_owned(), id.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Turns what the user typed into a full identifier.
    ///
    /// Three forms, all of them things people actually type:
    ///
    /// - a small integer, meaning that row of the last listing;
    /// - a UUID prefix, git-style, resolved against `candidates`;
    /// - a whole UUID.
    ///
    /// An ambiguous prefix is an error rather than a guess. Picking one would be a silent
    /// wrong answer, and the whole point of short identifiers is that they are typed
    /// quickly and therefore checked less.
    pub fn resolve(&self, input: &str, kind: &str, candidates: &[String]) -> Result<String> {
        if let Ok(row) = input.parse::<usize>() {
            if !self.rows_addressable {
                return Err(CliError::Message(format!(
                    "'{row}' is a row number, and row numbers address the terminal's last \
                     listing; pass an identifier"
                )));
            }
            let listing = self.recall();
            let of_kind: Vec<&String> = listing
                .iter()
                .filter(|(row_kind, _)| row_kind == kind)
                .map(|(_, id)| id)
                .collect();

            if of_kind.is_empty() {
                let showed = listing.first().map(|(row_kind, _)| row_kind.clone());
                let command = listing_command(kind);
                return Err(CliError::Message(match showed {
                    Some(showed) => format!(
                        "the last listing showed {showed}s, not {kind}s; run `{command}` \
                         first, or give the identifier"
                    ),
                    None => format!("nothing has been listed yet; run `{command}` first"),
                }));
            }
            return of_kind
                .get(row.wrapping_sub(1))
                .filter(|_| row >= 1)
                .map(|id| (*id).clone())
                .ok_or_else(|| {
                    CliError::Message(format!(
                        "there is no row {row}; the last listing showed {}",
                        crate::render::count_line(of_kind.len(), kind)
                    ))
                });
        }

        let lowered = input.to_lowercase();
        let matches: Vec<&String> =
            candidates.iter().filter(|id| id.starts_with(&lowered)).collect();
        match matches.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(CliError::Message(format!("nothing here matches '{input}'"))),
            many => Err(CliError::Message(format!(
                "'{input}' matches {} items; type more of it",
                many.len()
            ))),
        }
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
        .ok_or_else(|| CliError::Message(format!("'{text}' is not a number of backups to keep")))
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
            CliError::Message(format!(
                "'{text}' is not an interval; say something like 24h, 7d, or off"
            ))
        })
}
