//! Automerge documents, and how the data is sharded between them.
//!
//! Automerge loads and syncs **whole documents**, so how the data is divided between them
//! is a modelling decision with consequences you cannot undo later:
//!
//! - **`core`** — tasks, projects, labels, filters, reminders, settings. Small, and needed
//!   on every device including the watch.
//! - **`blocks-<year>`** — block series, exceptions, and assignments for one calendar year.
//!   This is what bounds memory on constrained devices and stops history growing without
//!   limit in one document. Years load lazily, only when viewed.
//! - **`devices`** — the paired roster.
//!
//! **There is no referential integrity across documents.** An assignment in `blocks-2027`
//! may name a task the local `core` has not merged yet, and a device holding only this year
//! is not holding a broken store. Everything downstream — hydration here, repair and
//! queries in core — has to treat a dangling reference as ordinary.

use std::collections::BTreeMap;

use automerge::transaction::Transactable;
use automerge::transaction::CommitOptions;
use automerge::{ActorId, AutoCommit, ChangeHash, ObjId, ObjType, ROOT, ReadDoc, Value};
use jiff::civil;
use lumenna_core::model::SCHEMA_VERSION;
use lumenna_core::model::{
    BlockAssignment, BlockException, BlockSeries, Device, Label, Project, Reminder, ReminderAck,
    SavedFilter, Settings, Task, TaskCompletion,
};
use lumenna_core::edit::{Change, Edit};
use lumenna_core::id::ProjectId;
use lumenna_core::snapshot::Snapshot;

use crate::error::{Result, StoreError};
use crate::records::{Record, read_settings, write_settings};
use crate::value::{Reader, Writer};

/// The actor that writes every document's genesis change.
///
/// Shared by every device on purpose, and used for exactly one change. See [`Doc::new`].
const GENESIS_ACTOR: &[u8] = b"lumenna-genesis-0";

/// The root map in `core` that records which years hold a recurring block series.
const SERIES_YEARS: &str = "series_years";

/// `core`'s second change, and its hash. See [`Doc::ensure_schema`].
///
/// **Frozen.** Its bytes are part of every store's history and must come out identical from
/// every build that ever makes it, so it writes literal keys rather than going through
/// [`Record::write`](crate::records::Record::write), whose layout is free to change. Never
/// edit what this writes; a further schema step is a further change on top.
fn core_schema_change() -> &'static (ChangeHash, Vec<u8>) {
    static CHANGE: std::sync::OnceLock<(ChangeHash, Vec<u8>)> = std::sync::OnceLock::new();
    CHANGE.get_or_init(|| {
        let mut doc = Doc::new(DocId::Core).doc;
        let genesis = doc.get_heads();
        doc.set_actor(ActorId::from(GENESIS_ACTOR));
        let projects = match doc.get(ROOT, "projects") {
            Ok(Some((Value::Object(ObjType::Map), id))) => id,
            _ => unreachable!("the genesis creates `projects`"),
        };
        let inbox = doc
            .put_object(&projects, ProjectId::INBOX.to_string(), ObjType::Map)
            .expect("a fresh map accepts a key");
        for (key, value) in [("name", "Inbox"), ("order", "V")] {
            doc.put(&inbox, key, value).expect("a fresh map accepts a key");
        }
        for (key, value) in [("archived", false), ("is_inbox", true)] {
            doc.put(&inbox, key, value).expect("a fresh map accepts a key");
        }
        doc.put_object(ROOT, SERIES_YEARS, ObjType::Map).expect("the root accepts a map");
        doc.commit_with(CommitOptions::default().with_time(0).with_message("inbox"));
        let change = doc
            .get_changes(&genesis)
            .into_iter()
            .next()
            .expect("the change just committed");
        (change.hash(), change.raw_bytes().to_vec())
    })
}

/// Which document this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocId {
    /// Tasks, projects, labels, filters, reminders, settings.
    Core,
    /// One calendar year of blocks.
    Blocks(i16),
    /// The paired device roster.
    Devices,
}

impl DocId {
    /// The name this document is stored and synced under.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Core => "core".to_owned(),
            Self::Blocks(year) => format!("blocks-{year}"),
            Self::Devices => "devices".to_owned(),
        }
    }

    /// The document a stored or synced name refers to, if it is one this version knows.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "core" => Some(Self::Core),
            "devices" => Some(Self::Devices),
            _ => name.strip_prefix("blocks-")?.parse().ok().map(Self::Blocks),
        }
    }

    /// The root collections a document of this kind holds, in a fixed order.
    ///
    /// Fixed because [`Doc::new`] writes them in a genesis change that must come out
    /// bit-identical on every device; see there for why.
    fn collections(self) -> &'static [&'static str] {
        match self {
            Self::Core => {
                &["tasks", "completions", "projects", "labels", "filters", "reminders", "acks",
                  "settings"]
            }
            Self::Blocks(_) => &["series", "exceptions", "assignments"],
            Self::Devices => &["devices"],
        }
    }

    fn domain(self) -> Domain {
        match self {
            Self::Core => Domain::Core,
            Self::Blocks(_) => Domain::Blocks,
            Self::Devices => Domain::Devices,
        }
    }
}

/// Which kind of document a record belongs in.
///
/// A runtime check rather than a type-level one: encoding it in the type system would mean
/// a marker parameter on every record for a mistake only this crate can make, and
/// [`Doc::put`] catches it on the first write in any case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// The `core` document.
    Core,
    /// A `blocks-<year>` document.
    Blocks,
    /// The `devices` document.
    Devices,
}

/// One Automerge document.
///
/// Wraps `AutoCommit` rather than `Automerge` because every write here is a single logical
/// edit that should become a change immediately; there is no place in this design where a
/// caller wants to batch several unrelated edits into one atomic change and roll them back.
pub struct Doc {
    id: DocId,
    doc: AutoCommit,
}

impl Doc {
    /// A new document, with its root collections created in a **deterministic genesis
    /// change**.
    ///
    /// # Why the genesis has to be deterministic
    ///
    /// Automerge object identity comes from the operation that created the object, so two
    /// devices that independently create a map at the same key create *different* maps. On
    /// merge one wins and the other's entire contents become unreachable — not a conflict
    /// to resolve, a silent loss of everything written into the loser.
    ///
    /// That is not hypothetical here. Year documents are created on demand, so two
    /// devices that both schedule something in 2027 while offline each bring a
    /// `blocks-2027` into existence, and merging them would drop one device's blocks
    /// entirely.
    ///
    /// The fix is to make independent creations produce the *same* change: a fixed actor, a
    /// fixed timestamp, and the collections written in a fixed order. The genesis change is
    /// then byte-identical wherever it happens, hashes to the same value, and merging two
    /// independently created documents is a no-op that leaves one set of collections.
    ///
    /// Writes after the genesis use a random actor, as they must — a shared actor
    /// identifier across devices would break Automerge's causality tracking outright.
    #[must_use]
    pub fn new(id: DocId) -> Self {
        let mut doc = AutoCommit::new();
        doc.set_actor(ActorId::from(GENESIS_ACTOR));
        for name in id.collections() {
            doc.put_object(ROOT, *name, ObjType::Map)
                .expect("the root of a fresh document accepts a map");
        }
        doc.commit_with(CommitOptions::default().with_time(0).with_message("genesis"));
        doc.set_actor(ActorId::random());
        Self { id, doc }
    }

    /// Brings in `core`'s second deterministic change, if it is not already here.
    ///
    /// The genesis creates the root collections; this one creates the two things that have
    /// to exist exactly once in every store and so must never be created by a device on its
    /// own: the **Inbox**, under [`ProjectId::INBOX`], and the **`series_years`** map that
    /// [`Documents::recurring_years`] reads. It is built the same way as the genesis — fixed
    /// actor, fixed time, fixed content, and depending on nothing but the genesis — so it is
    /// byte-identical wherever it is made, and a store that applies it late gains the same
    /// change a new store starts with.
    ///
    /// Returns whether it was newly applied. Other documents have nothing to apply.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the change.
    pub fn ensure_schema(&mut self) -> Result<bool> {
        if self.id != DocId::Core {
            return Ok(false);
        }
        let (hash, bytes) = core_schema_change();
        if self.doc.get_change_by_hash(hash).is_some() {
            return Ok(false);
        }
        self.doc.load_incremental(bytes)?;
        Ok(true)
    }

    /// The highest stored-format version that has written this document, or zero if none
    /// has said: a document from before versions.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        match self.doc.get(ROOT, "schema") {
            Ok(Some((Value::Scalar(value), _))) => value.to_i64().and_then(|v| u32::try_from(v).ok()).unwrap_or(0),
            _ => 0,
        }
    }

    /// Records that this build's [`SCHEMA_VERSION`] has written the document, unless one as
    /// new or newer already has. A plain value at the root, so two devices writing it at once
    /// create nothing for merge to choose between; whichever wins, the next write by the
    /// newer device raises it again.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn stamp_version(&mut self) -> Result<()> {
        if self.schema_version() < SCHEMA_VERSION {
            self.doc.put(ROOT, "schema", i64::from(SCHEMA_VERSION))?;
        }
        Ok(())
    }

    /// The next Automerge sync message for a peer, if there is anything to say.
    ///
    /// `state` is what this side knows of the peer's view of this document; one is kept per
    /// peer and per document for the length of a sync session.
    pub fn sync_message(&mut self, state: &mut automerge::sync::State) -> Option<Vec<u8>> {
        use automerge::sync::SyncDoc;
        self.doc.sync().generate_sync_message(state).map(automerge::sync::Message::encode)
    }

    /// Takes in a peer's sync message.
    ///
    /// # Errors
    ///
    /// If the bytes are not a sync message, or carry changes Automerge refuses.
    pub fn receive_sync_message(
        &mut self,
        state: &mut automerge::sync::State,
        bytes: &[u8],
    ) -> Result<()> {
        use automerge::sync::SyncDoc;
        let message = automerge::sync::Message::decode(bytes).map_err(|e| {
            StoreError::Unreadable(format!("a peer sent a sync message that does not read: {e}"))
        })?;
        self.doc.sync().receive_sync_message(state, message)?;
        Ok(())
    }

    /// Whether the document holds a change.
    pub fn has_change(&mut self, hash: &ChangeHash) -> bool {
        self.doc.get_change_by_hash(hash).is_some()
    }

    /// Loads a document from a saved snapshot.
    ///
    /// # Errors
    ///
    /// If the bytes are not a valid Automerge document.
    pub fn load(id: DocId, bytes: &[u8]) -> Result<Self> {
        Ok(Self { id, doc: AutoCommit::load(bytes)? })
    }

    /// Which document this is.
    #[must_use]
    pub fn id(&self) -> DocId {
        self.id
    }

    /// The whole document, compacted. This is `save()`, the form stored in `snapshots`
    /// and used for backups — self-describing, and carrying full history.
    pub fn save(&mut self) -> Vec<u8> {
        self.doc.save()
    }

    /// The changes made since the last call, for appending to the `changes` table.
    pub fn save_incremental(&mut self) -> Vec<u8> {
        self.doc.save_incremental()
    }

    /// Applies change chunks, from storage or from a peer.
    ///
    /// # Errors
    ///
    /// If the chunks are malformed.
    pub fn load_incremental(&mut self, bytes: &[u8]) -> Result<()> {
        self.doc.load_incremental(bytes)?;
        Ok(())
    }

    /// Merges another replica of the same document.
    ///
    /// # Errors
    ///
    /// If the documents cannot be merged.
    pub fn merge(&mut self, other: &mut Self) -> Result<()> {
        self.doc.merge(&mut other.doc)?;
        Ok(())
    }

    /// The individual changes made since `heads`, each with its hash.
    ///
    /// One row per change is what the `changes` table is shaped for, and it is what makes
    /// the table append-only: a change is immutable and identified by its hash, so writing
    /// one twice is a no-op rather than a conflict.
    pub fn changes_since(&mut self, heads: &[ChangeHash]) -> Vec<(ChangeHash, Vec<u8>)> {
        self.doc
            .get_changes(heads)
            .into_iter()
            .map(|change| (change.hash(), change.raw_bytes().to_vec()))
            .collect()
    }

    /// The current heads, against which a read model would detect its own staleness.
    pub fn heads(&mut self) -> Vec<ChangeHash> {
        self.doc.get_heads()
    }

    /// A fork with its own actor, for tests and for simulating a second device.
    #[must_use]
    pub fn fork(&mut self) -> Self {
        let mut doc = self.doc.fork();
        doc.set_actor(automerge::ActorId::random());
        Self { id: self.id, doc }
    }

    fn check_domain(&self, domain: Domain) -> Result<()> {
        if self.id.domain() == domain {
            return Ok(());
        }
        Err(StoreError::MalformedDocument {
            doc: self.id.name(),
            detail: format!("does not hold {domain:?} records"),
        })
    }

    /// The collection map at the root, created if this is the first write to it.
    fn collection_for_write(&mut self, name: &str) -> Result<ObjId> {
        Ok(match self.doc.get(ROOT, name)? {
            Some((Value::Object(ObjType::Map), id)) => id,
            _ => self.doc.put_object(ROOT, name, ObjType::Map)?,
        })
    }

    /// The collection map at the root, if it exists and is a map.
    fn collection_for_read(&self, name: &str) -> Option<ObjId> {
        match self.doc.get(ROOT, name) {
            Ok(Some((Value::Object(ObjType::Map), id))) => Some(id),
            _ => None,
        }
    }

    /// Writes a record, touching only the fields that differ from `before`.
    ///
    /// Pass `None` for `before` when creating. Pass the version the user actually edited
    /// when updating — **not** a freshly read one. The difference is the whole point: only
    /// the fields the user changed get written, so a field someone else changed on another
    /// device survives instead of being reverted by a write that had nothing to say about
    /// it.
    ///
    /// This also gives undo for free, since `put(before, Some(after))` is the exact
    /// inverse of `put(after, Some(before))`.
    ///
    /// # Errors
    ///
    /// If the record does not belong in this document, or Automerge refuses the write.
    pub(crate) fn put<R: Record>(&mut self, record: &R, before: Option<&R>) -> Result<()> {
        self.check_domain(R::DOMAIN)?;
        let collection = self.collection_for_write(R::COLLECTION)?;
        let key = R::key_string(&record.key());
        if R::INLINE {
            // A record written before it was inline is a map at its bare key. Its fields are
            // copied into the inline form as they stand — fields and values this build does
            // not know included — and the map goes, so there is never more than one copy to
            // read. Then only what the edit changed is written over them.
            if let Some((Value::Object(ObjType::Map), old)) = self.doc.get(&collection, key.as_str())? {
                let raw = Raw::read_map(&self.doc, &old);
                raw.write_inline(&mut self.doc, &collection, &format!("{key}/"))?;
                self.doc.delete(&collection, key.as_str())?;
            }
            let mut writer = Writer::inline(&mut self.doc, collection, format!("{key}/"));
            return record.write(&mut writer, before);
        }
        let obj = match self.doc.get(&collection, key.as_str())? {
            Some((Value::Object(ObjType::Map), id)) => id,
            _ => self.doc.put_object(&collection, key.as_str(), ObjType::Map)?,
        };
        let mut writer = Writer::new(&mut self.doc, obj);
        record.write(&mut writer, before)
    }

    /// A record's fields as they stand, known or not, or `None` if it is not here or is
    /// inline.
    pub(crate) fn raw<R: Record>(&self, key: &R::Key) -> Option<Raw> {
        let collection = self.collection_for_read(R::COLLECTION)?;
        match self.doc.get(&collection, R::key_string(key).as_str()).ok()?? {
            (Value::Object(ObjType::Map), obj) => Some(Raw::read_map(&self.doc, &obj)),
            _ => None,
        }
    }

    /// Writes a record copied from elsewhere exactly as it was, then `record` over it with
    /// only what differs from `before` — so a move keeps what this build cannot read.
    ///
    /// # Errors
    ///
    /// If the record does not belong in this document, or Automerge refuses the write.
    pub(crate) fn put_moved<R: Record>(&mut self, raw: &Raw, record: &R, before: &R) -> Result<()> {
        self.check_domain(R::DOMAIN)?;
        let collection = self.collection_for_write(R::COLLECTION)?;
        raw.write_at(&mut self.doc, &collection, &R::key_string(&record.key()))?;
        self.put(record, Some(before))
    }

    /// Removes a record outright.
    ///
    /// This is *purge*, not delete. Trash and undo are `deleted_at` on the record itself,
    /// which syncs and can be reversed; this is what emptying the trash does, and
    /// nothing else should call it.
    ///
    /// # Errors
    ///
    /// If the record does not belong in this document, or Automerge refuses the write.
    pub(crate) fn purge<R: Record>(&mut self, key: &R::Key) -> Result<()> {
        self.check_domain(R::DOMAIN)?;
        if let Some(collection) = self.collection_for_read(R::COLLECTION) {
            let key = R::key_string(key);
            if self.doc.get(&collection, key.as_str())?.is_some() {
                self.doc.delete(&collection, key.as_str())?;
            }
            if R::INLINE {
                let fields = format!("{key}/");
                let owned: Vec<String> =
                    self.doc.keys(&collection).filter(|k| k.starts_with(&fields)).collect();
                for field in owned {
                    self.doc.delete(&collection, field.as_str())?;
                }
            }
        }
        Ok(())
    }

    /// Reads every record of one kind, skipping any that cannot be understood.
    fn hydrate<R: Record>(&self, skipped: &mut Vec<Skipped>) -> BTreeMap<R::Key, R> {
        let Some(collection) = self.collection_for_read(R::COLLECTION) else {
            return BTreeMap::new();
        };
        let reader = Reader::new(&self.doc, collection.clone());
        // An inline record is every key `<record>/...`; a map at a bare key is either an
        // ordinary record or an inline one written before it was inline. Where both exist the
        // inline form is the newer, since writing it deletes the map.
        let mut keys: Vec<(String, bool)> = Vec::new();
        let mut inline = std::collections::BTreeSet::new();
        for key in reader.keys() {
            match key.split_once('/') {
                Some((record, _)) if R::INLINE => {
                    if inline.insert(record.to_owned()) {
                        keys.push((record.to_owned(), true));
                    }
                }
                _ => keys.push((key, false)),
            }
        }
        keys.retain(|(key, is_inline)| *is_inline || !inline.contains(key));

        let mut out = BTreeMap::new();
        for (key, is_inline) in keys {
            let fields = if is_inline {
                Some(Reader::inline(&self.doc, collection.clone(), format!("{key}/")))
            } else {
                reader.map(&key)
            };
            let record = R::parse_key(&key)
                .zip(fields)
                .and_then(|(parsed, fields)| R::hydrate(&fields, parsed));
            match record {
                Some(record) => {
                    out.insert(record.key(), record);
                }
                None => skipped.push(Skipped {
                    doc: self.id,
                    collection: R::COLLECTION,
                    key: key.clone(),
                }),
            }
        }
        out
    }
}

/// A record that could not be read, and was left out rather than failing the load.
///
/// Merge can produce documents that make no sense, and loading must tolerate them. But
/// tolerating is not the same as hiding: a task that silently fails to appear is
/// indistinguishable from a task that was deleted, and the user deserves to know which
/// happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// Which document it was in.
    pub doc: DocId,
    /// Which collection.
    pub collection: &'static str,
    /// Its key, as stored.
    pub key: String,
}

/// What hydrating a set of documents had to leave out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HydrationReport {
    /// Records that could not be read.
    pub skipped: Vec<Skipped>,
}

impl HydrationReport {
    /// Whether everything loaded.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.skipped.is_empty()
    }
}

/// The documents currently loaded, and the API for changing them.
///
/// `core` and `devices` are always present. Year documents appear as they are opened, which
/// is what lazy loading means in practice — a watch showing today holds one year, not
/// five.
pub struct Documents {
    core: Doc,
    devices: Doc,
    blocks: BTreeMap<i16, Doc>,
}

impl Default for Documents {
    fn default() -> Self {
        Self::new()
    }
}

impl Documents {
    /// An empty store: `core` and `devices`, no years yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: Doc::new(DocId::Core),
            devices: Doc::new(DocId::Devices),
            blocks: BTreeMap::new(),
        }
    }

    /// The `core` document.
    pub fn core(&mut self) -> &mut Doc {
        &mut self.core
    }

    /// The `devices` document.
    pub fn devices(&mut self) -> &mut Doc {
        &mut self.devices
    }

    /// Any document by name, creating a year's from its genesis if it is not here yet.
    pub fn get_mut(&mut self, id: DocId) -> &mut Doc {
        match id {
            DocId::Core => &mut self.core,
            DocId::Devices => &mut self.devices,
            DocId::Blocks(year) => self.blocks(year),
        }
    }

    /// One year of blocks, opening an empty one if that year is not loaded.
    pub fn blocks(&mut self, year: i16) -> &mut Doc {
        self.blocks.entry(year).or_insert_with(|| Doc::new(DocId::Blocks(year)))
    }

    /// A copy of every loaded document, as an independent replica.
    ///
    /// Each fork gets its own actor, so the result behaves as a second device that has
    /// already synced everything this one has — which is exactly what pairing produces,
    /// and what a convergence test needs as its starting point.
    pub fn fork(&mut self) -> Self {
        let mut blocks = BTreeMap::new();
        for (year, doc) in &mut self.blocks {
            blocks.insert(*year, doc.fork());
        }
        Self { core: self.core.fork(), devices: self.devices.fork(), blocks }
    }

    /// Merges another replica of the same store, document by document.
    ///
    /// # Errors
    ///
    /// If any document refuses the merge.
    pub fn merge(&mut self, other: &mut Self) -> Result<()> {
        self.core.merge(&mut other.core)?;
        self.devices.merge(&mut other.devices)?;
        for (year, doc) in &mut other.blocks {
            self.blocks
                .entry(*year)
                .or_insert_with(|| Doc::new(DocId::Blocks(*year)))
                .merge(doc)?;
        }
        Ok(())
    }

    /// Adopts an already-loaded document.
    pub fn insert(&mut self, doc: Doc) {
        match doc.id() {
            DocId::Core => self.core = doc,
            DocId::Devices => self.devices = doc,
            DocId::Blocks(year) => {
                self.blocks.insert(year, doc);
            }
        }
    }

    /// Every loaded document.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Doc> {
        std::iter::once(&mut self.core)
            .chain(std::iter::once(&mut self.devices))
            .chain(self.blocks.values_mut())
    }

    /// Whether a year is already loaded.
    #[must_use]
    pub fn has_year(&self, year: i16) -> bool {
        self.blocks.contains_key(&year)
    }

    /// Which years are loaded.
    #[must_use]
    pub fn loaded_years(&self) -> Vec<i16> {
        self.blocks.keys().copied().collect()
    }

    /// Materializes everything loaded into the view core queries over.
    ///
    /// The result is **not repaired**. Call [`Snapshot::repair`] before walking the tree or
    /// evaluating dependencies; it is separate because the caller may want to report what
    /// it changed, and because a snapshot taken purely to read one field does not need it.
    #[must_use]
    pub fn snapshot(&self) -> (Snapshot, HydrationReport) {
        let mut report = HydrationReport::default();
        let s = &mut report.skipped;

        let mut snapshot = Snapshot {
            tasks: self.core.hydrate::<Task>(s),
            completions: self.core.hydrate::<TaskCompletion>(s),
            projects: self.core.hydrate::<Project>(s),
            labels: self.core.hydrate::<Label>(s),
            saved_filters: self.core.hydrate::<SavedFilter>(s),
            reminders: self.core.hydrate::<Reminder>(s),
            acks: self.core.hydrate::<ReminderAck>(s),
            devices: self.devices.hydrate::<Device>(s),
            settings: self.core.settings(),
            ..Snapshot::default()
        };

        for doc in self.blocks.values() {
            snapshot.series.extend(doc.hydrate::<BlockSeries>(s));
            snapshot.exceptions.extend(doc.hydrate::<BlockException>(s));
            snapshot.assignments.extend(doc.hydrate::<BlockAssignment>(s));
        }
        (snapshot, report)
    }
}

impl Doc {
    /// Reads the settings singleton, falling back to defaults for anything unreadable.
    #[must_use]
    pub fn settings(&self) -> Settings {
        self.collection_for_read("settings")
            .map(|obj| read_settings(&Reader::new(&self.doc, obj)))
            .unwrap_or_default()
    }

    /// Writes the settings that differ from `before`.
    ///
    /// # Errors
    ///
    /// If this is not the `core` document, or Automerge refuses the write.
    pub fn put_settings(&mut self, now: &Settings, before: Option<&Settings>) -> Result<()> {
        self.check_domain(Domain::Core)?;
        let obj = self.collection_for_write("settings")?;
        let mut writer = Writer::new(&mut self.doc, obj);
        write_settings(&mut writer, before, now)
    }
}

/// Generates the typed write methods, one pair per record type.
///
/// Typed rather than a public generic `put::<Task>`: the shard a record lands in is not
/// something a caller should have to know, and stating it in the method's own documentation
/// is worth more than the lines this saves.
macro_rules! core_methods {
    ($($put:ident, $purge:ident, $ty:ty, $key:ty, $what:literal;)*) => {
        impl Documents {
            $(
                #[doc = concat!("Creates or updates ", $what, " in `core`.")]
                ///
                /// Pass the version the user edited as `before`, or `None` to create. See
                /// [`Doc::put`] for why that distinction matters.
                ///
                /// # Errors
                ///
                /// If Automerge refuses the write.
                pub fn $put(&mut self, record: &$ty, before: Option<&$ty>) -> Result<()> {
                    self.core.put(record, before)
                }

                #[doc = concat!("Permanently removes ", $what, ". This is emptying the trash, not deleting.")]
                ///
                /// # Errors
                ///
                /// If Automerge refuses the write.
                pub fn $purge(&mut self, key: &$key) -> Result<()> {
                    self.core.purge::<$ty>(key)
                }
            )*
        }
    };
}

use lumenna_core::id::{CompletionId, FilterId, LabelId, ReminderId, TaskId};

core_methods! {
    put_task, purge_task, Task, TaskId, "a task";
    put_completion, purge_completion, TaskCompletion, CompletionId, "a completion";
    put_project, purge_project, Project, ProjectId, "a project";
    put_label, purge_label, Label, LabelId, "a label";
    put_filter, purge_filter, SavedFilter, FilterId, "a saved filter";
    put_reminder, purge_reminder, Reminder, ReminderId, "a reminder";
}

impl Documents {
    /// Creates or updates a reminder acknowledgement in `core`.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn put_ack(&mut self, ack: &ReminderAck, before: Option<&ReminderAck>) -> Result<()> {
        self.core.put(ack, before)
    }

    /// Creates or updates a device in the roster.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn put_device(&mut self, device: &Device, before: Option<&Device>) -> Result<()> {
        self.devices.put(device, before)
    }

    /// Creates or updates a block series, in the year it starts.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn put_series(&mut self, s: &BlockSeries, before: Option<&BlockSeries>) -> Result<()> {
        let year = s.start_date.year();
        if s.is_recurring() {
            self.note_recurring_year(year)?;
        }
        match before {
            // A series lives in the year it starts, so a new start year is a new document.
            // Writing only the changed fields there would leave a record missing everything
            // else, so it is written whole and removed from the old year. The old year has to
            // be loaded for that removal to reach the disk, which is why anything editing by
            // identifier loads every year.
            Some(was) if was.start_date.year() != year => {
                let from = was.start_date.year();
                let raw = self.blocks(from).raw::<BlockSeries>(&was.id);
                self.blocks(from).purge::<BlockSeries>(&was.id)?;
                match raw {
                    Some(raw) => self.blocks(year).put_moved(&raw, s, was),
                    None => self.blocks(year).put(s, None),
                }
            }
            _ => self.blocks(year).put(s, before),
        }
    }

    /// Ensures `core` is at its current schema. See [`Doc::ensure_schema`].
    ///
    /// # Errors
    ///
    /// If Automerge refuses the change.
    pub fn ensure_schema(&mut self) -> Result<bool> {
        self.core.ensure_schema()
    }

    /// The years whose document holds at least one recurring series.
    ///
    /// A series lives in the year it starts but recurs into every year after, so
    /// showing a day means loading its own year **and** each of these before it. One-off
    /// blocks never need that, which is why this is an index of recurring years rather than
    /// a reason to load every year there is.
    #[must_use]
    pub fn recurring_years(&self) -> Vec<i16> {
        let Some(obj) = self.core.collection_for_read(SERIES_YEARS) else {
            return Vec::new();
        };
        let reader = Reader::new(&self.core.doc, obj);
        reader
            .keys()
            .into_iter()
            .filter(|k| reader.bool(k) == Some(true))
            .filter_map(|k| k.parse().ok())
            .collect()
    }

    /// Records that `year` holds a recurring series. Only ever adds, and writes nothing
    /// when the year is already there.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn note_recurring_year(&mut self, year: i16) -> Result<()> {
        self.core.ensure_schema()?;
        let Some(obj) = self.core.collection_for_read(SERIES_YEARS) else {
            return Ok(());
        };
        let key = year.to_string();
        if Reader::new(&self.core.doc, obj.clone()).bool(&key) != Some(true) {
            self.core.doc.put(&obj, key.as_str(), true)?;
        }
        Ok(())
    }

    /// Creates or updates an exception, in the year of the occurrence it modifies.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn put_exception(
        &mut self,
        e: &BlockException,
        before: Option<&BlockException>,
    ) -> Result<()> {
        self.blocks(e.original_date.year()).put(e, before)
    }

    /// Creates or updates an assignment, in the year of the block occurrence it is for.
    ///
    /// **`year` is explicit, and has to be.** An assignment to a recurring block names its
    /// occurrence date and could be sharded from that, but
    /// [`BlockRef::OneOff`](lumenna_core::model::BlockRef::OneOff) carries only the series
    /// identifier — deliberately, so that moving a one-off block carries its assignments
    /// with it. The consequence is that the store cannot derive the year without
    /// reading the series, which may live in a document that is not loaded. The caller
    /// knows the block; the store does not.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn put_assignment(
        &mut self,
        year: i16,
        a: &BlockAssignment,
        before: Option<&BlockAssignment>,
    ) -> Result<()> {
        self.blocks(year).put(a, before)
    }

    /// Writes the settings that differ from `before`.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn put_settings(&mut self, now: &Settings, before: Option<&Settings>) -> Result<()> {
        self.core.put_settings(now, before)
    }

    /// Permanently removes a reminder acknowledgement.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn purge_ack(&mut self, key: &(lumenna_core::id::ReminderId, civil::Date)) -> Result<()> {
        self.core.purge::<ReminderAck>(key)
    }

    /// Permanently removes a device from the roster. This is unpairing.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn purge_device(&mut self, key: &lumenna_core::id::NodeId) -> Result<()> {
        self.devices.purge::<Device>(key)
    }

    /// Permanently removes a block series from one year.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn purge_series(&mut self, year: i16, key: &lumenna_core::id::SeriesId) -> Result<()> {
        self.blocks(year).purge::<BlockSeries>(key)
    }

    /// Permanently removes an exception, restoring the occurrence the rule would produce.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn purge_exception(
        &mut self,
        key: &(lumenna_core::id::SeriesId, civil::Date),
    ) -> Result<()> {
        self.blocks(key.1.year()).purge::<BlockException>(key)
    }

    /// Permanently removes an assignment.
    ///
    /// # Errors
    ///
    /// If Automerge refuses the write.
    pub fn purge_assignment(
        &mut self,
        year: i16,
        key: &lumenna_core::id::AssignmentId,
    ) -> Result<()> {
        self.blocks(year).purge::<BlockAssignment>(key)
    }

    /// Applies an [`Edit`] computed by [`lumenna_core::edit`].
    ///
    /// The whole edit or none of it, as far as the in-memory documents are concerned: an
    /// error part-way leaves earlier changes applied, which Automerge has already recorded
    /// as operations and which `Store::write` will persist. That is the honest outcome —
    /// pretending otherwise would mean a rollback mechanism the CRDT does not have — and it
    /// is why the operations in `core::edit` are written so that every change in one edit is
    /// independently valid.
    ///
    /// # Errors
    ///
    /// If any record does not belong in its document, or Automerge refuses a write.
    pub fn apply(&mut self, edit: &Edit) -> Result<()> {
        for change in &edit.changes {
            self.apply_change(change)?;
        }
        Ok(())
    }

    fn apply_change(&mut self, change: &Change) -> Result<()> {
        match change {
            Change::Task(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_task(after, before.as_ref()),
                (Some(before), None) => self.purge_task(&before.id),
                (None, None) => Ok(()),
            },
            Change::Completion(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_completion(after, before.as_ref()),
                (Some(before), None) => self.purge_completion(&before.id),
                (None, None) => Ok(()),
            },
            Change::Project(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_project(after, before.as_ref()),
                (Some(before), None) => self.purge_project(&before.id),
                (None, None) => Ok(()),
            },
            Change::Label(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_label(after, before.as_ref()),
                (Some(before), None) => self.purge_label(&before.id),
                (None, None) => Ok(()),
            },
            Change::Filter(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_filter(after, before.as_ref()),
                (Some(before), None) => self.purge_filter(&before.id),
                (None, None) => Ok(()),
            },
            Change::Reminder(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_reminder(after, before.as_ref()),
                (Some(before), None) => self.purge_reminder(&before.id),
                (None, None) => Ok(()),
            },
            Change::Ack(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_ack(after, before.as_ref()),
                (Some(before), None) => {
                    self.purge_ack(&(before.reminder_id, before.occurrence_date))
                }
                (None, None) => Ok(()),
            },
            Change::Series(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_series(after, before.as_ref()),
                (Some(before), None) => {
                    self.purge_series(before.start_date.year(), &before.id)
                }
                (None, None) => Ok(()),
            },
            Change::Exception(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_exception(after, before.as_ref()),
                (Some(before), None) => {
                    self.purge_exception(&(before.series_id, before.original_date))
                }
                (None, None) => Ok(()),
            },
            Change::Assignment { year, transition } => {
                match (&transition.before, &transition.after) {
                    (before, Some(after)) => self.put_assignment(*year, after, before.as_ref()),
                    (Some(before), None) => self.purge_assignment(*year, &before.id),
                    (None, None) => Ok(()),
                }
            }
            Change::Device(t) => match (&t.before, &t.after) {
                (before, Some(after)) => self.put_device(after, before.as_ref()),
                (Some(before), None) => self.purge_device(&before.node_id),
                (None, None) => Ok(()),
            },
            Change::Settings { before, after } => self.put_settings(after, Some(before)),
        }
    }
}

/// The occurrence date an assignment is for, when it has one.
#[must_use]
pub fn assignment_year(a: &BlockAssignment, series: Option<&BlockSeries>) -> Option<i16> {
    a.block_ref
        .date()
        .map(civil::Date::year)
        .or_else(|| series.map(|s| s.start_date.year()))
}

/// A value as it stands in a document, known to this build or not, to be copied somewhere
/// else whole. Builds of different versions sync for good, so a field one does not know is
/// still another's data, and moving a record must not drop it.
#[derive(Debug, Clone)]
pub(crate) enum Raw {
    Scalar(automerge::ScalarValue),
    Map(Vec<(String, Raw)>),
    List(Vec<Raw>),
    Text(String),
}

impl Raw {
    fn read_map(doc: &AutoCommit, obj: &ObjId) -> Self {
        Self::Map(
            doc.keys(obj)
                .filter_map(|key| doc.get(obj, key.as_str()).ok().flatten().map(|v| (key, v)))
                .map(|(key, (value, id))| (key, Self::read(doc, value, &id)))
                .collect(),
        )
    }

    fn read(doc: &AutoCommit, value: Value<'_>, id: &ObjId) -> Self {
        match value {
            Value::Scalar(scalar) => Self::Scalar(scalar.into_owned()),
            Value::Object(ObjType::Text) => Self::Text(doc.text(id).unwrap_or_default()),
            Value::Object(ObjType::List) => Self::List(
                (0..doc.length(id))
                    .filter_map(|i| doc.get(id, i).ok().flatten())
                    .map(|(value, inner)| Self::read(doc, value, &inner))
                    .collect(),
            ),
            Value::Object(_) => Self::read_map(doc, id),
        }
    }

    /// Writes this at `key` of `obj`: a map becomes a map, and so on.
    fn write_at(&self, doc: &mut AutoCommit, obj: &ObjId, key: &str) -> Result<()> {
        match self {
            Self::Scalar(scalar) => {
                doc.put(obj, key, scalar.clone())?;
            }
            Self::Text(text) => {
                let id = doc.put_object(obj, key, ObjType::Text)?;
                doc.splice_text(&id, 0, 0, text)?;
            }
            Self::List(items) => {
                let id = doc.put_object(obj, key, ObjType::List)?;
                for (index, item) in items.iter().enumerate() {
                    item.insert_into(doc, &id, index)?;
                }
            }
            Self::Map(fields) => {
                let id = doc.put_object(obj, key, ObjType::Map)?;
                for (field, value) in fields {
                    value.write_at(doc, &id, field)?;
                }
            }
        }
        Ok(())
    }

    fn insert_into(&self, doc: &mut AutoCommit, list: &ObjId, index: usize) -> Result<()> {
        match self {
            Self::Scalar(scalar) => {
                doc.insert(list, index, scalar.clone())?;
            }
            Self::Text(text) => {
                let id = doc.insert_object(list, index, ObjType::Text)?;
                doc.splice_text(&id, 0, 0, text)?;
            }
            Self::List(items) => {
                let id = doc.insert_object(list, index, ObjType::List)?;
                for (i, item) in items.iter().enumerate() {
                    item.insert_into(doc, &id, i)?;
                }
            }
            Self::Map(fields) => {
                let id = doc.insert_object(list, index, ObjType::Map)?;
                for (field, value) in fields {
                    value.write_at(doc, &id, field)?;
                }
            }
        }
        Ok(())
    }

    /// Writes a map's fields as keys of `collection` starting with `prefix`, a nested map
    /// as a longer prefix, as inline records are stored. Text is written as a plain string;
    /// a list, which the inline form cannot hold, is left behind.
    fn write_inline(&self, doc: &mut AutoCommit, collection: &ObjId, prefix: &str) -> Result<()> {
        let Self::Map(fields) = self else { return Ok(()) };
        for (field, value) in fields {
            let key = format!("{prefix}{field}");
            match value {
                Self::Scalar(scalar) => {
                    doc.put(collection, key.as_str(), scalar.clone())?;
                }
                Self::Text(text) => {
                    doc.put(collection, key.as_str(), text.as_str())?;
                }
                Self::Map(_) => value.write_inline(doc, collection, &format!("{key}."))?,
                Self::List(_) => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod unknown_fields {
    //! A build syncs with older and newer ones for good, so what it cannot read it must not
    //! drop when it writes.

    use automerge::transaction::Transactable;
    use automerge::{ObjType, ROOT, ReadDoc, Value};
    use jiff::civil::{date, time};
    use lumenna_core::id::NodeId;
    use lumenna_core::model::{BlockKind, BlockSeries, Device, Project, Task};
    use lumenna_core::order::OrderKey;

    use super::{Doc, Documents};
    use crate::records::Record;

    /// The map a non-inline record lives in.
    fn record_map<R: Record>(doc: &Doc, key: &R::Key) -> automerge::ObjId {
        let (_, collection) = doc.doc.get(ROOT, R::COLLECTION).unwrap().unwrap();
        let (_, obj) = doc.doc.get(&collection, R::key_string(key).as_str()).unwrap().unwrap();
        obj
    }

    fn string_at(doc: &Doc, obj: &automerge::ObjId, key: &str) -> Option<String> {
        match doc.doc.get(obj, key).unwrap()? {
            (Value::Scalar(value), _) => value.to_str().map(str::to_owned),
            _ => None,
        }
    }

    #[test]
    fn an_edit_keeps_a_field_this_build_does_not_know() {
        let mut docs = Documents::new();
        let task = Task::new(Project::inbox().id, "essay", OrderKey::middle());
        docs.core().put(&task, None).unwrap();
        let obj = record_map::<Task>(&docs.core, &task.id);
        docs.core.doc.put(&obj, "written_by_a_newer_build", "keep me").unwrap();

        let edited = Task { title: "the essay".to_owned(), ..task.clone() };
        docs.core().put(&edited, Some(&task)).unwrap();
        assert_eq!(string_at(&docs.core, &obj, "written_by_a_newer_build").as_deref(), Some("keep me"));
    }

    #[test]
    fn a_series_moved_to_another_year_keeps_what_this_build_does_not_know() {
        let mut docs = Documents::new();
        let series = BlockSeries::one_off("Deep work", BlockKind::Work, date(2026, 12, 31), time(9, 0, 0, 0), 60).unwrap();
        docs.put_series(&series, None).unwrap();
        let obj = record_map::<BlockSeries>(docs.blocks(2026), &series.id);
        let year = docs.blocks(2026);
        year.doc.put(&obj, "written_by_a_newer_build", "keep me").unwrap();
        // A kind this build reads as work, which writing back must not turn into one.
        year.doc.put(&obj, "kind", "focus").unwrap();

        let moved = BlockSeries { start_date: date(2027, 1, 4), ..series.clone() };
        docs.put_series(&moved, Some(&series)).unwrap();
        let obj = record_map::<BlockSeries>(docs.blocks(2027), &series.id);
        let next = docs.blocks(2027);
        assert_eq!(string_at(next, &obj, "written_by_a_newer_build").as_deref(), Some("keep me"));
        assert_eq!(string_at(next, &obj, "kind").as_deref(), Some("focus"));
        assert_eq!(string_at(next, &obj, "start_date").as_deref(), Some("2027-01-04"));
    }

    #[test]
    fn a_record_made_inline_keeps_what_this_build_does_not_know() {
        let mut docs = Documents::new();
        let device = Device {
            node_id: NodeId::from_bytes([7; 32]),
            name: "Laptop".to_owned(),
            platform: "linux".to_owned(),
            paired_at: lumenna_core::time::now(),
            last_seen: lumenna_core::time::now(),
            schema: 1,
        };
        // Written as a map, as before the device list was inline, with a field from elsewhere.
        let devices = docs.devices();
        let (_, collection) = devices.doc.get(ROOT, Device::COLLECTION).unwrap().unwrap();
        let key = Device::key_string(&device.node_id);
        let obj = devices.doc.put_object(&collection, key.as_str(), ObjType::Map).unwrap();
        devices.doc.put(&obj, "name", "Laptop").unwrap();
        devices.doc.put(&obj, "written_by_a_newer_build", "keep me").unwrap();

        let renamed = Device { name: "Work laptop".to_owned(), ..device.clone() };
        docs.put_device(&renamed, Some(&device)).unwrap();
        let devices = docs.devices();
        let kept = format!("{key}/written_by_a_newer_build");
        assert!(matches!(devices.doc.get(&collection, kept.as_str()).unwrap(), Some((Value::Scalar(_), _))));
        assert_eq!(string_at(devices, &collection, &format!("{key}/name")).as_deref(), Some("Work laptop"));
    }

    #[test]
    fn a_document_says_which_version_wrote_it_and_never_goes_down() {
        let mut doc = Doc::new(super::DocId::Core);
        assert_eq!(doc.schema_version(), 0, "a document from before versions says none");
        doc.stamp_version().unwrap();
        assert_eq!(doc.schema_version(), lumenna_core::model::SCHEMA_VERSION);
        // A newer build wrote it: this one leaves that alone.
        doc.doc.put(ROOT, "schema", i64::from(lumenna_core::model::SCHEMA_VERSION) + 1).unwrap();
        doc.stamp_version().unwrap();
        assert_eq!(doc.schema_version(), lumenna_core::model::SCHEMA_VERSION + 1);
    }
}
