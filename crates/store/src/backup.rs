//! Backups: the whole store, history and all, in one file that can rebuild it.
//!
//! **Sync is not backup.** Peer-to-peer sync is replication, and replicated corruption or
//! deletion is still corruption or deletion. Every client runs these — opportunistically, when
//! the last one is older than the interval — because the daemon is optional on most platforms
//! and anything only it did would effectively not exist.
//!
//! # The format
//!
//! Each document's Automerge [`save`](Doc::save) output, framed:
//!
//! ```text
//! "LUMENNA-BACKUP\n"                 magic
//! u16                                format version, little-endian
//! u32                                document count
//! per document:
//!   u16 + bytes                      its name: "core", "devices", "blocks-2026"
//!   u64 + bytes                      its saved bytes
//! ```
//!
//! A backup is `save()` output rather than a copy of the SQLite file because it is self-describing,
//! checksummed by Automerge itself, and carries no WAL state or locks, so a backup taken while
//! another process writes is still whole. The frame around it is deliberately trivial —
//! anything that can read Automerge can read a backup.
//!
//! # What a backup holds
//!
//! **Everything, including every task ever deleted.** An Automerge document keeps its change
//! log, so this is the one file in the system that contains what the trash was emptied of.
//! That is why it is written only where the user's own files are, never by default anywhere a
//! cloud service would carry it off, and why it is kept apart from current-state export,
//! which holds no history at all.

use std::path::{Path, PathBuf};

use jiff::Timestamp;

use crate::doc::DocId;
use crate::error::{Result, StoreError};

const MAGIC: &[u8] = b"LUMENNA-BACKUP\n";
const VERSION: u16 = 1;
const PREFIX: &str = "lumenna-";
const SUFFIX: &str = ".lumbak";

/// Whether some bytes are a backup, as opposed to an export or anything else.
#[must_use]
pub fn is_backup(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// Frames saved documents as a backup file.
#[must_use]
pub fn encode(documents: &[(DocId, Vec<u8>)]) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    let count = u32::try_from(documents.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&count.to_le_bytes());
    for (id, data) in documents {
        let name = id.name();
        let length = u16::try_from(name.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        out.extend_from_slice(data);
    }
    out
}

/// What [`decode`] found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decoded {
    /// The documents, with their saved bytes.
    pub documents: Vec<(DocId, Vec<u8>)>,
    /// Names of documents this version does not know, which were left out rather than
    /// failing the whole restore. A newer version may add a kind of document; the rest of the
    /// backup is still yours.
    pub unknown: Vec<String>,
}

/// Reads a backup file's frame.
///
/// # Errors
///
/// [`StoreError::Unreadable`] if it is not a backup, was written by a newer format, or is cut
/// short. Each document's own bytes are checked when Automerge loads them.
pub fn decode(bytes: &[u8]) -> Result<Decoded> {
    let mut rest = bytes
        .strip_prefix(MAGIC)
        .ok_or_else(|| StoreError::Unreadable("that is not a Lumenna backup".to_owned()))?;
    let version = u16::from_le_bytes(take(&mut rest)?);
    if version > VERSION {
        return Err(StoreError::Unreadable(format!(
            "that backup was made by a newer version of Lumenna (format {version}); update to \
             restore it"
        )));
    }
    let count = u32::from_le_bytes(take(&mut rest)?);
    let mut decoded = Decoded::default();
    for _ in 0..count {
        let name_length = usize::from(u16::from_le_bytes(take(&mut rest)?));
        let name = String::from_utf8_lossy(take_slice(&mut rest, name_length)?).into_owned();
        let data_length = usize::try_from(u64::from_le_bytes(take(&mut rest)?))
            .map_err(|_| truncated())?;
        let data = take_slice(&mut rest, data_length)?.to_vec();
        match DocId::parse(&name) {
            Some(id) => decoded.documents.push((id, data)),
            None => decoded.unknown.push(name),
        }
    }
    Ok(decoded)
}

fn truncated() -> StoreError {
    StoreError::Unreadable("that backup is cut short, or damaged".to_owned())
}

fn take<const N: usize>(rest: &mut &[u8]) -> Result<[u8; N]> {
    let bytes = take_slice(rest, N)?;
    bytes.try_into().map_err(|_| truncated())
}

fn take_slice<'a>(rest: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if rest.len() < n {
        return Err(truncated());
    }
    let (head, tail) = rest.split_at(n);
    *rest = tail;
    Ok(head)
}

/// Where backups go, how many are kept, and how often one is due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// The directory. It should be **outside** the profile directory, so that a bug that
    /// corrupts one cannot take both.
    pub directory: PathBuf,
    /// How many to keep. The oldest go first.
    pub keep: usize,
    /// How old the newest may get before another is taken. `None` turns automatic backups
    /// off; asking for one still works.
    pub every: Option<jiff::SignedDuration>,
}

impl Policy {
    /// How many backups are kept unless the user says otherwise.
    pub const DEFAULT_KEEP: usize = 10;

    /// How often a backup is due unless the user says otherwise.
    pub const DEFAULT_EVERY: jiff::SignedDuration = jiff::SignedDuration::from_hours(24);
}

/// A backup file, and when it was taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Where it is.
    pub path: PathBuf,
    /// When it was taken, as its name records.
    pub taken: Timestamp,
}

/// Every backup in a directory, oldest first.
///
/// Recognised by name — `lumenna-<UTC time>.lumbak` — so anything else a user keeps there is
/// never listed, and never pruned.
///
/// # Errors
///
/// If the directory exists but cannot be read.
pub fn list(directory: &Path) -> Result<Vec<Entry>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut found: Vec<Entry> = entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let stamp = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
            Some(Entry { path: entry.path(), taken: parse_stamp(stamp)? })
        })
        .collect();
    found.sort_by_key(|entry| entry.taken);
    Ok(found)
}

/// `lumenna-20261004T003922.926Z.lumbak`: UTC, to the millisecond, so that an automatic
/// backup and one asked for in the same second do not share a name and replace each other.
pub fn file_name(taken: Timestamp) -> String {
    let millis = taken.as_millisecond().rem_euclid(1000);
    format!("{PREFIX}{}.{millis:03}Z{SUFFIX}", taken.strftime("%Y%m%dT%H%M%S"))
}

fn parse_stamp(stamp: &str) -> Option<Timestamp> {
    let stamp = stamp.strip_suffix('Z')?;
    let (seconds, millis) = stamp.split_once('.').unwrap_or((stamp, "0"));
    let base = Timestamp::strptime("%Y%m%dT%H%M%S%z", format!("{seconds}+0000")).ok()?;
    let millis: i64 = millis.parse().ok()?;
    base.checked_add(jiff::SignedDuration::from_millis(millis)).ok()
}

/// Writes a backup into the policy's directory and prunes the oldest beyond `keep`.
///
/// Written to a temporary name and renamed into place, so an interrupted backup never leaves
/// a file that looks complete. On Unix it is readable by its owner only: it holds the whole
/// history, deleted tasks included.
///
/// # Errors
///
/// If the directory cannot be created or written.
pub fn write(policy: &Policy, bytes: &[u8], taken: Timestamp) -> Result<PathBuf> {
    std::fs::create_dir_all(&policy.directory)?;
    let path = policy.directory.join(file_name(taken));
    let partial = path.with_extension("partial");
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut file = options.open(&partial)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&partial, &path)?;

    let existing = list(&policy.directory)?;
    let surplus = existing.len().saturating_sub(policy.keep.max(1));
    for old in existing.into_iter().take(surplus) {
        std::fs::remove_file(old.path)?;
    }
    Ok(path)
}

/// Whether the newest backup is older than the policy allows.
///
/// # Errors
///
/// If the directory cannot be read.
pub fn is_due(policy: &Policy, now: Timestamp) -> Result<bool> {
    let Some(every) = policy.every else {
        return Ok(false);
    };
    Ok(match list(&policy.directory)?.last() {
        None => true,
        Some(newest) => now.duration_since(newest.taken) >= every,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_round_trips() {
        let documents = vec![
            (DocId::Core, vec![1, 2, 3]),
            (DocId::Devices, vec![]),
            (DocId::Blocks(2026), vec![9; 300]),
        ];
        let bytes = encode(&documents);
        assert!(is_backup(&bytes));
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.documents, documents);
        assert!(decoded.unknown.is_empty());
    }

    #[test]
    fn anything_else_is_refused_with_a_reason() {
        assert!(decode(b"{\"format\": \"lumenna-export\"}").is_err());
        let bytes = encode(&[(DocId::Core, vec![1, 2, 3])]);
        let cut = decode(&bytes[..bytes.len() - 1]).unwrap_err();
        assert!(cut.to_string().contains("cut short"), "{cut}");

        let mut newer = bytes.clone();
        newer[MAGIC.len()] = 2;
        assert!(decode(&newer).unwrap_err().to_string().contains("newer version"));
    }

    #[test]
    fn a_document_this_version_does_not_know_is_set_aside() {
        let mut bytes = encode(&[(DocId::Core, vec![1])]);
        // Patch the count to two and append a document from the future.
        bytes[MAGIC.len() + 2] = 2;
        let name = b"calendars";
        bytes.extend_from_slice(&(name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(name);
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.push(7);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded.documents.len(), 1);
        assert_eq!(decoded.unknown, vec!["calendars".to_owned()]);
    }

    #[test]
    fn names_carry_the_time_and_sort_by_it() {
        let taken = Timestamp::from_millisecond(1_790_000_000_123).unwrap();
        let name = file_name(taken);
        let stamp = name.strip_prefix(PREFIX).unwrap().strip_suffix(SUFFIX).unwrap();
        assert_eq!(parse_stamp(stamp), Some(taken));
        let next = Timestamp::from_millisecond(1_790_000_000_124).unwrap();
        assert!(file_name(taken) < file_name(next), "a millisecond apart, two names");
    }

    #[test]
    fn writing_prunes_the_oldest_and_leaves_other_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "mine").unwrap();
        let policy =
            Policy { directory: dir.path().to_path_buf(), keep: 3, every: Some(Policy::DEFAULT_EVERY) };
        let start = Timestamp::from_second(1_790_000_000).unwrap();
        for hour in 0..5 {
            let taken = start + jiff::SignedDuration::from_hours(hour);
            write(&policy, b"backup", taken).unwrap();
        }
        let kept = list(dir.path()).unwrap();
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[0].taken, start + jiff::SignedDuration::from_hours(2));
        assert!(dir.path().join("notes.txt").exists());
    }

    #[test]
    fn a_backup_is_due_only_once_the_interval_has_passed() {
        let dir = tempfile::tempdir().unwrap();
        let policy =
            Policy { directory: dir.path().join("b"), keep: 3, every: Some(Policy::DEFAULT_EVERY) };
        let start = Timestamp::from_second(1_790_000_000).unwrap();
        assert!(is_due(&policy, start).unwrap(), "no backup yet");
        write(&policy, b"x", start).unwrap();
        assert!(!is_due(&policy, start + jiff::SignedDuration::from_hours(23)).unwrap());
        assert!(is_due(&policy, start + jiff::SignedDuration::from_hours(24)).unwrap());
        let off = Policy { every: None, ..policy };
        assert!(!is_due(&off, start + jiff::SignedDuration::from_hours(48)).unwrap());
    }
}
