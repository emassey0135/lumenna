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
    /// Set when this process took in another's changes outside [`Store::refresh`] — during
    /// a compaction — so that the next refresh still reports them. A watcher that only ever
    /// hears about changes from `refresh` would otherwise never be told.
    unreported: bool,
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
            unreported: false,
        };
        // `core` and `devices` are small and needed on every device; years are not loaded
        // until asked for.
        for id in [DocId::Core, DocId::Devices] {
            let (doc, stored) = store.read_document(id)?;
            store.adopt(doc, stored);
        }
        // The Inbox and the recurring-year index come from a deterministic change (see
        // `Doc::ensure_schema`). A store from before it gets it now — and, once, has its
        // years searched for recurring series the index has never heard of, since until now
        // nothing recorded them.
        if store.docs.ensure_schema()? {
            store.load_all_years()?;
            let (snapshot, _) = store.docs.snapshot();
            for series in snapshot.series.values().filter(|s| s.is_recurring()) {
                store.docs.note_recurring_year(series.start_date.year())?;
            }
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
        let (snapshot, changes) =
            self.db.consistently(|db| Ok((db.snapshot(id)?, db.changes(id)?)))?;
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

    /// Loads what showing a day in `year` needs: that year's blocks, and every earlier year
    /// holding a series that recurs into it.
    ///
    /// A series lives in the document for the year it **starts** (§3.1), so a daily routine
    /// begun in 2026 is in `blocks-2026` and nowhere else on 1 January 2027. Loading only
    /// 2027 would make it vanish at New Year. [`Documents::recurring_years`] says which
    /// earlier years hold recurring series, so those load too and years holding only one-off
    /// blocks stay on disk — which keeps the laziness a watch depends on (§16.9).
    ///
    /// # Errors
    ///
    /// If a document cannot be read.
    pub fn load_year(&mut self, year: i16) -> Result<()> {
        let mut years = vec![year];
        years.extend(self.docs.recurring_years().into_iter().filter(|y| *y < year));
        for year in years {
            if !self.docs.has_year(year) {
                let (doc, stored) = self.read_document(DocId::Blocks(year))?;
                self.adopt(doc, stored);
            }
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
        let before = doc.heads();
        // Everything on disk goes into the document before it is saved over the changes:
        // another process may have written since this one last refreshed, or compacted
        // changes this one never saw into the snapshot being replaced. See `Db::compact`.
        self.db.compact(id, |snapshot, changes| {
            if let Some(bytes) = snapshot {
                doc.load_incremental(&bytes)?;
            }
            for change in &changes {
                doc.load_incremental(change)?;
            }
            Ok((doc.heads(), doc.save()))
        })?;
        let after = doc.heads();
        if after != before {
            self.unreported = true;
            if self.persisted.get(&id) == Some(&before) {
                self.persisted.insert(id, after);
            }
        }
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
        let unreported = std::mem::take(&mut self.unreported);
        let version = self.db.data_version()?;
        if version == self.data_version {
            return Ok(unreported);
        }
        self.data_version = version;

        let cursor = self.cursor;
        let Self { db, docs, persisted, .. } = self;
        // One read transaction, so a compaction by another process cannot land between
        // reading the snapshot and reading the changes after the cursor (see
        // `Db::consistently`).
        let (changed, highest) = db.consistently(|db| {
            let mut changed = false;
            for doc in docs.iter_mut() {
                let before = doc.heads();
                // A snapshot whose heads this document lacks was compacted by another
                // process, out of changes that may now be deleted. Its rows are gone, so the
                // cursor will never show them; the snapshot is the only place they remain.
                if let Some(heads) = db.snapshot_heads(doc.id())?
                    && !heads.iter().all(|h| doc.has_change(h))
                    && let Some(bytes) = db.snapshot(doc.id())?
                {
                    doc.load_incremental(&bytes)?;
                }
                let (changes, _) = db.changes_after(doc.id(), cursor)?;
                for change in &changes {
                    doc.load_incremental(change)?;
                }
                let after = doc.heads();
                if after != before {
                    changed = true;
                    // Only when nothing local was waiting to be written: marking an
                    // unwritten local change as persisted would mean it is never written.
                    if persisted.get(&doc.id()) == Some(&before) {
                        persisted.insert(doc.id(), after);
                    }
                }
            }
            Ok((changed, db.cursor()?))
        })?;
        self.cursor = highest.max(cursor);
        Ok(changed || unreported)
    }

    /// The whole store as a backup file (see [`crate::backup`]).
    ///
    /// Every year is loaded first — a backup of the years someone happened to look at is not
    /// a backup — and another process's latest changes are taken in, so the file is as
    /// current as the database it came from.
    ///
    /// # Errors
    ///
    /// If a document cannot be read.
    pub fn backup(&mut self) -> Result<Vec<u8>> {
        self.refresh()?;
        self.load_all_years()?;
        let documents: Vec<(DocId, Vec<u8>)> =
            self.docs.iter_mut().map(|doc| (doc.id(), doc.save())).collect();
        Ok(crate::backup::encode(&documents))
    }

    /// Writes a backup under `policy`, pruning old ones. See [`crate::backup::write`].
    ///
    /// # Errors
    ///
    /// If a document cannot be read or the file cannot be written.
    pub fn back_up_to(
        &mut self,
        policy: &crate::backup::Policy,
        now: jiff::Timestamp,
    ) -> Result<std::path::PathBuf> {
        let bytes = self.backup()?;
        crate::backup::write(policy, &bytes, now)
    }

    /// Takes a backup if the newest under `policy` is older than its interval.
    ///
    /// What every client calls opportunistically (§9) — at launch, or on a one-shot command —
    /// since a schedule only exists where something stays running.
    ///
    /// # Errors
    ///
    /// If the directory cannot be read, or a due backup cannot be written.
    pub fn back_up_if_due(
        &mut self,
        policy: &crate::backup::Policy,
        now: jiff::Timestamp,
    ) -> Result<Option<std::path::PathBuf>> {
        if !crate::backup::is_due(policy, now)? {
            return Ok(None);
        }
        self.back_up_to(policy, now).map(Some)
    }

    /// Merges a backup into the store.
    ///
    /// **Merge, never replace.** A backup's documents are Automerge histories, so loading one
    /// into the live store adds whatever changes it holds that the store lacks and leaves
    /// everything else alone. Restoring onto a new device rebuilds the store; restoring onto
    /// one that has moved on since loses nothing it has done. What it cannot do is take back
    /// a later change — a task deleted after the backup stays deleted, because the deletion is
    /// history too. That is the honest limit of restoring into a CRDT.
    ///
    /// # Errors
    ///
    /// If the file is not a readable backup, or a document in it will not load.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<Restored> {
        let decoded = crate::backup::decode(bytes)?;
        let mut restored = Restored { unknown: decoded.unknown, ..Restored::default() };
        for (id, _) in &decoded.documents {
            if let DocId::Blocks(year) = id {
                self.load_year(*year)?;
            }
        }
        self.write(|docs| {
            for (id, data) in &decoded.documents {
                let doc = match id {
                    DocId::Core => docs.core(),
                    DocId::Devices => docs.devices(),
                    DocId::Blocks(year) => docs.blocks(*year),
                };
                let before = doc.heads();
                doc.load_incremental(data)?;
                restored.documents += 1;
                if doc.heads() != before {
                    restored.changed += 1;
                }
            }
            // A backup of a store from before the recurring-year index still has routines in
            // it, so the restored series are indexed the same way an old store's are.
            let (snapshot, _) = docs.snapshot();
            for series in snapshot.series.values().filter(|s| s.is_recurring()) {
                docs.note_recurring_year(series.start_date.year())?;
            }
            Ok(())
        })?;
        Ok(restored)
    }

    /// Writes an export's records into the store (see [`crate::export`]).
    ///
    /// Identifiers are kept, so importing the same file twice changes nothing the second
    /// time, and importing onto the store it came from only puts back what has changed since.
    /// A record that exists here is updated field by field against what is here — the same
    /// rule as every other write — and one that does not is created. Nothing absent from the
    /// file is touched: an export says what *is*, not what should be removed.
    ///
    /// # Errors
    ///
    /// If a document cannot be read or written.
    pub fn import(&mut self, imported: &crate::export::Imported) -> Result<Imports> {
        self.refresh()?;
        self.load_all_years()?;
        let (current, _) = self.docs.snapshot();
        let mut report = Imports::default();

        fn tally<T: PartialEq>(report: &mut Imports, before: Option<&T>, after: &T) -> bool {
            match before {
                Some(before) if before == after => {
                    report.unchanged += 1;
                    false
                }
                Some(_) => {
                    report.updated += 1;
                    true
                }
                None => {
                    report.created += 1;
                    true
                }
            }
        }

        self.write(|docs| {
            if let Some(settings) = &imported.settings
                && settings != &current.settings
            {
                docs.put_settings(settings, Some(&current.settings))?;
            }
            for p in &imported.projects {
                let before = current.projects.get(&p.id);
                if tally(&mut report, before, p) {
                    docs.put_project(p, before)?;
                }
            }
            for l in &imported.labels {
                let before = current.labels.get(&l.id);
                if tally(&mut report, before, l) {
                    docs.put_label(l, before)?;
                }
            }
            for f in &imported.filters {
                let before = current.saved_filters.get(&f.id);
                if tally(&mut report, before, f) {
                    docs.put_filter(f, before)?;
                }
            }
            for t in &imported.tasks {
                let before = current.tasks.get(&t.id);
                if tally(&mut report, before, t) {
                    docs.put_task(t, before)?;
                }
            }
            for c in &imported.completions {
                let before = current.completions.get(&c.id);
                if tally(&mut report, before, c) {
                    docs.put_completion(c, before)?;
                }
            }
            for s in &imported.series {
                let before = current.series.get(&s.id);
                if tally(&mut report, before, s) {
                    docs.put_series(s, before)?;
                }
            }
            for e in &imported.exceptions {
                let before = current.exceptions.get(&(e.series_id, e.original_date));
                if tally(&mut report, before, e) {
                    docs.put_exception(e, before)?;
                }
            }
            for a in &imported.assignments {
                // A one-off block's assignment lives in its series' year, and the series is in
                // the file or in the store; with neither there is no telling which document it
                // belongs to, and guessing would leave a fragment in the wrong one.
                let series = imported
                    .series
                    .iter()
                    .find(|s| s.id == a.block_ref.series_id())
                    .or_else(|| current.series.get(&a.block_ref.series_id()));
                let Some(year) = crate::doc::assignment_year(a, series) else {
                    report.skipped += 1;
                    continue;
                };
                let before = current.assignments.get(&a.id);
                if tally(&mut report, before, a) {
                    docs.put_assignment(year, a, before)?;
                }
            }
            for r in &imported.reminders {
                let before = current.reminders.get(&r.id);
                if tally(&mut report, before, r) {
                    docs.put_reminder(r, before)?;
                }
            }
            Ok(())
        })?;
        Ok(report)
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

/// What [`Store::restore`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Restored {
    /// How many documents the backup held that this version could read.
    pub documents: usize,
    /// How many of them brought in anything the store did not already have.
    pub changed: usize,
    /// Documents of a kind this version does not know, left out.
    pub unknown: Vec<String>,
}

/// What [`Store::import`] did, in records.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Imports {
    /// Records the store did not have.
    pub created: usize,
    /// Records it had, now changed to match the file.
    pub updated: usize,
    /// Records already exactly as the file has them.
    pub unchanged: usize,
    /// Assignments left out because their block is in neither the file nor the store.
    pub skipped: usize,
}
