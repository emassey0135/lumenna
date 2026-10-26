//! The SQLite file: the storage layout, and the coordination that comes with it.
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
//! snapshots plus year-sharding already bound that. The one way history goes is a real
//! erasure, which begins every document again as a new generation that every device takes in
//! (`crate::generation`, [`Db::replace_generation`]).
//!
//! # Why SQLite at all
//!
//! Chunks are stored as **opaque blobs**. SQLite replaces the *file*, not the
//! representation, so there is no structural conversion and nothing here understands
//! Automerge's format. What it buys is transactional appends and multi-process locking
//! instead of hand-rolled file locking — which is the whole of the coordination story.

use automerge::ChangeHash;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::doc::DocId;
use crate::error::{Result, StoreError};

/// Write a fresh snapshot once a document has accumulated this many loose changes.
///
/// Replay is linear, so the threshold trades a periodic write against startup time. A few
/// thousand is where the replay starts being noticeable on the slowest target that matters.
const COMPACT_AFTER_CHANGES: i64 = 2_000;

/// Where the file says which generation of the store's history it holds (`crate::generation`):
/// beside the documents, written in the transaction that replaces them, so every process
/// sharing the file sees the two change together.
const GENERATION: &str = "generation";

/// Where it keeps each document's heads as its generation began.
pub(crate) const GENERATION_BEGAN: &str = "generation-began";

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
        // WAL is what lets a reader and a writer coexist, which is needed because several
        // processes on one machine share this file: the tray app, a CLI invocation, an
        // Emacs subprocess.
        // In a browser one worker owns the file — OPFS allows one connection — so
        // there is nobody to coexist with, and its storage has no WAL to give.
        if !cfg!(all(target_family = "wasm", target_os = "unknown")) {
            // Turning WAL on needs the file to itself for a moment, and SQLite answers "busy"
            // at once rather than waiting: two processes opening a new store together — a
            // `lum` command beside the BTSpeak app's server — failed here. WAL, once set,
            // stays, so asking again soon finds it set by the other.
            let mut mode = String::new();
            for _ in 0..100 {
                mode = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0)).unwrap_or_default();
                if mode.eq_ignore_ascii_case("wal") || mode.eq_ignore_ascii_case("memory") {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            if !mode.eq_ignore_ascii_case("wal") && !mode.eq_ignore_ascii_case("memory") {
                return Err(StoreError::Sqlite(format!("could not enable WAL mode (got {mode})")));
            }
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
             );

             -- Things about this device that must never leave it: its secret key,
             -- above all. Sync and backups carry Automerge documents, never this table.
             CREATE TABLE IF NOT EXISTS local_state (
                 key   TEXT PRIMARY KEY,
                 value BLOB NOT NULL
             );

             -- How syncing with each peer last went, for `lum sync status`. Local:
             -- another device's view of the network is not this one's.
             CREATE TABLE IF NOT EXISTS peers (
                 node_id      TEXT PRIMARY KEY,
                 last_attempt INTEGER,
                 last_success INTEGER,
                 last_error   TEXT
             );

             -- This device's undo history. Local-only: sync and backups carry
             -- Automerge documents, never this table. `undone` marks the redo side.
             -- Each document's part of the snapshot as one build materialized it, at the heads
             -- it was read at, so a process opening the store reads only what changed since
             -- (`Store::snapshot`), and the postponements' tally (`postponed`) likewise. A rebuildable cache, never truth: used only by the build that
             -- wrote it, and only where the document holds those heads. Local, like the rest.
             CREATE TABLE IF NOT EXISTS hydrated (
                 doc_id TEXT PRIMARY KEY,
                 build  TEXT NOT NULL,
                 heads  BLOB NOT NULL,
                 part   BLOB NOT NULL
             );

             -- Each document's heads as last written, so a sync can say what a year holds
             -- without loading it (`Store::sync_heads`). Local and rebuildable: `through` (the
             -- highest change rowid of the document then) and `snapshot` (its snapshot's heads
             -- then) say whether anything has written the document since without saying so,
             -- such as a build from before this table, and a row that no longer matches is not
             -- believed.
             CREATE TABLE IF NOT EXISTS kept_heads (
                 doc_id   TEXT PRIMARY KEY,
                 heads    BLOB NOT NULL,
                 through  INTEGER NOT NULL,
                 snapshot BLOB NOT NULL
             );

             CREATE TABLE IF NOT EXISTS undo (
                 seq         INTEGER PRIMARY KEY AUTOINCREMENT,
                 undone      INTEGER NOT NULL DEFAULT 0,
                 format      INTEGER NOT NULL,
                 recorded_at INTEGER NOT NULL,
                 edit        TEXT NOT NULL
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
        Ok(heads.map(|bytes| decode_heads(&bytes)))
    }

    /// A value that changes whenever another connection has modified the database.
    ///
    /// SQLite has no cross-process notification — `sqlite3_update_hook` and the WAL hook
    /// fire only for your own connection — so this is the cheap check that stands in for
    /// one. Watching the `-wal` file tells a process *when* to look; this tells it
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
    /// past it is news from somewhere else. What it writes is not counted in the document's kept
    /// heads, which it forgets, since nothing here says what they are now.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn append_changes(
        &mut self,
        doc: DocId,
        changes: &[(ChangeHash, Vec<u8>)],
    ) -> Result<Option<i64>> {
        self.append(None, &[Appending { doc, changes: changes.to_vec(), heads: None }], |_, _| false)
    }

    /// [`append_changes`](Self::append_changes) for a process holding `generation`'s
    /// documents: refused, [`StoreError::Rebuilt`](crate::StoreError::Rebuilt), when another
    /// has since rebuilt the store as a newer one, in the same transaction, so a change of the
    /// older history never lands among the newer's.
    ///
    /// # Errors
    ///
    /// If the write fails, or the file holds another generation.
    pub fn append_changes_of(
        &mut self,
        generation: Option<&str>,
        doc: DocId,
        changes: &[(ChangeHash, Vec<u8>)],
    ) -> Result<Option<i64>> {
        self.append(Some(generation), &[Appending { doc, changes: changes.to_vec(), heads: None }], |_, _| false)
    }

    /// Appends every document's changes in one transaction, refused as a whole when the file
    /// holds another generation than `generation`: one edit can touch several documents (a
    /// series moved to another year), and a crash or a rebuild between them must not leave half
    /// of it on disk.
    ///
    /// Each document's kept heads become the heads it has once its changes are written, when
    /// the heads kept before are among what the writer holds (`holds`), which says nothing on
    /// disk is news to it; otherwise they are forgotten, and found again by loading it.
    ///
    /// # Errors
    ///
    /// If the write fails, or the file holds another generation.
    pub fn append_documents(
        &mut self,
        generation: Option<&str>,
        documents: &[Appending],
        holds: impl FnMut(DocId, &ChangeHash) -> bool,
    ) -> Result<Option<i64>> {
        self.append(Some(generation), documents, holds)
    }

    fn append(
        &mut self,
        generation: Option<Option<&str>>,
        documents: &[Appending],
        mut holds: impl FnMut(DocId, &ChangeHash) -> bool,
    ) -> Result<Option<i64>> {
        if documents.iter().all(|d| d.changes.is_empty()) {
            return Ok(None);
        }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(generation) = generation {
            check_generation(&tx, generation)?;
        }
        for appending in documents {
            let doc = appending.doc;
            // What was there before this write, so the heads kept can be believed after it.
            let kept = read_kept(&tx, doc)?;
            let fresh = kept.is_none() && max_rowid(&tx, doc)? == 0 && snapshot_heads_blob(&tx, doc)?.is_none();
            {
                // A change is immutable and named by its hash, so re-appending one is a no-op.
                // That is what makes replaying a sync exchange, or a crash mid-write, harmless.
                let mut insert = tx.prepare(
                    "INSERT OR IGNORE INTO changes (doc_id, hash, data) VALUES (?1, ?2, ?3)",
                )?;
                for (hash, data) in &appending.changes {
                    insert.execute(params![doc.name(), &hash.0[..], data])?;
                }
            }
            let known = match (&appending.heads, kept) {
                (Some(_), Some(kept)) => kept.iter().all(|head| holds(doc, head)),
                (Some(_), None) => fresh,
                (None, _) => false,
            };
            match &appending.heads {
                Some(heads) if known => write_kept(&tx, doc, heads)?,
                _ => {
                    tx.execute("DELETE FROM kept_heads WHERE doc_id = ?1", params![doc.name()])?;
                }
            }
        }
        let last = tx.query_row("SELECT max(rowid) FROM changes", [], |row| row.get(0))?;
        tx.commit()?;
        Ok(last)
    }

    /// The heads `doc` had as last written, when nothing has written it since without saying
    /// so (see the `kept_heads` table): what a sync says it holds without loading it.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn kept_heads(&self, doc: DocId) -> Result<Option<Vec<ChangeHash>>> {
        self.consistently(|db| read_kept(&db.conn, doc))
    }

    /// Keeps `heads` as `doc`'s, read by a process holding it whole: only when every change and
    /// snapshot on disk for it is among what that process holds (`holds`), checked with the
    /// write lock held, so nothing written meanwhile goes uncounted. Returns whether they were
    /// kept.
    ///
    /// # Errors
    ///
    /// If the read or write fails.
    pub fn note_heads(
        &mut self,
        doc: DocId,
        heads: &[ChangeHash],
        mut holds: impl FnMut(&ChangeHash) -> bool,
    ) -> Result<bool> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut on_disk: Vec<ChangeHash> = {
            let mut stmt = tx.prepare("SELECT hash FROM changes WHERE doc_id = ?1")?;
            let rows = stmt.query_map(params![doc.name()], |row| row.get::<_, Vec<u8>>(0))?;
            rows.filter_map(|row| row.ok().and_then(|bytes| Some(ChangeHash(bytes.try_into().ok()?)))).collect()
        };
        on_disk.extend(snapshot_heads_blob(&tx, doc)?.map(|blob| decode_heads(&blob)).unwrap_or_default());
        let held = on_disk.iter().all(&mut holds);
        if held {
            write_kept(&tx, doc, heads)?;
        }
        tx.commit()?;
        Ok(held)
    }

    /// The highest change number the file has ever handed out: it moves with every append by
    /// any process and never goes back, even when compaction deletes the newest rows.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn sequence(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name = 'changes'), 0)",
            [],
            |row| row.get(0),
        )?)
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
    /// This is the read half of the cross-process notification: another process appended,
    /// `data_version` moved, and these are the changes to `load_incremental`. It is the
    /// same path whether the change originated locally, in another local process, or from a
    /// peer over Iroh.
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

    /// Replaces a document's stored form with a fresh snapshot, refused
    /// ([`StoreError::Rebuilt`]) when the file no longer holds `generation`.
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
        generation: Option<&str>,
        doc: DocId,
        fold: impl FnOnce(Option<Vec<u8>>, Vec<Vec<u8>>) -> Result<(Vec<ChangeHash>, Vec<u8>)>,
    ) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // A process still holding the history before a real erasure must not save it over the
        // newer one, which it would bring back whole.
        check_generation(&tx, generation)?;
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
        // Everything on disk went into the snapshot, so its heads are the document's.
        write_kept(&tx, doc, &heads)?;
        tx.commit()?;
        Ok(())
    }

    /// The generation of the store's history this file holds (see `crate::generation`):
    /// `None` for the first.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn generation(&self) -> Result<Option<String>> {
        Ok(self.local_value(GENERATION)?.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }

    /// Replaces every document with `documents`, generation `generation`, in one transaction:
    /// a real erasure, here or taken in from another device. Everything that held the history
    /// before goes with it: the changes and snapshots, the parts read from them, the undo
    /// history (whose edits carry the records they removed), and what a watch link last agreed
    /// on. `began` is kept, for when a newer generation is taken in after this one.
    ///
    /// Only when the file is still as the caller read it: generation `from`, and no change
    /// appended since [`sequence`](Self::sequence) said `sequence`. Otherwise nothing is written
    /// and it returns false: another process wrote meanwhile, and deleting the changes would
    /// delete its edit, so the caller reads again and rebuilds.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn replace_generation(
        &mut self,
        from: Option<&str>,
        sequence: i64,
        generation: &str,
        documents: &[(DocId, Vec<ChangeHash>, Vec<u8>)],
        began: &str,
    ) -> Result<bool> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let held: Option<Vec<u8>> = tx
            .query_row("SELECT value FROM local_state WHERE key = ?1", params![GENERATION], |r| r.get(0))
            .optional()?;
        let now: i64 = tx.query_row(
            "SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name = 'changes'), 0)",
            [],
            |row| row.get(0),
        )?;
        if held.as_deref() != from.map(str::as_bytes) || now != sequence {
            return Ok(false);
        }
        tx.execute_batch(
            "DELETE FROM changes; DELETE FROM snapshots; DELETE FROM hydrated; DELETE FROM undo;
             DELETE FROM kept_heads;",
        )?;
        for (doc, heads, data) in documents {
            tx.execute(
                "INSERT INTO snapshots (doc_id, heads, data) VALUES (?1, ?2, ?3)",
                params![doc.name(), encode_heads(heads), data],
            )?;
            write_kept(&tx, *doc, heads)?;
        }
        for (key, value) in [(GENERATION, generation), (GENERATION_BEGAN, began)] {
            tx.execute(
                "INSERT INTO local_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, value.as_bytes()],
            )?;
        }
        tx.execute("DELETE FROM local_state WHERE key = 'link-agreed'", [])?;
        tx.commit()?;
        Ok(true)
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

/// Heads, concatenated.
fn encode_heads(heads: &[ChangeHash]) -> Vec<u8> {
    heads.iter().flat_map(|h| h.0.to_vec()).collect()
}

fn decode_heads(bytes: &[u8]) -> Vec<ChangeHash> {
    bytes.as_chunks::<32>().0.iter().map(|chunk| ChangeHash(*chunk)).collect()
}

/// One document's changes for [`Db::append_documents`], and its heads once they are written:
/// `None` when the writer cannot say.
#[derive(Debug, Clone)]
pub struct Appending {
    /// Which document.
    pub doc: DocId,
    /// Its changes to write, each with its hash.
    pub changes: Vec<(ChangeHash, Vec<u8>)>,
    /// Its heads with them.
    pub heads: Option<Vec<ChangeHash>>,
}

/// Refuses, [`StoreError::Rebuilt`], when the file holds another generation than `generation`.
fn check_generation(conn: &Connection, generation: Option<&str>) -> Result<()> {
    let held: Option<Vec<u8>> = conn
        .query_row("SELECT value FROM local_state WHERE key = ?1", params![GENERATION], |r| r.get(0))
        .optional()?;
    if held.as_deref() == generation.map(str::as_bytes) { Ok(()) } else { Err(StoreError::Rebuilt) }
}

/// The highest change rowid of `doc`, zero when it has none.
fn max_rowid(conn: &Connection, doc: DocId) -> Result<i64> {
    Ok(conn.query_row("SELECT coalesce(max(rowid), 0) FROM changes WHERE doc_id = ?1", params![doc.name()], |row| {
        row.get(0)
    })?)
}

/// `doc`'s snapshot's heads, as stored, if it has a snapshot.
fn snapshot_heads_blob(conn: &Connection, doc: DocId) -> Result<Option<Vec<u8>>> {
    Ok(conn
        .query_row("SELECT heads FROM snapshots WHERE doc_id = ?1", params![doc.name()], |r| r.get(0))
        .optional()?)
}

/// `doc`'s kept heads, when nothing has written it since they were kept: no change after the
/// last one counted, and the snapshot they were kept beside.
fn read_kept(conn: &Connection, doc: DocId) -> Result<Option<Vec<ChangeHash>>> {
    let row: Option<(Vec<u8>, i64, Vec<u8>)> = conn
        .query_row(
            "SELECT heads, through, snapshot FROM kept_heads WHERE doc_id = ?1",
            params![doc.name()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((heads, through, snapshot)) = row else { return Ok(None) };
    let current = snapshot_heads_blob(conn, doc)?.unwrap_or_default();
    Ok((max_rowid(conn, doc)? <= through && current == snapshot).then(|| decode_heads(&heads)))
}

/// Keeps `heads` as `doc`'s, counting everything on disk for it now.
fn write_kept(conn: &Connection, doc: DocId, heads: &[ChangeHash]) -> Result<()> {
    let through = max_rowid(conn, doc)?;
    let snapshot = snapshot_heads_blob(conn, doc)?.unwrap_or_default();
    conn.execute(
        "INSERT OR REPLACE INTO kept_heads (doc_id, heads, through, snapshot) VALUES (?1, ?2, ?3, ?4)",
        params![doc.name(), encode_heads(heads), through, snapshot],
    )?;
    Ok(())
}

/// What is kept in `hydrated`: the build that wrote it, the heads it was read at, and itself.
pub type Kept = (String, Vec<ChangeHash>, Vec<u8>);

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

impl Db {
    /// Saves an edit to the undo history, clearing anything that was waiting to be redone.
    ///
    /// Doing something new discards the redo side: redoing into a branch a later edit has
    /// diverged from would apply an edit computed for a state that no longer exists. The
    /// oldest entries beyond `depth` fall off.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn record_undo(&mut self, format: i64, edit: &str, at_ms: i64, depth: usize) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM undo WHERE undone = 1", [])?;
        tx.execute(
            "INSERT INTO undo (format, recorded_at, edit) VALUES (?1, ?2, ?3)",
            params![format, at_ms, edit],
        )?;
        tx.execute(
            "DELETE FROM undo WHERE seq NOT IN (SELECT seq FROM undo ORDER BY seq DESC LIMIT ?1)",
            params![i64::try_from(depth).unwrap_or(i64::MAX)],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Takes the next entry to undo (`redo` false) or redo (`redo` true), marking it moved to
    /// the other side in the same transaction — so two processes asking at once never both
    /// get the same one.
    ///
    /// Undo takes the newest entry not yet undone; redo the oldest of those undone, which is
    /// the one most recently undone, since undoing always works backwards from the end.
    ///
    /// # Errors
    ///
    /// If the read or write fails.
    pub fn claim_undo(&mut self, redo: bool) -> Result<Option<(i64, i64, String)>> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let query = if redo {
            "SELECT seq, format, edit FROM undo WHERE undone = 1 ORDER BY seq ASC LIMIT 1"
        } else {
            "SELECT seq, format, edit FROM undo WHERE undone = 0 ORDER BY seq DESC LIMIT 1"
        };
        let entry: Option<(i64, i64, String)> = tx
            .query_row(query, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .optional()?;
        if let Some((seq, _, _)) = &entry {
            tx.execute("UPDATE undo SET undone = ?1 WHERE seq = ?2", params![!redo, seq])?;
        }
        tx.commit()?;
        Ok(entry)
    }

    /// Puts a claimed entry back where it was, when applying it failed.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn unclaim_undo(&mut self, seq: i64, redo: bool) -> Result<()> {
        self.conn.execute("UPDATE undo SET undone = ?1 WHERE seq = ?2", params![redo, seq])?;
        Ok(())
    }

    /// Removes an entry outright — one this version cannot read.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn drop_undo(&mut self, seq: i64) -> Result<()> {
        self.conn.execute("DELETE FROM undo WHERE seq = ?1", params![seq])?;
        Ok(())
    }

    /// How many entries can be undone, and how many redone.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn undo_depths(&self) -> Result<(usize, usize)> {
        let count = |undone: bool| -> Result<usize> {
            let n: i64 = self.conn.query_row(
                "SELECT count(*) FROM undo WHERE undone = ?1",
                params![undone],
                |row| row.get(0),
            )?;
            Ok(usize::try_from(n).unwrap_or(0))
        };
        Ok((count(false)?, count(true)?))
    }
}

/// How syncing with one peer last went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerStatus {
    /// The peer, as hex.
    pub node_id: String,
    /// When this device last tried, in milliseconds since the epoch.
    pub last_attempt: Option<i64>,
    /// When it last succeeded.
    pub last_success: Option<i64>,
    /// What went wrong the last time it failed, if the last attempt failed.
    pub last_error: Option<String>,
}

impl Db {
    /// A value from this device's local state, if set.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn local_value(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM local_state WHERE key = ?1", params![key], |r| r.get(0))
            .optional()?)
    }

    /// What is kept under `key` (a document's name for its part of the snapshot, or
    /// `postponed` for the tally), if anything: the build that wrote it, the heads it was read
    /// at, and what was kept.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn hydrated(&self, key: &str) -> Result<Option<Kept>> {
        Ok(self
            .conn
            .query_row(
                "SELECT build, heads, part FROM hydrated WHERE doc_id = ?1",
                params![key],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, Vec<u8>>(2)?)),
            )
            .optional()?
            .map(|(build, heads, part)| {
                (build, heads.as_chunks::<32>().0.iter().map(|chunk| ChangeHash(*chunk)).collect(), part)
            }))
    }

    /// Keeps `part` under `key`, read at `heads` by `build`, in place of the last.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn keep_hydrated(&self, key: &str, build: &str, heads: &[ChangeHash], part: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO hydrated (doc_id, build, heads, part) VALUES (?1, ?2, ?3, ?4)",
            params![key, build, encode_heads(heads), part],
        )?;
        Ok(())
    }

    /// Sets a local value unless one is already there, and returns whichever is.
    ///
    /// The race this closes is two processes on a fresh profile each minting a device key:
    /// whichever writes first wins, and the other reads the winner's back, so both are the
    /// same device: identity is per store, not per process.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn local_value_or_insert(&mut self, key: &str, value: &[u8]) -> Result<Vec<u8>> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT OR IGNORE INTO local_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        let stored: Vec<u8> =
            tx.query_row("SELECT value FROM local_state WHERE key = ?1", params![key], |r| {
                r.get(0)
            })?;
        tx.commit()?;
        Ok(stored)
    }

    /// Sets a local value, replacing what was there.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn set_local_value(&mut self, key: &str, value: &[u8]) -> Result<()> {
        self.conn.execute("INSERT OR REPLACE INTO local_state (key, value) VALUES (?1, ?2)", params![key, value])?;
        Ok(())
    }

    /// Records one attempt to sync with a peer.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn record_peer(&mut self, node_id: &str, at_ms: i64, error: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO peers (node_id, last_attempt, last_success, last_error)
                 VALUES (?1, ?2, CASE WHEN ?3 IS NULL THEN ?2 END, ?3)
             ON CONFLICT (node_id) DO UPDATE SET
                 last_attempt = excluded.last_attempt,
                 last_success = coalesce(excluded.last_success, peers.last_success),
                 last_error = excluded.last_error",
            params![node_id, at_ms, error],
        )?;
        Ok(())
    }

    /// Every peer this device has tried to sync with.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn peers(&self) -> Result<Vec<PeerStatus>> {
        let mut stmt = self.conn.prepare(
            "SELECT node_id, last_attempt, last_success, last_error FROM peers ORDER BY node_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PeerStatus {
                node_id: row.get(0)?,
                last_attempt: row.get(1)?,
                last_success: row.get(2)?,
                last_error: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}
