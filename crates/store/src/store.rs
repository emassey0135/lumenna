//! A profile: the SQLite file and the documents currently loaded from it.
//!
//! This is the startup and write paths, and the two are shorter than the reasoning behind
//! them:
//!
//! **Startup** opens SQLite in WAL mode, loads `core` and `devices` from their snapshot plus
//! any changes after it, and records the highest change rowid as this process's watch
//! cursor. Year documents are *not* loaded — they arrive when a year is viewed, which is
//! what keeps a watch viable, where memory is tight.
//!
//! **Writes** mutate the in-memory document and append the resulting change bytes in one
//! SQLite transaction, so what is on disk can never disagree with what is in memory about
//! whether an edit happened.
//!
//! Nothing here is privileged. The daemon is simply whichever participant happens to be
//! headless, running this same code path as the tray app, and several processes
//! may hold the same file open at once — which is what [`Store::refresh`] is for.

use std::collections::BTreeMap;
use std::path::Path;

use automerge::ChangeHash;
use lumenna_core::snapshot::Snapshot;

use crate::db::Db;
use crate::doc::{Doc, DocId, Documents, HydrationReport};
use crate::error::{Result, StoreError};

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
    /// The heads each device was last synced at over a direct link — a watch's Bluetooth to
    /// its phone — so the Iroh round can skip it while nothing has changed since. Memory
    /// only: a restart costs one round that finds nothing to send.
    linked: BTreeMap<String, Vec<ChangeHash>>,
    /// How often each task's due date was moved later, kept as the core document grows.
    postponed: crate::postponed::Postponed,
    /// Those counts, by task, as the snapshot carries them.
    postponements: BTreeMap<lumenna_core::id::TaskId, u32>,
    /// Each loaded document's part of the snapshot, as last materialized.
    hydrated: BTreeMap<DocId, Hydrated>,
}

/// A document's part of the snapshot, kept as of `heads`.
///
/// Materializing is most of what answering costs (2,000 tasks took 100 ms, and one command
/// asked for it four times: opening, repairs, settings, the query), and a resident app asks
/// on every turn, though nothing changed since the last. Kept per document, so loading a year
/// reads that year alone; when a document moves, Automerge's diff from `heads` says which
/// records changed, and only those are read again. Keyed by heads, not by a count of writes,
/// so nothing that changes a document can forget to say so.
struct Hydrated {
    heads: Vec<ChangeHash>,
    part: Snapshot,
    skipped: Vec<crate::doc::Skipped>,
    /// When this part was last kept on disk for the next process (see [`Store::snapshot`]).
    saved_at: Option<jiff::Timestamp>,
}

/// Which build materialized a part kept on disk: a hash of every source of the model and of
/// hydration (`build.rs`), so a part is used only by a build that reads every record alike.
const HYDRATION_BUILD: &str = env!("LUMENNA_HYDRATION_BUILD");

/// The key the postponements' tally is kept under, beside the documents' parts.
const POSTPONED: &str = "postponed";

/// How often a part a running process follows is kept on disk again, at most: often enough that
/// the next process has little to follow, seldom enough that a write costs no more than it did.
const SAVE_EVERY: jiff::SignedDuration = jiff::SignedDuration::from_secs(60);

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
            linked: BTreeMap::new(),
            postponed: crate::postponed::Postponed::default(),
            postponements: BTreeMap::new(),
            hydrated: BTreeMap::new(),
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
        // The postponements' tally as an earlier process kept it, read on from there rather than
        // from the start of the history (see `Store::snapshot`).
        if let Some(kept) = tally_from_disk(&store.db, store.docs.core()) {
            store.postponed = kept;
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
        // Every change in one call: Automerge reads a run of chunks and orders them by their
        // dependencies itself, where a call per change paid its setup hundreds of times.
        if !changes.is_empty() {
            doc.load_incremental(&changes.concat())?;
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

    /// Materializes everything loaded, reading again only the documents that have moved
    /// since the last time (see [`Hydrated`]).
    ///
    /// A document this process has not yet read starts from the part an earlier process kept
    /// on disk, when this build wrote it and the document holds the heads it was read at, and
    /// follows the changes since as it would its own. A part is kept on disk after a whole
    /// reading, after following one from disk, and while followed at most every
    /// [`SAVE_EVERY`]; never with a record it could not read, which a kept part does not carry.
    #[must_use]
    pub fn snapshot(&mut self) -> (Snapshot, HydrationReport) {
        let mut loaded = Vec::new();
        let now = lumenna_core::time::now();
        for doc in self.docs.iter_mut() {
            let (id, heads) = (doc.id(), doc.heads());
            let in_memory = match self.hydrated.get_mut(&id) {
                Some(kept) if kept.heads == heads => Some(false),
                // Moved since: only what the changes touched is read again.
                Some(kept) => doc.refresh_part(&kept.heads, &heads, &mut kept.part, &mut kept.skipped).then(|| {
                    kept.heads.clone_from(&heads);
                    kept.saved_at.is_none_or(|at| now.duration_since(at) >= SAVE_EVERY)
                }),
                None => None,
            };
            let save = in_memory.unwrap_or_else(|| {
                let (part, skipped, saved) = from_disk(&self.db, doc, id, &heads).unwrap_or_else(|| {
                    let mut skipped = Vec::new();
                    (doc.part(&mut skipped), skipped, Vec::new())
                });
                self.hydrated.insert(id, Hydrated { heads: heads.clone(), part, skipped, saved_at: None });
                // Read whole, or brought up to date from disk: kept again, so the next process
                // has nothing to do. Up to date there already: left as it is.
                saved != heads
            });
            if let Some(kept) = self.hydrated.get_mut(&id)
                && save
                && kept.skipped.is_empty()
                && self.db.keep_hydrated(&id.name(), HYDRATION_BUILD, &heads, &crate::doc::encode_part(&kept.part)).is_ok()
            {
                kept.saved_at = Some(now);
                // The postponements' tally reads the core document's history from its start, so
                // it is kept with that document's part, and the next process reads on from it.
                if id == DocId::Core {
                    let tally = (self.postponed.heads(), self.postponed.encode());
                    let _ = self.db.keep_hydrated(POSTPONED, HYDRATION_BUILD, tally.0, &tally.1);
                }
            }
            loaded.push(id);
        }
        self.hydrated.retain(|id, _| loaded.contains(id));
        let mut snapshot = Snapshot::default();
        let mut report = HydrationReport::default();
        for id in loaded {
            let kept = &self.hydrated[&id];
            crate::doc::absorb(&mut snapshot, id, &kept.part);
            report.skipped.extend(kept.skipped.iter().cloned());
        }
        snapshot.postponed.clone_from(&self.postponements);
        (snapshot, report)
    }

    /// Reads the core document's new changes into how often each task's date was moved later.
    fn tally(&mut self) {
        let counts = self.postponed.update(self.docs.core().automerge_mut());
        self.postponements = counts.into_iter().filter_map(|(task, count)| Some((task.parse().ok()?, count))).collect();
    }

    /// Loads what showing a day in `year` needs: that year's blocks, and every earlier year
    /// holding a series that recurs into it.
    ///
    /// A series lives in the document for the year it **starts**, so a daily routine
    /// begun in 2026 is in `blocks-2026` and nowhere else on 1 January 2027. Loading only
    /// 2027 would make it vanish at New Year. [`Documents::recurring_years`] says which
    /// earlier years hold recurring series, so those load too and years holding only one-off
    /// blocks stay on disk — which keeps the laziness a watch depends on.
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
    /// Lazy loading is what keeps a watch viable, and most commands want exactly one
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
            if doc.heads() == known {
                continue;
            }
            // Written to by this build: it says so, on the way to disk, so no write misses it.
            doc.stamp_version()?;
            let heads = doc.heads();
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
        self.tally();
        Ok(())
    }

    /// Replaces a document's loose changes with a snapshot of it.
    ///
    /// Happens on its own once a document has accumulated enough changes to make replay
    /// noticeable; this is the same thing on demand, for a caller that would rather pay the
    /// cost at a moment it chooses.
    ///
    /// It discards no operations — an Automerge snapshot carries full history, which is why
    /// a backup uses the same bytes.
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
    /// anything to do.
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
        if changed {
            self.tally();
        }
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
    /// What every client calls opportunistically — at launch, or on a one-shot command —
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

        // What the export held beyond the model goes back into each record it creates.
        self.docs.revive(imported.unknown.clone());
        let written = self.write(|docs| {
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
        });
        self.docs.forget_revived();
        written?;
        Ok(report)
    }

    /// This device's secret key, minted the first time anything asks.
    ///
    /// It lives in the store, so every process using the store *is* the same device. It is
    /// local state and never syncs, never appears in a backup, and is never typed.
    /// `fresh` supplies a new key if there is none yet; store does not generate keys itself
    /// because which kind of key is the transport's business.
    ///
    /// # Errors
    ///
    /// If the local state cannot be read or written, or holds something that is not a key.
    pub fn device_secret(&mut self, fresh: impl FnOnce() -> [u8; 32]) -> Result<[u8; 32]> {
        const KEY: &str = "device_secret_key";
        let stored = match self.db.local_value(KEY)? {
            Some(value) => value,
            None => self.db.local_value_or_insert(KEY, &fresh())?,
        };
        stored.try_into().map_err(|_| {
            StoreError::Unreadable("this device's stored key is damaged".to_owned())
        })
    }

    /// Every document this store holds or has loaded, for a sync session to offer a peer.
    ///
    /// # Errors
    ///
    /// If the stored documents cannot be listed.
    pub fn sync_documents(&self) -> Result<Vec<DocId>> {
        let mut ids: Vec<DocId> = self
            .db
            .stored_documents()?
            .iter()
            .filter_map(|name| DocId::parse(name))
            .chain([DocId::Core, DocId::Devices])
            .chain(self.docs.loaded_years().into_iter().map(DocId::Blocks))
            .collect();
        ids.sort();
        ids.dedup();
        Ok(ids)
    }

    /// The next sync message for one document, loading it first if it is a year not yet
    /// opened.
    ///
    /// # Errors
    ///
    /// If the document cannot be read.
    pub fn sync_message(
        &mut self,
        id: DocId,
        state: &mut automerge::sync::State,
    ) -> Result<Option<Vec<u8>>> {
        self.open_for_sync(id)?;
        Ok(self.docs.get_mut(id).sync_message(state))
    }

    /// Takes in a peer's sync message for one document and persists what it brought, so
    /// every other process on the device sees it through the ordinary change path.
    ///
    /// Returns whether the document changed.
    ///
    /// # Errors
    ///
    /// If the message does not read, or the document cannot be read or written.
    pub fn receive_sync_message(
        &mut self,
        id: DocId,
        state: &mut automerge::sync::State,
        bytes: &[u8],
    ) -> Result<bool> {
        self.open_for_sync(id)?;
        self.write(|docs| {
            let doc = docs.get_mut(id);
            let before = doc.heads();
            doc.receive_sync_message(state, bytes)?;
            Ok(doc.heads() != before)
        })
    }

    fn open_for_sync(&mut self, id: DocId) -> Result<()> {
        if let DocId::Blocks(year) = id {
            self.load_year(year)?;
        }
        Ok(())
    }

    /// Records how an attempt to sync with a peer went.
    ///
    /// # Errors
    ///
    /// If the local state cannot be written.
    pub fn record_peer(&mut self, node_id: &str, error: Option<&str>) -> Result<()> {
        self.db.record_peer(node_id, jiff::Timestamp::now().as_millisecond(), error)
    }

    /// How syncing with each peer last went.
    ///
    /// # Errors
    ///
    /// If the local state cannot be read.
    pub fn peers(&self) -> Result<Vec<crate::db::PeerStatus>> {
        self.db.peers()
    }

    /// Applies an [`Edit`] the user made, and saves it to this device's undo history.
    ///
    /// Everything a person does goes through here; what the app does on their behalf — folding
    /// a legacy Inbox, importing, restoring — goes through [`Store::apply`] and is not theirs
    /// to undo one record at a time.
    ///
    /// # Errors
    ///
    /// If the edit cannot be applied, or the history cannot be written.
    pub fn apply_recorded(&mut self, edit: &lumenna_core::edit::Edit) -> Result<()> {
        // What it removes outright, as it stands, before it goes: its undo writes it back.
        let removed = self.docs.removed_by(edit);
        self.apply(edit)?;
        if !edit.is_empty() {
            let text = crate::undo::save(edit, &removed)
                .map_err(|e| StoreError::Io(format!("could not save the undo history: {e}")))?;
            let at = jiff::Timestamp::now().as_millisecond();
            self.db.record_undo(crate::undo::FORMAT, &text, at, crate::undo::DEPTH)?;
        }
        Ok(())
    }

    /// Undoes the newest edit on this device's history. See [`crate::undo`].
    ///
    /// # Errors
    ///
    /// If the documents cannot be read or written.
    pub fn undo(&mut self) -> Result<crate::undo::Step> {
        self.take_step(crate::undo::Direction::Undo)
    }

    /// Redoes the edit most recently undone.
    ///
    /// # Errors
    ///
    /// If the documents cannot be read or written.
    pub fn redo(&mut self) -> Result<crate::undo::Step> {
        self.take_step(crate::undo::Direction::Redo)
    }

    /// How many edits can be undone, and how many redone.
    ///
    /// # Errors
    ///
    /// If the history cannot be read.
    pub fn undo_depths(&self) -> Result<(usize, usize)> {
        self.db.undo_depths()
    }

    fn take_step(&mut self, direction: crate::undo::Direction) -> Result<crate::undo::Step> {
        use crate::undo::{Direction, Step};
        let redo = direction == Direction::Redo;
        // Rebased against everything, so a record in a year not yet opened is still found.
        self.refresh()?;
        self.load_all_years()?;

        let Some((seq, format, text)) = self.db.claim_undo(redo)? else {
            return Ok(Step::Nothing);
        };
        let Some((edit, removed)) = crate::undo::read(format, &text) else {
            self.db.drop_undo(seq)?;
            return Ok(Step::Unreadable);
        };

        let (snapshot, _) = self.docs.snapshot();
        let reverted = crate::undo::rebase(&edit, direction, &snapshot);
        // An undo that creates again what the edit removed writes it back as it stood first.
        if !redo {
            self.docs.revive(removed);
        }
        let applied = self.apply(&reverted.applied);
        self.docs.forget_revived();
        if let Err(error) = applied {
            self.db.unclaim_undo(seq, redo)?;
            return Err(error);
        }
        Ok(Step::Done(reverted))
    }

    /// Applies an [`Edit`] and persists it.
    ///
    /// # Errors
    ///
    /// If the edit cannot be applied, or the changes cannot be written.
    pub fn apply(&mut self, edit: &lumenna_core::edit::Edit) -> Result<()> {
        self.write(|docs| docs.apply(edit))
    }

    /// Every loaded document's heads: a value that changes whenever anything is written,
    /// whoever wrote it.
    ///
    /// [`refresh`](Self::refresh) only reports what *other* connections wrote, because
    /// `data_version` does not move for this one's own writes. Something that shares this
    /// connection and needs to notice those too — the sync loop in an app, which must send a
    /// local edit on — compares this instead.
    #[must_use]
    pub fn version(&mut self) -> Vec<automerge::ChangeHash> {
        self.docs.iter_mut().flat_map(|doc| doc.heads()).collect()
    }

    /// Whether this device has set the local flag `key`: state of its own, never synced.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn local_flag(&self, key: &str) -> Result<bool> {
        Ok(self.db.local_value(key)?.is_some())
    }

    /// Sets the local flag `key`.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn set_local_flag(&mut self, key: &str) -> Result<()> {
        self.db.local_value_or_insert(key, b"1")?;
        Ok(())
    }

    /// A value of this device's own, never synced, if set.
    ///
    /// # Errors
    ///
    /// If the read fails.
    pub fn local_value(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.db.local_value(key)
    }

    /// Sets a value of this device's own, replacing what was there.
    ///
    /// # Errors
    ///
    /// If the write fails.
    pub fn set_local_value(&mut self, key: &str, value: &[u8]) -> Result<()> {
        self.db.set_local_value(key, value)
    }

    /// Each document synced and its heads, as this device has them.
    ///
    /// # Errors
    ///
    /// If a document cannot be read.
    pub fn sync_heads(&mut self) -> Result<Vec<(DocId, Vec<automerge::ChangeHash>)>> {
        let mut heads = Vec::new();
        for id in self.sync_documents()? {
            self.open_for_sync(id)?;
            heads.push((id, self.docs.get_mut(id).heads()));
        }
        Ok(heads)
    }

    /// Document `id`'s changes that are not among `heads` or before them, each as its bytes:
    /// what a device that has `heads` lacks.
    ///
    /// # Errors
    ///
    /// If the document cannot be read.
    pub fn changes_since(&mut self, id: DocId, heads: &[automerge::ChangeHash]) -> Result<Vec<Vec<u8>>> {
        self.open_for_sync(id)?;
        Ok(self.docs.get_mut(id).changes_since(heads).into_iter().map(|(_, bytes)| bytes).collect())
    }

    /// Takes in changes to document `id` that came from another device, whatever this device
    /// already has. Returns whether the document changed.
    ///
    /// # Errors
    ///
    /// If a change does not read, or the document cannot be read or written.
    pub fn take_changes(&mut self, id: DocId, changes: &[Vec<u8>]) -> Result<bool> {
        self.open_for_sync(id)?;
        self.write(|docs| {
            let doc = docs.get_mut(id);
            let before = doc.heads();
            for change in changes {
                doc.load_incremental(change)?;
            }
            Ok(doc.heads() != before)
        })
    }

    /// Notes that `node_id` was just synced over a direct link, at the heads the store has now.
    pub fn note_linked(&mut self, node_id: &str) {
        let heads = self.version();
        self.linked.insert(node_id.to_owned(), heads);
    }

    /// Whether `node_id` was synced over a direct link and nothing has changed here since:
    /// an Iroh round would have nothing to send it.
    pub fn linked_and_unchanged(&mut self, node_id: &str) -> bool {
        let heads = self.version();
        self.linked.get(node_id) == Some(&heads)
    }

    /// A number that moves when another connection commits to the store, and for nothing
    /// else: SQLite's `data_version`, which this connection's own writes leave alone.
    ///
    /// Unlike [`refresh`](Self::refresh)'s answer, no other call can use it up, and unlike
    /// [`version`](Self::version), this connection's own edits do not move it.
    ///
    /// # Errors
    ///
    /// If SQLite cannot be asked.
    pub fn outside_version(&self) -> Result<i64> {
        self.db.data_version()
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

/// The part of `doc`'s snapshot an earlier process kept on disk, brought up to `heads`: the
/// part, what it could not read, and the heads it was kept at. Nothing when this build
/// did not write it, the document lacks the heads it was read at (a store made afresh in its
/// place), it does not read, or the changes since cannot be followed record by record.
fn from_disk(
    db: &Db,
    doc: &mut Doc,
    id: DocId,
    heads: &[ChangeHash],
) -> Option<(Snapshot, Vec<crate::doc::Skipped>, Vec<ChangeHash>)> {
    let (build, saved, bytes) = db.hydrated(&id.name()).ok()??;
    if build != HYDRATION_BUILD || !saved.iter().all(|head| doc.has_change(head)) {
        return None;
    }
    let mut part = crate::doc::decode_part(&bytes)?;
    let mut skipped = Vec::new();
    if saved != heads && !doc.refresh_part(&saved, heads, &mut part, &mut skipped) {
        return None;
    }
    Some((part, skipped, saved))
}

/// The postponements' tally an earlier process kept, when this build wrote it and the core
/// document holds the heads it read to.
fn tally_from_disk(db: &Db, core: &mut Doc) -> Option<crate::postponed::Postponed> {
    let (build, saved, bytes) = db.hydrated(POSTPONED).ok()??;
    let tally = crate::postponed::Postponed::decode(&bytes)?;
    (build == HYDRATION_BUILD && tally.heads() == saved && saved.iter().all(|head| core.has_change(head))).then_some(tally)
}

#[cfg(test)]
mod tests {
    use lumenna_core::ProjectId;
    use lumenna_core::edit::{create_task, purge_task};
    use lumenna_core::model::Task;
    use lumenna_core::order::OrderKey;

    use super::*;

    #[test]
    fn undoing_an_erase_brings_back_what_a_newer_build_wrote_on_the_task() {
        let mut store = Store::open_in_memory().unwrap();
        let task = Task::new(ProjectId::INBOX, "essay", OrderKey::middle());
        store.apply_recorded(&create_task(task.clone())).unwrap();
        let key = task.id.to_string();
        store.docs.core().plant("tasks", &key, "written_by_a_newer_build", "keep me");

        let (snapshot, _) = store.snapshot();
        store.apply_recorded(&purge_task(&snapshot, task.id).unwrap()).unwrap();
        assert!(store.snapshot().0.tasks.is_empty(), "erased");
        assert!(matches!(store.undo().unwrap(), crate::undo::Step::Done(_)));

        assert_eq!(store.snapshot().0.tasks.get(&task.id).map(|t| t.title.as_str()), Some("essay"));
        assert_eq!(store.docs.core().field("tasks", &key, "written_by_a_newer_build").as_deref(), Some("keep me"));
    }

    #[test]
    fn a_json_export_carries_what_a_newer_build_wrote_into_the_store_it_is_imported_into() {
        let mut store = Store::open_in_memory().unwrap();
        let task = Task::new(ProjectId::INBOX, "essay", OrderKey::middle());
        store.apply_recorded(&create_task(task.clone())).unwrap();
        let key = task.id.to_string();
        store.docs.core().plant("tasks", &key, "written_by_a_newer_build", "keep me");

        let (snapshot, _) = store.snapshot();
        let json = crate::export::export_json_keeping(&snapshot, jiff::Timestamp::now(), store.documents());
        assert!(json.contains("written_by_a_newer_build"), "{json}");
        let mut elsewhere = Store::open_in_memory().unwrap();
        elsewhere.import(&crate::export::parse_json(&json).unwrap()).unwrap();
        assert_eq!(elsewhere.snapshot().0.tasks.get(&task.id).map(|t| t.title.as_str()), Some("essay"));
        assert_eq!(elsewhere.docs.core().field("tasks", &key, "written_by_a_newer_build").as_deref(), Some("keep me"));
        // What the model holds is written over it, as it is anywhere else.
        assert_eq!(elsewhere.docs.core().field("tasks", &key, "title").as_deref(), Some("essay"));
    }

    #[test]
    fn an_undo_entry_of_the_format_before_still_reads() {
        let task = Task::new(ProjectId::INBOX, "essay", OrderKey::middle());
        let old = serde_json::to_string(&create_task(task)).unwrap();
        let (edit, removed) = crate::undo::read(1, &old).unwrap();
        assert_eq!(edit.description, "Added essay");
        assert!(removed.is_empty());
    }
}
