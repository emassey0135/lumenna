//! Finding the store, and remembering the last listing.
//!
//! The store and every operation on it are [`Lumenna`]'s; a profile adds what only a terminal
//! has — a default location, and row numbers that address the last listing (§15).

use std::ops::Deref;
use std::path::{Path, PathBuf};

use lumenna_surface::Lumenna;
use lumenna_surface::words::count_line;

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

/// The open store, and what the last listing showed.
pub struct Profile {
    lumenna: Lumenna,
    rows_addressable: bool,
}

impl Deref for Profile {
    type Target = Lumenna;

    fn deref(&self) -> &Lumenna {
        &self.lumenna
    }
}

/// The profile directory when nothing names another one.
#[must_use]
pub fn default_directory() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "lumenna").map(|dirs| {
        // The *local* data directory. On Linux and macOS it is the same place as `data_dir`;
        // on Windows `data_dir` is Roaming AppData, which a domain with roaming profiles
        // copies to a server at every logon and logoff — carrying the store, and its backups
        // beside it, off the machine.
        dirs.data_local_dir().to_path_buf()
    })
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
                None => default_directory().ok_or_else(|| {
                    CliError::Message("cannot find a data directory; set LUMENNA_PROFILE".to_owned())
                })?,
            },
        };
        Ok(Self { lumenna: Lumenna::open_at(&directory)?, rows_addressable: true })
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

    /// The socket `lum sync-daemon` serves the command surface on (§8), which the BTSpeak app
    /// tries before spawning a server of its own.
    #[must_use]
    pub fn socket_path(&self) -> PathBuf {
        self.path().join("lumenna.sock")
    }

    /// The profile directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        self.path()
    }

    fn listing_path(&self) -> PathBuf {
        self.path().join("last-listing")
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
    pub fn remember(&self, rows: &[(String, String)]) {
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

    /// Turns a row number into the identifier it showed, and passes anything else through
    /// for the surface to resolve as an identifier or a prefix of one.
    ///
    /// §15: a UUID is thirty-six characters, miserable to type and worse to dictate. Small
    /// integers against the last listing are what Taskwarrior does and what makes a terminal
    /// usable at all. They are counted within `kind`, so `--block 1` means the first block
    /// even when the listing also showed tasks.
    ///
    /// # Errors
    ///
    /// If the number is not a row of that kind in the last listing.
    pub fn row(&self, input: &str, kind: &str) -> Result<String> {
        let Ok(row) = input.trim().parse::<usize>() else {
            return Ok(input.to_owned());
        };
        if !self.rows_addressable {
            // The surface refuses bare numbers with the same explanation.
            return Ok(input.to_owned());
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
                    "the last listing showed {showed}s, not {kind}s; run `{command}` first, \
                     or give the identifier"
                ),
                None if self.listing_path().exists() => format!(
                    "the last listing showed nothing, so there is no row {row}; run `{command}` \
                     to list {kind}s"
                ),
                None => format!("nothing has been listed yet; run `{command}` first"),
            }));
        }
        of_kind
            .get(row.wrapping_sub(1))
            .filter(|_| row >= 1)
            .map(|id| (*id).clone())
            .ok_or_else(|| {
                CliError::Message(format!(
                    "there is no row {row}; the last listing showed {}",
                    count_line(of_kind.len(), kind)
                ))
            })
    }
}

