//! A profile: the SQLite file and the documents currently loaded from it.
//!
//! This is §8's startup and write paths, and the two are shorter than the reasoning behind
//! them:
//!
//! **Startup** opens SQLite in WAL mode, loads `core` and `devices` from their snapshot plus
//! any changes after it, and records the highest change rowid as this process's watch
//! cursor. Year documents are *not* loaded — they arrive when a year is viewed, which is
//! what keeps the watch viable (§16.9).
//!
//! **Writes** mutate the in-memory document and append the resulting change bytes in one
//! SQLite transaction, so what is on disk can never disagree with what is in memory about
//! whether an edit happened.
//!
//! Nothing here is privileged. A `lum sync-daemon` is simply whichever participant happens
//! to be headless, running this same code path as the tray app (§8), and several processes
//! may hold the same file open at once — which is what [`Store::refresh`] is for.

use std::collections::BTreeMap;
use std::path::Path;

use automerge::ChangeHash;
use lumenna_core::snapshot::Snapshot;

use crate::db::Db;
use crate::doc::{Doc, DocId, Documents, HydrationReport};
use crate::error::Result;

/// One profile's store.
pub struct Store {
    db: Db,
    docs: Documents,
    /// The heads already written to SQLite, per document. What is in the document but not
    /// in here is what the next write has to append.
    persisted: BTreeMap<DocId, Vec<ChangeHash>>,
    /// The highest change rowid this process has seen. Everything past it is news from
    /// another process, or from a peer.
    cursor: i64,
    data_version: i64,
}

impl Store {
    /// Opens the profile at `path`, creating it if it is not there.
    ///
    /// # Errors
    ///
    /// If the file cannot be opened, or a stored document cannot be loaded.
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_db(Db::open(path)?)
    }

    /// An in-memory profile, for tests.
    ///
    /// # Errors
    ///
    /// If the schema cannot be created.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_db(Db::open_in_memory()?)
    }

    fn from_db(db: Db) -> Result<Self> {
        let mut store = Self {
            persisted: BTreeMap::new(),
            cursor: db.cursor()?,
            data_version: db.data_version()?,
            db,
            docs: Documents::new(),
        };
        // `core` and `devices` are small and needed on every device; years are not loaded
        // until asked for.
        for id in [DocId::Core, DocId::Devices] {
            let (doc, stored) = store.read_document(id)?;
            store.adopt(doc, stored);
        }
        // A fresh profile records its genesis now rather than letting the first real edit
        // carry it. Cheap, and it means "the file exists" and "the file holds a store" are
        // the same statement.
        store.persist()?;
        Ok(store)
    }

    /// Reads a document out of SQLite, or mints a fresh one if the file has never held it.
    ///
    /// A fresh document's genesis is deterministic ([`Doc::new`]), so a device that mints
    /// one for a year another device has already started does not fork the collection —
    /// the two geneses are the same change.
    /// Returns the document and whether the file already held it.
    fn read_document(&self, id: DocId) -> Result<(Doc, bool)> {
        let snapshot = self.db.snapshot(id)?;
        let changes = self.db.changes(id)?;
        let stored = snapshot.is_some() || !changes.is_empty();
        let mut doc = match snapshot {
            Some(bytes) => Doc::load(id, &bytes)?,
            None => Doc::new(id),
        };
        for change in changes {
            doc.load_incremental(&change)?;
        }
        Ok((doc, stored))
    }

    fn adopt(&mut self, mut doc: Doc, stored: bool) {
        // A document read from the file is by definition already persisted. A freshly
        // minted one is not, genesis included — so it starts with no known heads and the
        // next persist writes everything.
        let heads = if stored { doc.heads() } else { Vec::new() };
        self.persisted.insert(doc.id(), heads);
        self.docs.insert(doc);
    }

    /// The documents, read-only.
    #[must_use]
    pub fn documents(&self) -> &Documents {
        &self.docs
    }

    /// Materializes everything loaded.
    #[must_use]
    pub fn snapshot(&self) -> (Snapshot, HydrationReport) {
        self.docs.snapshot()
    }

    /// Loads one year of blocks, if it is not already loaded.
    ///
    /// # Errors
    ///
    /// If the document cannot be read.
    pub fn load_year(&mut self, year: i16) -> Result<()> {
        if !self.docs.has_year(year) {
            let (doc, stored) = self.read_document(DocId::Blocks(year))?;
            self.adopt(doc, stored);
        }
        Ok(())
    }

    /// Loads every year this file holds.
    ///
    /// Lazy loading is what keeps a watch viable (§8), and most commands want exactly one
    /// year. But anything that has to *find* a block or an assignment by identifier cannot
    /// know which year to open without opening them — so those pay the cost deliberately,
    /// rather than silently failing to find a record that is right there on disk.
    ///
    /// # Errors
    ///
    /// If a stored document cannot be read.
    pub fn load_all_years(&mut self) -> Result<()> {
        for name in self.db.stored_documents()? {
            if let Some(year) = name.strip_prefix("blocks-")
                && let Ok(year) = year.parse::<i16>()
            {
                self.load_year(year)?;
            }
        }
        Ok(())
    }

    /// Runs `edit` against the documents and persists whatever it changed.
    ///
    /// Writing is the only way to reach the documents mutably, which is deliberate: an edit
    /// that never reached SQLite would be lost on exit and, worse, would be invisible to
    /// the other processes sharing this file.
    ///
    /// # Errors
    ///
    /// If `edit` fails, or the changes cannot be written. A failure inside `edit` still
    /// persists whatever it managed to do first — Automerge has already recorded those
    /// operations, and leaving them only in memory would be the more surprising outcome.
    pub fn write<R>(&mut self, edit: impl FnOnce(&mut Documents) -> Result<R>) -> Result<R> {
        let result = edit(&mut self.docs);
        self.persist()?;
        result
    }

    /// Appends every loaded document's new changes, then compacts anything that has grown
    /// enough to be worth it.
    fn persist(&mut self) -> Result<()> {
        let mut compact = Vec::new();
        for doc in self.docs.iter_mut() {
            let id = doc.id();
            let known = self.persisted.get(&id).cloned().unwrap_or_default();
            let heads = doc.heads();
            if heads == known {
                continue;
            }
            let changes = doc.changes_since(&known);
            self.db.append_changes(id, &changes)?;
            self.persisted.insert(id, heads);
            if self.db.needs_compaction(id)? {
                compact.push(id);
            }
        }
        for id in compact {
            self.compact(id)?;
        }
        // Deliberately does not touch `cursor` or `data_version`.
        //
        // `PRAGMA data_version` does not move for your own connection's writes, so
        // re-reading it here would record a value that already reflects *another*
        // process's write and make `refresh` skip it. And advancing the cursor past rows
        // this process appended would skip anything another process interleaved between
        // them. The cursor only ever moves in `refresh`, where the rows it passes have
        // actually been applied; re-reading a change already held is a no-op, so lagging
        // costs nothing and skipping would lose an edit.
        Ok(())
    }

    /// Replaces a document's loose changes with a snapshot of it.
    ///
    /// Happens on its own once a document has accumulated enough changes to make replay
    /// noticeable; this is the same thing on demand, for a caller that would rather pay the
    /// cost at a moment it chooses.
    ///
    /// It discards no operations — an Automerge snapshot carries full history, which is why
    /// §9 can use the same bytes as a backup.
    ///
    /// # Errors
    ///
    /// If the document is not loaded, or the write fails.
    pub fn compact_document(&mut self, id: DocId) -> Result<()> {
        self.compact(id)
    }

    fn compact(&mut self, id: DocId) -> Result<()> {
        let Some(doc) = self.docs.iter_mut().find(|d| d.id() == id) else {
            return Ok(());
        };
        let heads = doc.heads();
        let data = doc.save();
        self.db.compact(id, &heads, &data)?;
        Ok(())
    }

    /// Picks up changes written by another process, returning whether anything arrived.
    ///
    /// SQLite has no cross-process notification, so the trigger for calling this comes from
    /// outside — a `-wal` file event, or a one-second timer where file watching is
    /// unreliable. What makes either cheap is that
    /// [`data_version`](Db::data_version) settles in one pragma read whether there is
    /// anything to do (§8).
    ///
    /// # Errors
    ///
    /// If the changes cannot be read or applied.
    pub fn refresh(&mut self) -> Result<bool> {
        let version = self.db.data_version()?;
        if version == self.data_version {
            return Ok(false);
        }
        self.data_version = version;

        let mut changed = false;
        let cursor = self.cursor;
        let mut highest = cursor;
        for doc in self.docs.iter_mut() {
            let (changes, last) = self.db.changes_after(doc.id(), cursor)?;
            for change in &changes {
                doc.load_incremental(change)?;
                changed = true;
            }
            highest = highest.max(last);
            if changed {
                self.persisted.insert(doc.id(), doc.heads());
            }
        }
        self.cursor = highest;
        Ok(changed)
    }

    /// Applies an [`Edit`] and persists it.
    ///
    /// # Errors
    ///
    /// If the edit cannot be applied, or the changes cannot be written.
    pub fn apply(&mut self, edit: &lumenna_core::edit::Edit) -> Result<()> {
        self.write(|docs| docs.apply(edit))
    }

    /// This process's watch cursor: the highest change rowid it has seen.
    #[must_use]
    pub fn cursor(&self) -> i64 {
        self.cursor
    }

    /// The underlying file, for callers that need to ask it something directly.
    #[must_use]
    pub fn db(&self) -> &Db {
        &self.db
    }
}
