//! The SQLite file: §8's storage layout, and the coordination that comes with it.
//!
//! # A document *is* its changes
//!
//! There is no "document" record. Loading the change chunks for a document reconstructs it,
//! and snapshots exist only to bound that: replay is linear in the number of changes, so
//! past a few thousand a fresh [`Automerge::save`](automerge::AutoCommit::save) is written
//! and the changes it subsumes are deleted.
//!
//! **History is not pruned, and does not need to be.** Automerge changes reference their
//! dependencies by hash, so discarding old operations breaks the DAG for any replica that
//! has not merged past them — and safe pruning would need every device to agree on a
//! fully-merged common prefix and drop it simultaneously, which is impossible when a device
//! may be offline for months. Compaction changes the *representation*; it discards no
//! operations. Nor is the volume a problem: a heavy user generating a hundred operations a
//! day produces roughly 2 MB a year compressed. The cost that matters is load time, and
//! snapshots plus year-sharding already bound that.
//!
//! # Why SQLite at all
//!
//! Chunks are stored as **opaque blobs**. SQLite replaces the *file*, not the
//! representation, so there is no structural conversion and nothing here understands
//! Automerge's format. What it buys is transactional appends and multi-process locking
//! instead of hand-rolled file locking — which is the whole of §8's coordination story.

use automerge::ChangeHash;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::doc::DocId;
use crate::error::{Result, StoreError};

/// Write a fresh snapshot once a document has accumulated this many loose changes.
///
/// Replay is linear, so the threshold trades a periodic write against startup time. A few
/// thousand is where the replay starts being noticeable on the slowest target that matters.
const COMPACT_AFTER_CHANGES: i64 = 2_000;

/// The SQLite file backing one profile.
pub struct Db {
    conn: Connection,
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error.to_string())
    }
}

impl Db {
    /// Opens or creates the store at `path`.
    ///
    /// # Errors
    ///
    /// If the file cannot be opened or the schema cannot be created.
    pub fn open(path: &std::path::Path) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// An in-memory store, for tests.
    ///
    /// # Errors
    ///
    /// If the schema cannot be created.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self> {
        // WAL is what lets a reader and a writer coexist, which §8 needs because several
        // processes on one machine share this file: the tray app, a CLI invocation, an
        // Emacs subprocess.
        let mode: String =
            conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0)).unwrap_or_default();
        if !mode.eq_ignore_ascii_case("wal") && !mode.eq_ignore_ascii_case("memory") {
            return Err(StoreError::Sqlite(format!("could not enable WAL mode (got {mode})")));
        }
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS changes (
                 rowid  INTEGER PRIMARY KEY AUTOINCREMENT,
                 doc_id TEXT NOT NULL,
                 hash   BLOB NOT NULL,
                 data   BLOB NOT NULL,
                 UNIQUE (doc_id, hash)
             );
             CREATE INDEX IF NOT EXISTS changes_by_doc ON changes (doc_id, rowid);

             CREATE TABLE IF NOT EXISTS snapshots (
                 doc_id TEXT PRIMARY KEY,
                 heads  BLOB NOT NULL,
                 data   BLOB NOT NULL
             );",
        )?;
        migrate_to_autoincrement(&mut conn)?;
        Ok(Self { conn })
    }

    /// Runs `read` inside one read transaction, so everything it reads comes from the same
    /// moment.
    ///
    /// Two separate reads can straddle another process's compaction: the snapshot read
    /// before it, the changes after it, and the changes that compaction folded into the new
    /// snapshot seen by neither. WAL gives a read transaction a fixed view of the file, which
    /// closes that gap.
    ///
    /// # Errors
    ///
    /// Whatever `read` returns, or a failure to open the transaction.
    pub fn consistently<R>(&self, read: impl FnOnce(&Self) -> Result<R>) -> Result<R> {
        let tx = self.conn.unchecked_transaction()?;
        let result = read(self)?;
        tx.commit()?;
        Ok(result)
    }

    /// The heads a document's stored snapshot was taken at, if it has one.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn snapshot_heads(&self, doc: DocId) -> Result<Option<Vec<ChangeHash>>> {
        let heads: Option<Vec<u8>> = self
            .conn
            .query_row("SELECT heads FROM snapshots WHERE doc_id = ?1", params![doc.name()], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(heads.map(|bytes| {
            bytes.as_chunks::<32>().0.iter().map(|chunk| ChangeHash(*chunk)).collect()
        }))
    }

    /// A value that changes whenever another connection has modified the database.
    ///
    /// SQLite has no cross-process notification — `sqlite3_update_hook` and the WAL hook
    /// fire only for your own connection — so this is the cheap check that stands in for
    /// one (§8). Watching the `-wal` file tells a process *when* to look; this tells it
    /// whether anything actually happened, and it is nearly free to read, so polling it on
    /// a one-second timer is a perfectly good fallback where file watching is unreliable.
    ///
    /// # Errors
    ///
    /// If the pragma cannot be read.
    pub fn data_version(&self) -> Result<i64> {
        Ok(self.conn.query_row("PRAGMA data_version", [], |row| row.get(0))?)
    }

    /// Appends changes for one document, ignoring any already stored.
    ///
    /// Returns the highest `rowid` written, which is a process's watch cursor: everything
    /// past it is news from somewhere else.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn append_changes(
        &mut self,
        doc: DocId,
        changes: &[(ChangeHash, Vec<u8>)],
    ) -> Result<Option<i64>> {
        if changes.is_empty() {
            return Ok(None);
        }
        let tx = self.conn.transaction()?;
        {
            // A change is immutable and named by its hash, so re-appending one is a no-op.
            // That is what makes replaying a sync exchange, or a crash mid-write, harmless.
            let mut insert = tx.prepare(
                "INSERT OR IGNORE INTO changes (doc_id, hash, data) VALUES (?1, ?2, ?3)",
            )?;
            for (hash, data) in changes {
                insert.execute(params![doc.name(), &hash.0[..], data])?;
            }
        }
        let last = tx.query_row("SELECT max(rowid) FROM changes", [], |row| row.get(0))?;
        tx.commit()?;
        Ok(last)
    }

    /// The stored snapshot for a document, if there is one.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn snapshot(&self, doc: DocId) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row("SELECT data FROM snapshots WHERE doc_id = ?1", params![doc.name()], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Every change stored for a document, oldest first.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn changes(&self, doc: DocId) -> Result<Vec<Vec<u8>>> {
        let mut stmt =
            self.conn.prepare("SELECT data FROM changes WHERE doc_id = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map(params![doc.name()], |row| row.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Changes for a document written after `cursor`, oldest first, with the new cursor.
    ///
    /// This is the read half of §8's cross-process notification: another process appended,
    /// `data_version` moved, and these are the changes to `load_incremental`. It is the
    /// same path whether the change originated locally, in another local process, or from a
    /// peer over Iroh — which is a good sign the design is right.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn changes_after(&self, doc: DocId, cursor: i64) -> Result<(Vec<Vec<u8>>, i64)> {
        let mut stmt = self.conn.prepare(
            "SELECT rowid, data FROM changes WHERE doc_id = ?1 AND rowid > ?2 ORDER BY rowid",
        )?;
        let rows = stmt.query_map(params![doc.name(), cursor], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut data = Vec::new();
        let mut last = cursor;
        for row in rows {
            let (rowid, bytes) = row?;
            last = rowid;
            data.push(bytes);
        }
        Ok((data, last))
    }

    /// The highest `rowid` in the table, or zero when it is empty.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn cursor(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT coalesce(max(rowid), 0) FROM changes", [], |row| row.get(0))?)
    }

    /// How many loose changes a document has accumulated.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn change_count(&self, doc: DocId) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT count(*) FROM changes WHERE doc_id = ?1",
            params![doc.name()],
            |row| row.get(0),
        )?)
    }

    /// Whether a document has enough loose changes to be worth compacting.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn needs_compaction(&self, doc: DocId) -> Result<bool> {
        Ok(self.change_count(doc)? >= COMPACT_AFTER_CHANGES)
    }

    /// Replaces a document's stored form with a fresh snapshot.
    ///
    /// `fold` is handed the stored snapshot and every stored change, applies them to the
    /// caller's document, and returns that document's heads and saved bytes. All of it
    /// happens inside one **immediate** transaction, which holds the write lock throughout:
    ///
    /// - No other process can append a change between `fold` reading the table and the
    ///   delete that follows, so nothing is deleted that the new snapshot does not contain.
    /// - Changes another process wrote and this one never loaded — including those inside a
    ///   snapshot another process compacted earlier — are applied before saving, so this
    ///   process's view going into the snapshot is never narrower than what is on disk.
    /// - Deleting the changes and writing the snapshot commit together, so there is no
    ///   instant at which a reader could see a document with neither.
    ///
    /// # Errors
    ///
    /// Whatever `fold` returns, or a failed read or write.
    pub fn compact(
        &mut self,
        doc: DocId,
        fold: impl FnOnce(Option<Vec<u8>>, Vec<Vec<u8>>) -> Result<(Vec<ChangeHash>, Vec<u8>)>,
    ) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let snapshot: Option<Vec<u8>> = tx
            .query_row("SELECT data FROM snapshots WHERE doc_id = ?1", params![doc.name()], |r| {
                r.get(0)
            })
            .optional()?;
        let changes: Vec<Vec<u8>> = {
            let mut stmt =
                tx.prepare("SELECT data FROM changes WHERE doc_id = ?1 ORDER BY rowid")?;
            let rows = stmt.query_map(params![doc.name()], |row| row.get(0))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let (heads, data) = fold(snapshot, changes)?;
        tx.execute(
            "INSERT INTO snapshots (doc_id, heads, data) VALUES (?1, ?2, ?3)
             ON CONFLICT (doc_id) DO UPDATE SET heads = excluded.heads, data = excluded.data",
            params![doc.name(), encode_heads(&heads), data],
        )?;
        tx.execute("DELETE FROM changes WHERE doc_id = ?1", params![doc.name()])?;
        tx.commit()?;
        Ok(())
    }

    /// Which documents this file holds anything for.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn stored_documents(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT doc_id FROM changes UNION SELECT doc_id FROM snapshots ORDER BY 1",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

/// Heads, concatenated. Stored for staleness checks, never parsed back by this crate.
fn encode_heads(heads: &[ChangeHash]) -> Vec<u8> {
    heads.iter().flat_map(|h| h.0.to_vec()).collect()
}

/// Rebuilds a `changes` table from before `AUTOINCREMENT`, keeping every row and its rowid.
///
/// Without it SQLite hands out `max(rowid) + 1`, so once compaction deletes a document's
/// rows — which are usually the newest — the next change reuses a rowid some running process
/// has already read past. That process's cursor then skips it, and it never sees the edit.
/// `AUTOINCREMENT` never reuses a rowid, which is the only property a cursor needs.
///
/// Checked again inside the write lock, since two processes may open an old file at once.
fn migrate_to_autoincrement(conn: &mut Connection) -> Result<()> {
    fn current(conn: &Connection) -> Result<bool> {
        let sql: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'changes'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(sql.is_some_and(|sql| sql.to_uppercase().contains("AUTOINCREMENT")))
    }
    if current(conn)? {
        return Ok(());
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if !current(&tx)? {
        tx.execute_batch(
            "CREATE TABLE changes_v2 (
                 rowid  INTEGER PRIMARY KEY AUTOINCREMENT,
                 doc_id TEXT NOT NULL,
                 hash   BLOB NOT NULL,
                 data   BLOB NOT NULL,
                 UNIQUE (doc_id, hash)
             );
             INSERT INTO changes_v2 (rowid, doc_id, hash, data)
                 SELECT rowid, doc_id, hash, data FROM changes;
             DROP TABLE changes;
             ALTER TABLE changes_v2 RENAME TO changes;
             CREATE INDEX IF NOT EXISTS changes_by_doc ON changes (doc_id, rowid);",
        )?;
    }
    tx.commit()?;
    Ok(())
}
