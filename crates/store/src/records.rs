//! How each record of §3 is laid out in an Automerge document.
//!
//! One [`Record`] implementation per type, and each reads as the list of its fields. The
//! mechanics — tolerant reads, difference-only writes, sets as maps, text as text — live in
//! [`crate::value`]; what is here is the schema.
//!
//! # Conventions
//!
//! - Every collection is a **map keyed by identifier**, never a list. Records have no
//!   inherent order (§3.13 puts ordering in a fractional index instead), and a list would
//!   make concurrent creation a merge problem for nothing.
//! - Enums are stored as **lowercase strings**, not ordinals. An ordinal is one reordered
//!   variant away from silently reinterpreting every stored record, and there is no version
//!   of this app where that failure is worth the bytes saved.
//! - An unrecognised enum string reads as the type's default rather than failing the record,
//!   which is what lets a future version add a variant without older clients refusing to
//!   load documents that use it.

use automerge::ReadDoc;
use automerge::transaction::Transactable;
use jiff::civil;
use lumenna_core::id::{
    AssignmentId, CompletionId, FilterId, LabelId, NodeId, ProjectId, ReminderId, SeriesId,
    TaskId,
};
use lumenna_core::model::{
    AssignmentStatus, BlockAssignment, BlockException, BlockFlags, BlockKind, BlockRef,
    BlockSeries, Delivery, Device, Due, ExceptionAction, ExternalProvider, ExternalRef, Label,
    Priority, Project, Recurrence, Reminder, ReminderAck, ReminderAction, ReminderAnchor,
    ReminderTarget, SavedFilter, Settings, Task, TaskCompletion, Trigger, TzName, Verbosity,
};

use crate::doc::Domain;
use crate::error::Result;
use crate::value::{Reader, Writer, date_key};

/// A type that lives in a keyed collection at the root of a document.
pub(crate) trait Record: Sized {
    /// Which of §3.1's documents this record lives in.
    const DOMAIN: Domain;

    /// The root key holding this kind of record.
    const COLLECTION: &'static str;

    /// Whether the record is stored inline — a key per field in the collection, rather than
    /// a map of its own (see [`crate::value`]).
    ///
    /// Required of any record whose key two devices can produce independently. A record keyed
    /// by a fresh UUID is only ever created once, so a map is safe and is the default.
    const INLINE: bool = false;

    /// How records of this kind are addressed.
    type Key: Ord + Clone;

    /// This record's key.
    fn key(&self) -> Self::Key;

    /// The key as it appears in the document.
    fn key_string(key: &Self::Key) -> String;

    /// Reads a key back, or `None` if it is not one this version understands.
    fn parse_key(text: &str) -> Option<Self::Key>;

    /// Reads a record, or `None` if it is missing something it cannot do without.
    ///
    /// Returning `None` skips this one record and reports it (§3.1); it never fails the
    /// load. What counts as indispensable is deliberately narrow — a field the record
    /// cannot be displayed or placed without, such as a task's project or its position.
    /// Everything else falls back to a default, because a task that shows up with the wrong
    /// colour is better than a task that does not show up.
    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, key: Self::Key) -> Option<Self>;

    /// Writes the fields that differ from `before`, or all of them if there is no `before`.
    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, before: Option<&Self>)
    -> Result<()>;
}

// ---------------------------------------------------------------------------------------
// Enums, as strings.
// ---------------------------------------------------------------------------------------

fn block_kind(text: Option<String>) -> BlockKind {
    match text.as_deref() {
        Some("break") => BlockKind::Break,
        Some("event") => BlockKind::Event,
        _ => BlockKind::Work,
    }
}

fn block_kind_str(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Work => "work",
        BlockKind::Break => "break",
        BlockKind::Event => "event",
    }
}

fn status(text: Option<String>) -> AssignmentStatus {
    match text.as_deref() {
        Some("in_progress") => AssignmentStatus::InProgress,
        Some("worked") => AssignmentStatus::Worked,
        Some("skipped") => AssignmentStatus::Skipped,
        Some("deferred") => AssignmentStatus::Deferred,
        _ => AssignmentStatus::Planned,
    }
}

fn status_str(status: AssignmentStatus) -> &'static str {
    match status {
        AssignmentStatus::Planned => "planned",
        AssignmentStatus::InProgress => "in_progress",
        AssignmentStatus::Worked => "worked",
        AssignmentStatus::Skipped => "skipped",
        AssignmentStatus::Deferred => "deferred",
    }
}

fn provider(text: Option<String>) -> ExternalProvider {
    match text.as_deref() {
        Some("caldav") => ExternalProvider::CalDav,
        Some("ics_url") => ExternalProvider::IcsUrl,
        _ => ExternalProvider::EventKit,
    }
}

fn provider_str(provider: ExternalProvider) -> &'static str {
    match provider {
        ExternalProvider::EventKit => "eventkit",
        ExternalProvider::CalDav => "caldav",
        ExternalProvider::IcsUrl => "ics_url",
    }
}

// ---------------------------------------------------------------------------------------
// Small composites, written wholesale (see Writer::set_nested).
// ---------------------------------------------------------------------------------------

fn read_due<D: ReadDoc>(r: &Reader<'_, D>) -> Option<Due> {
    Some(Due {
        date: r.parsed("date")?,
        time: r.parsed("time"),
        timezone: r.string("timezone").map(TzName::new),
        recurrence: r.map("recurrence").and_then(|rec| {
            Some(Recurrence {
                rrule: rec.string("rrule")?,
                from_completion: rec.bool("from_completion").unwrap_or(false),
            })
        }),
    })
}

fn write_due<T: Transactable>(w: &mut Writer<'_, T>, due: &Due) -> Result<()> {
    w.set_str("date", None, &due.date)?;
    w.set_opt_str("time", None, due.time.as_ref())?;
    w.set_opt_str("timezone", None, due.timezone.as_ref())?;
    if let Some(rec) = &due.recurrence {
        let mut nested = w.map("recurrence")?;
        nested.set_string("rrule", None, &rec.rrule)?;
        nested.set_bool("from_completion", None, rec.from_completion)?;
    }
    Ok(())
}

fn read_external<D: ReadDoc>(r: &Reader<'_, D>) -> Option<ExternalRef> {
    Some(ExternalRef {
        provider: provider(r.string("provider")),
        external_id: r.string("external_id")?,
        read_only: r.bool("read_only").unwrap_or(true),
        last_synced: r.timestamp("last_synced")?,
    })
}

fn write_external<T: Transactable>(w: &mut Writer<'_, T>, ext: &ExternalRef) -> Result<()> {
    w.set_string("provider", None, provider_str(ext.provider))?;
    w.set_string("external_id", None, &ext.external_id)?;
    w.set_bool("read_only", None, ext.read_only)?;
    w.set_time("last_synced", None, &ext.last_synced)
}

fn read_flags<D: ReadDoc>(r: &Reader<'_, D>, kind: BlockKind) -> BlockFlags {
    let default = kind.default_flags();
    BlockFlags {
        accepts_tasks: r.bool("accepts_tasks").unwrap_or(default.accepts_tasks),
        counts_capacity: r.bool("counts_capacity").unwrap_or(default.counts_capacity),
        anchored: r.bool("anchored").unwrap_or(default.anchored),
    }
}

fn write_flags<T: Transactable>(w: &mut Writer<'_, T>, flags: &BlockFlags) -> Result<()> {
    w.set_bool("accepts_tasks", None, flags.accepts_tasks)?;
    w.set_bool("counts_capacity", None, flags.counts_capacity)?;
    w.set_bool("anchored", None, flags.anchored)
}

fn read_trigger<D: ReadDoc>(r: &Reader<'_, D>) -> Option<Trigger> {
    let anchor = match r.string("anchor").as_deref() {
        Some("due") => ReminderAnchor::Due,
        Some("block_start") => ReminderAnchor::BlockStart,
        Some("block_end") => ReminderAnchor::BlockEnd,
        Some("absolute") => ReminderAnchor::Absolute(r.parsed("at")?),
        _ => return None,
    };
    let offset = i32::try_from(r.int("offset_mins").unwrap_or(0)).ok()?;
    Some(Trigger { anchor, offset_mins: offset })
}

fn write_trigger<T: Transactable>(w: &mut Writer<'_, T>, trigger: &Trigger) -> Result<()> {
    let name = match &trigger.anchor {
        ReminderAnchor::Due => "due",
        ReminderAnchor::BlockStart => "block_start",
        ReminderAnchor::BlockEnd => "block_end",
        ReminderAnchor::Absolute(at) => {
            w.set_str("at", None, at)?;
            "absolute"
        }
    };
    w.set_string("anchor", None, name)?;
    w.set_int("offset_mins", None, i64::from(trigger.offset_mins))
}

fn read_delivery<D: ReadDoc>(r: &Reader<'_, D>) -> Delivery {
    match r.string("mode").as_deref() {
        Some("only") => Delivery::OnlyDevices(r.id_set("devices")),
        Some("except") => Delivery::ExceptDevices(r.id_set("devices")),
        _ => Delivery::AllDevices,
    }
}

fn write_delivery<T: Transactable>(w: &mut Writer<'_, T>, delivery: &Delivery) -> Result<()> {
    let (mode, devices) = match delivery {
        Delivery::AllDevices => ("all", None),
        Delivery::OnlyDevices(set) => ("only", Some(set)),
        Delivery::ExceptDevices(set) => ("except", Some(set)),
    };
    w.set_string("mode", None, mode)?;
    match devices {
        Some(set) => w.set_members("devices", None, set),
        None => w.clear("devices"),
    }
}

fn read_block_ref<D: ReadDoc>(r: &Reader<'_, D>) -> Option<BlockRef> {
    let series: SeriesId = r.parsed("series")?;
    Some(match r.parsed::<civil::Date>("date") {
        Some(date) => BlockRef::Occurrence(series, date),
        None => BlockRef::OneOff(series),
    })
}

fn write_block_ref<T: Transactable>(w: &mut Writer<'_, T>, block: &BlockRef) -> Result<()> {
    w.set_str("series", None, &block.series_id())?;
    w.set_opt_str("date", None, block.date().as_ref())
}

/// Splits `<uuid>@<date>` — the key shape shared by exceptions and reminder
/// acknowledgements, both of which address one occurrence of a repeating thing.
fn split_occurrence_key(text: &str) -> Option<(&str, civil::Date)> {
    let (id, date) = text.split_once('@')?;
    Some((id, date.parse().ok()?))
}

// ---------------------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------------------

impl Record for Task {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "tasks";
    type Key = TaskId;

    fn key(&self) -> TaskId {
        self.id
    }

    fn key_string(key: &TaskId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<TaskId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: TaskId) -> Option<Self> {
        Some(Self {
            id,
            title: r.string("title").unwrap_or_default(),
            notes: r.text("notes"),
            parent_id: r.parsed("parent_id"),
            // A task with no project cannot be shown anywhere: Inbox is a real project
            // (§3.4), so this is never legitimately absent.
            project_id: r.parsed("project_id")?,
            priority: Priority::from_u8(r.u32("priority").unwrap_or(4).try_into().unwrap_or(4)),
            labels: r.id_set("labels"),
            depends: r.id_set("depends"),
            due: r.map("due").and_then(|due| read_due(&due)),
            estimate_mins: r.u32("estimate_mins"),
            // Position is not something this device may invent: a locally generated order
            // key would differ on every replica and reorder the list on each sync.
            order: r.parsed("order")?,
            external: r.map("external").and_then(|e| read_external(&e)),
            created_at: r.timestamp("created_at")?,
            deleted_at: r.timestamp("deleted_at"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_string("title", was.map(|b| b.title.as_str()), &self.title)?;
        w.set_text("notes", was.map(|b| b.notes.as_str()), &self.notes)?;
        w.set_opt_str("parent_id", was.map(|b| b.parent_id.as_ref()), self.parent_id.as_ref())?;
        w.set_str("project_id", was.map(|b| &b.project_id), &self.project_id)?;
        w.set_int(
            "priority",
            was.map(|b| i64::from(b.priority.as_u8())),
            i64::from(self.priority.as_u8()),
        )?;
        w.set_members("labels", was.map(|b| &b.labels), &self.labels)?;
        w.set_members("depends", was.map(|b| &b.depends), &self.depends)?;
        w.set_nested("due", was.map(|b| b.due.as_ref()), self.due.as_ref(), write_due)?;
        w.set_opt_u32("estimate_mins", was.map(|b| b.estimate_mins), self.estimate_mins)?;
        w.set_str("order", was.map(|b| &b.order), &self.order)?;
        w.set_nested(
            "external",
            was.map(|b| b.external.as_ref()),
            self.external.as_ref(),
            write_external,
        )?;
        w.set_time("created_at", was.map(|b| &b.created_at), &self.created_at)?;
        w.set_opt_time("deleted_at", was.map(|b| b.deleted_at.as_ref()), self.deleted_at.as_ref())
    }
}

// ---------------------------------------------------------------------------------------
// TaskCompletion
// ---------------------------------------------------------------------------------------

impl Record for TaskCompletion {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "completions";
    type Key = CompletionId;

    fn key(&self) -> CompletionId {
        self.id
    }

    fn key_string(key: &CompletionId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<CompletionId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: CompletionId) -> Option<Self> {
        Some(Self {
            id,
            task_id: r.parsed("task_id")?,
            completed_at: r.timestamp("completed_at")?,
            occurrence_date: r.parsed("occurrence_date"),
            cascaded_from: r.parsed("cascaded_from"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_str("task_id", was.map(|b| &b.task_id), &self.task_id)?;
        w.set_time("completed_at", was.map(|b| &b.completed_at), &self.completed_at)?;
        w.set_opt_str(
            "occurrence_date",
            was.map(|b| b.occurrence_date.as_ref()),
            self.occurrence_date.as_ref(),
        )?;
        w.set_opt_str(
            "cascaded_from",
            was.map(|b| b.cascaded_from.as_ref()),
            self.cascaded_from.as_ref(),
        )
    }
}

// ---------------------------------------------------------------------------------------
// Project, Label, SavedFilter
// ---------------------------------------------------------------------------------------

impl Record for Project {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "projects";
    type Key = ProjectId;

    fn key(&self) -> ProjectId {
        self.id
    }

    fn key_string(key: &ProjectId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<ProjectId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: ProjectId) -> Option<Self> {
        Some(Self {
            id,
            name: r.string("name").unwrap_or_default(),
            parent_id: r.parsed("parent_id"),
            color: r.string("color"),
            order: r.parsed("order")?,
            archived: r.bool("archived").unwrap_or(false),
            is_inbox: r.bool("is_inbox").unwrap_or(false),
            // `own_weight` is present exactly when the project sets one. Stores from before
            // it wrote `weight` on every project, 1.0 for "inherit", so there a 1.0 means
            // unset and anything else is the project's own.
            weight: r.f32("own_weight").or_else(|| {
                r.f32("weight").filter(|w| *w != Self::NEUTRAL_WEIGHT)
            }),
            deleted_at: r.timestamp("deleted_at"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_string("name", was.map(|b| b.name.as_str()), &self.name)?;
        w.set_opt_str("parent_id", was.map(|b| b.parent_id.as_ref()), self.parent_id.as_ref())?;
        w.set_opt_str("color", was.map(|b| b.color.as_ref()), self.color.as_ref())?;
        w.set_str("order", was.map(|b| &b.order), &self.order)?;
        w.set_bool("archived", was.map(|b| b.archived), self.archived)?;
        w.set_bool("is_inbox", was.map(|b| b.is_inbox), self.is_inbox)?;
        if was.map(|b| b.weight) != Some(self.weight) {
            match self.weight {
                Some(weight) => w.set_f32("own_weight", None, weight)?,
                None => w.clear("own_weight")?,
            }
            // The legacy key would otherwise go on overriding an inherited weight.
            w.clear("weight")?;
        }
        w.set_opt_time("deleted_at", was.map(|b| b.deleted_at.as_ref()), self.deleted_at.as_ref())
    }
}

impl Record for Label {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "labels";
    type Key = LabelId;

    fn key(&self) -> LabelId {
        self.id
    }

    fn key_string(key: &LabelId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<LabelId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: LabelId) -> Option<Self> {
        Some(Self {
            id,
            name: r.string("name").unwrap_or_default(),
            color: r.string("color"),
            order: r.parsed("order")?,
            deleted_at: r.timestamp("deleted_at"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_string("name", was.map(|b| b.name.as_str()), &self.name)?;
        w.set_opt_str("color", was.map(|b| b.color.as_ref()), self.color.as_ref())?;
        w.set_str("order", was.map(|b| &b.order), &self.order)?;
        w.set_opt_time("deleted_at", was.map(|b| b.deleted_at.as_ref()), self.deleted_at.as_ref())
    }
}

impl Record for SavedFilter {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "filters";
    type Key = FilterId;

    fn key(&self) -> FilterId {
        self.id
    }

    fn key_string(key: &FilterId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<FilterId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: FilterId) -> Option<Self> {
        Some(Self {
            id,
            name: r.string("name").unwrap_or_default(),
            // Stored as text, never as a resolved date range (§6.2): a filter saved as
            // "due before next Friday" must still mean that next month.
            query: r.string("query")?,
            order: r.parsed("order")?,
            color: r.string("color"),
            deleted_at: r.timestamp("deleted_at"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_string("name", was.map(|b| b.name.as_str()), &self.name)?;
        w.set_string("query", was.map(|b| b.query.as_str()), &self.query)?;
        w.set_str("order", was.map(|b| &b.order), &self.order)?;
        w.set_opt_str("color", was.map(|b| b.color.as_ref()), self.color.as_ref())?;
        w.set_opt_time("deleted_at", was.map(|b| b.deleted_at.as_ref()), self.deleted_at.as_ref())
    }
}

// ---------------------------------------------------------------------------------------
// Blocks
// ---------------------------------------------------------------------------------------

impl Record for BlockSeries {
    const DOMAIN: Domain = Domain::Blocks;
    const COLLECTION: &'static str = "series";
    type Key = SeriesId;

    fn key(&self) -> SeriesId {
        self.id
    }

    fn key_string(key: &SeriesId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<SeriesId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: SeriesId) -> Option<Self> {
        let kind = block_kind(r.string("kind"));
        Some(Self {
            id,
            title: r.string("title").unwrap_or_default(),
            notes: r.text("notes"),
            kind,
            flags: r.map("flags").map_or_else(|| kind.default_flags(), |f| read_flags(&f, kind)),
            start_time: r.parsed("start_time")?,
            // Duration, not end time (§3.6). Zero is not a block.
            duration_mins: r.u32("duration_mins").filter(|d| *d > 0)?,
            min_duration_mins: r.u32("min_duration_mins"),
            start_date: r.parsed("start_date")?,
            end_date: r.parsed("end_date"),
            rrule: r.string("rrule"),
            timezone: r.string("timezone").map(TzName::new),
            color: r.string("color"),
            icon: r.string("icon"),
            task_filter: r.string("task_filter"),
            external: r.map("external").and_then(|e| read_external(&e)),
            deleted_at: r.timestamp("deleted_at"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_string("title", was.map(|b| b.title.as_str()), &self.title)?;
        w.set_text("notes", was.map(|b| b.notes.as_str()), &self.notes)?;
        w.set_string("kind", was.map(|b| block_kind_str(b.kind)), block_kind_str(self.kind))?;
        if was.map(|b| b.flags) != Some(self.flags) {
            let mut flags = w.map("flags")?;
            write_flags(&mut flags, &self.flags)?;
        }
        w.set_str("start_time", was.map(|b| &b.start_time), &self.start_time)?;
        w.set_int(
            "duration_mins",
            was.map(|b| i64::from(b.duration_mins)),
            i64::from(self.duration_mins),
        )?;
        w.set_opt_u32(
            "min_duration_mins",
            was.map(|b| b.min_duration_mins),
            self.min_duration_mins,
        )?;
        w.set_str("start_date", was.map(|b| &b.start_date), &self.start_date)?;
        w.set_opt_str("end_date", was.map(|b| b.end_date.as_ref()), self.end_date.as_ref())?;
        w.set_opt_str("rrule", was.map(|b| b.rrule.as_ref()), self.rrule.as_ref())?;
        w.set_opt_str("timezone", was.map(|b| b.timezone.as_ref()), self.timezone.as_ref())?;
        w.set_opt_str("color", was.map(|b| b.color.as_ref()), self.color.as_ref())?;
        w.set_opt_str("icon", was.map(|b| b.icon.as_ref()), self.icon.as_ref())?;
        w.set_opt_str(
            "task_filter",
            was.map(|b| b.task_filter.as_ref()),
            self.task_filter.as_ref(),
        )?;
        w.set_nested(
            "external",
            was.map(|b| b.external.as_ref()),
            self.external.as_ref(),
            write_external,
        )?;
        w.set_opt_time("deleted_at", was.map(|b| b.deleted_at.as_ref()), self.deleted_at.as_ref())
    }
}

impl Record for BlockException {
    const DOMAIN: Domain = Domain::Blocks;
    const COLLECTION: &'static str = "exceptions";
    // Keyed by series and date, which any device editing that occurrence arrives at.
    const INLINE: bool = true;
    type Key = (SeriesId, civil::Date);

    fn key(&self) -> Self::Key {
        (self.series_id, self.original_date)
    }

    fn key_string(key: &Self::Key) -> String {
        format!("{}@{}", key.0, date_key(key.1))
    }

    fn parse_key(text: &str) -> Option<Self::Key> {
        let (id, date) = split_occurrence_key(text)?;
        Some((id.parse().ok()?, date))
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, key: Self::Key) -> Option<Self> {
        let action = if r.string("action").as_deref() == Some("cancelled") {
            ExceptionAction::Cancelled
        } else {
            ExceptionAction::Modified {
                start_time: r.parsed("start_time"),
                duration_mins: r.u32("duration_mins"),
                title: r.string("title"),
                kind: r.string("kind").map(|k| block_kind(Some(k))),
                flags: r.map("flags").map(|f| {
                    read_flags(&f, r.string("kind").map_or(BlockKind::Work, |k| block_kind(Some(k))))
                }),
            }
        };
        Some(Self { series_id: key.0, original_date: key.1, action })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        if was.map(|b| &b.action) == Some(&self.action) {
            return Ok(());
        }
        match &self.action {
            ExceptionAction::Cancelled => {
                w.set_string("action", None, "cancelled")?;
                for key in ["start_time", "duration_mins", "title", "kind", "flags"] {
                    w.clear(key)?;
                }
            }
            ExceptionAction::Modified { start_time, duration_mins, title, kind, flags } => {
                // Field by field against what was there, so that two devices changing
                // different things about one occurrence both keep their change.
                let before = match was.map(|b| &b.action) {
                    Some(ExceptionAction::Modified {
                        start_time,
                        duration_mins,
                        title,
                        kind,
                        flags,
                    }) => Some((start_time, duration_mins, title, kind, flags)),
                    _ => None,
                };
                w.set_string("action", before.map(|_| "modified"), "modified")?;
                w.set_opt_str(
                    "start_time",
                    before.map(|b| b.0.as_ref()),
                    start_time.as_ref(),
                )?;
                w.set_opt_u32("duration_mins", before.map(|b| *b.1), *duration_mins)?;
                w.set_opt_str("title", before.map(|b| b.2.as_ref()), title.as_ref())?;
                if before.map(|b| b.3) != Some(kind) {
                    match kind {
                        Some(k) => w.set_string("kind", None, block_kind_str(*k))?,
                        None => w.clear("kind")?,
                    }
                }
                if before.map(|b| b.4) != Some(flags) {
                    match flags {
                        Some(f) => {
                            let mut nested = w.map("flags")?;
                            write_flags(&mut nested, f)?;
                        }
                        None => w.clear("flags")?,
                    }
                }
            }
        }
        Ok(())
    }
}

impl Record for BlockAssignment {
    const DOMAIN: Domain = Domain::Blocks;
    const COLLECTION: &'static str = "assignments";
    type Key = AssignmentId;

    fn key(&self) -> AssignmentId {
        self.id
    }

    fn key_string(key: &AssignmentId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<AssignmentId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: AssignmentId) -> Option<Self> {
        Some(Self {
            id,
            block_ref: r.map("block").and_then(|b| read_block_ref(&b))?,
            task_id: r.parsed("task_id")?,
            planned_mins: r.u32("planned_mins"),
            accumulated_mins: r.u32("accumulated_mins").unwrap_or(0),
            running_since: r.timestamp("running_since"),
            status: status(r.string("status")),
            order: r.parsed("order")?,
            created_at: r.timestamp("created_at")?,
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_nested(
            "block",
            was.map(|b| Some(&b.block_ref)),
            Some(&self.block_ref),
            write_block_ref,
        )?;
        w.set_str("task_id", was.map(|b| &b.task_id), &self.task_id)?;
        w.set_opt_u32("planned_mins", was.map(|b| b.planned_mins), self.planned_mins)?;
        w.set_int(
            "accumulated_mins",
            was.map(|b| i64::from(b.accumulated_mins)),
            i64::from(self.accumulated_mins),
        )?;
        w.set_opt_time(
            "running_since",
            was.map(|b| b.running_since.as_ref()),
            self.running_since.as_ref(),
        )?;
        w.set_string("status", was.map(|b| status_str(b.status)), status_str(self.status))?;
        w.set_str("order", was.map(|b| &b.order), &self.order)?;
        w.set_time("created_at", was.map(|b| &b.created_at), &self.created_at)
    }
}

// ---------------------------------------------------------------------------------------
// Reminders
// ---------------------------------------------------------------------------------------

impl Record for Reminder {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "reminders";
    type Key = ReminderId;

    fn key(&self) -> ReminderId {
        self.id
    }

    fn key_string(key: &ReminderId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<ReminderId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, id: ReminderId) -> Option<Self> {
        let target = match r.string("target_kind").as_deref() {
            Some("task") => ReminderTarget::Task(r.parsed("target_id")?),
            Some("block") => ReminderTarget::Block(r.parsed("target_id")?),
            _ => return None,
        };
        let trigger = r.map("trigger").and_then(|t| read_trigger(&t))?;
        // An anchor that cannot apply to its target (§3.8) means a writer that was wrong,
        // and there is no sound way to guess which half it meant.
        if !trigger.anchor.applies_to(&target) {
            return None;
        }
        Some(Self {
            id,
            target,
            trigger,
            delivery: r.map("delivery").map_or(Delivery::AllDevices, |d| read_delivery(&d)),
            deleted_at: r.timestamp("deleted_at"),
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        let (kind, id) = match self.target {
            ReminderTarget::Task(id) => ("task", id.to_string()),
            ReminderTarget::Block(id) => ("block", id.to_string()),
        };
        w.set_string("target_kind", None, kind)?;
        w.set_string("target_id", None, &id)?;
        w.set_nested(
            "trigger",
            was.map(|b| Some(&b.trigger)),
            Some(&self.trigger),
            write_trigger,
        )?;
        w.set_nested(
            "delivery",
            was.map(|b| Some(&b.delivery)),
            Some(&self.delivery),
            write_delivery,
        )?;
        w.set_opt_time("deleted_at", was.map(|b| b.deleted_at.as_ref()), self.deleted_at.as_ref())
    }
}

impl Record for ReminderAck {
    const DOMAIN: Domain = Domain::Core;
    const COLLECTION: &'static str = "acks";
    // Keyed by reminder and date, which every device that dismisses that firing arrives at.
    const INLINE: bool = true;
    type Key = (ReminderId, civil::Date);

    fn key(&self) -> Self::Key {
        (self.reminder_id, self.occurrence_date)
    }

    fn key_string(key: &Self::Key) -> String {
        format!("{}@{}", key.0, date_key(key.1))
    }

    fn parse_key(text: &str) -> Option<Self::Key> {
        let (id, date) = split_occurrence_key(text)?;
        Some((id.parse().ok()?, date))
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, key: Self::Key) -> Option<Self> {
        let action = match r.string("action").as_deref() {
            Some("snoozed") => ReminderAction::Snoozed { until: r.parsed("until")? },
            Some("dismissed") => ReminderAction::Dismissed,
            _ => return None,
        };
        Some(Self {
            reminder_id: key.0,
            occurrence_date: key.1,
            action,
            acked_at: r.timestamp("acked_at")?,
            acked_by: r.parsed("acked_by")?,
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        if was.map(|b| &b.action) != Some(&self.action) {
            match &self.action {
                ReminderAction::Dismissed => {
                    w.set_string("action", None, "dismissed")?;
                    w.clear("until")?;
                }
                ReminderAction::Snoozed { until } => {
                    w.set_string("action", None, "snoozed")?;
                    w.set_str("until", None, until)?;
                }
            }
        }
        w.set_time("acked_at", was.map(|b| &b.acked_at), &self.acked_at)?;
        w.set_str("acked_by", was.map(|b| &b.acked_by), &self.acked_by)
    }
}

// ---------------------------------------------------------------------------------------
// Device
// ---------------------------------------------------------------------------------------

impl Record for Device {
    const DOMAIN: Domain = Domain::Devices;
    const COLLECTION: &'static str = "devices";
    // Keyed by the device's public key, which both sides of a pairing write.
    const INLINE: bool = true;
    type Key = NodeId;

    fn key(&self) -> NodeId {
        self.node_id
    }

    fn key_string(key: &NodeId) -> String {
        key.to_string()
    }

    fn parse_key(text: &str) -> Option<NodeId> {
        text.parse().ok()
    }

    fn hydrate<D: ReadDoc>(r: &Reader<'_, D>, node_id: NodeId) -> Option<Self> {
        Some(Self {
            node_id,
            name: r.string("name").unwrap_or_default(),
            platform: r.string("platform").unwrap_or_default(),
            paired_at: r.timestamp("paired_at")?,
            last_seen: r.timestamp("last_seen")?,
        })
    }

    fn write<T: Transactable>(&self, w: &mut Writer<'_, T>, was: Option<&Self>) -> Result<()> {
        w.set_string("name", was.map(|b| b.name.as_str()), &self.name)?;
        w.set_string("platform", was.map(|b| b.platform.as_str()), &self.platform)?;
        w.set_time("paired_at", was.map(|b| &b.paired_at), &self.paired_at)?;
        w.set_time("last_seen", was.map(|b| &b.last_seen), &self.last_seen)
    }
}

// ---------------------------------------------------------------------------------------
// Settings — a singleton, not a collection.
// ---------------------------------------------------------------------------------------

/// Reads settings, falling back field by field to the defaults.
///
/// Never fails and never skips: an unreadable setting is one preference reverting, which is
/// annoying, whereas refusing the whole record would silently reset all of them at once.
pub(crate) fn read_settings<D: ReadDoc>(r: &Reader<'_, D>) -> Settings {
    let defaults = Settings::default();
    // An absent key means the setting was never written, so the default applies. A key
    // holding an empty map means the user emptied the list, which is a different thing and
    // must survive a reload.
    let triggers = |key: &str, default: &[Trigger]| -> Vec<Trigger> {
        r.map(key).map_or_else(|| default.to_vec(), |list| {
            let mut keys = list.keys();
            keys.sort();
            keys.iter().filter_map(|k| list.map(k)).filter_map(|t| read_trigger(&t)).collect()
        })
    };
    let task_reminders = triggers("default_task_reminders", &defaults.default_task_reminders);
    let block_reminders = triggers("default_block_reminders", &defaults.default_block_reminders);
    Settings {
        cascade_complete_subtasks: r
            .bool("cascade_complete_subtasks")
            .unwrap_or(defaults.cascade_complete_subtasks),
        verbosity: match r.string("verbosity").as_deref() {
            Some("terse") => Verbosity::Terse,
            _ => Verbosity::Full,
        },
        default_task_reminders: task_reminders,
        default_block_reminders: block_reminders,
        all_day_reminder_hour: r
            .parsed("all_day_reminder_hour")
            .unwrap_or(defaults.all_day_reminder_hour),
        day_window: (
            r.parsed("day_start").unwrap_or(defaults.day_window.0),
            r.parsed("day_end").unwrap_or(defaults.day_window.1),
        ),
        week_start: r
            .int("week_start")
            .and_then(|n| i8::try_from(n).ok())
            .and_then(|n| civil::Weekday::from_monday_zero_offset(n).ok())
            .unwrap_or(defaults.week_start),
    }
}

/// Writes the settings that differ from `before`.
pub(crate) fn write_settings<T: Transactable>(
    w: &mut Writer<'_, T>,
    before: Option<&Settings>,
    now: &Settings,
) -> Result<()> {
    w.set_bool(
        "cascade_complete_subtasks",
        before.map(|b| b.cascade_complete_subtasks),
        now.cascade_complete_subtasks,
    )?;
    let verbosity = |v: Verbosity| match v {
        Verbosity::Terse => "terse",
        Verbosity::Full => "full",
    };
    w.set_string("verbosity", before.map(|b| verbosity(b.verbosity)), verbosity(now.verbosity))?;
    write_triggers(
        w,
        "default_task_reminders",
        before.map(|b| b.default_task_reminders.as_slice()),
        &now.default_task_reminders,
    )?;
    write_triggers(
        w,
        "default_block_reminders",
        before.map(|b| b.default_block_reminders.as_slice()),
        &now.default_block_reminders,
    )?;
    w.set_str(
        "all_day_reminder_hour",
        before.map(|b| &b.all_day_reminder_hour),
        &now.all_day_reminder_hour,
    )?;
    w.set_str("day_start", before.map(|b| &b.day_window.0), &now.day_window.0)?;
    w.set_str("day_end", before.map(|b| &b.day_window.1), &now.day_window.1)?;
    w.set_int(
        "week_start",
        before.map(|b| i64::from(b.week_start.to_monday_zero_offset())),
        i64::from(now.week_start.to_monday_zero_offset()),
    )
}

/// Writes a list of triggers as a map keyed by position.
///
/// A map rather than a list because the whole thing is replaced when it changes: these are
/// short preference lists edited in one screen, so preserving concurrent insertions into
/// them would be machinery serving nothing.
fn write_triggers<T: Transactable>(
    w: &mut Writer<'_, T>,
    key: &str,
    before: Option<&[Trigger]>,
    now: &[Trigger],
) -> Result<()> {
    if before == Some(now) {
        return Ok(());
    }
    w.clear(key)?;
    let mut list = w.map(key)?;
    for (index, trigger) in now.iter().enumerate() {
        let mut entry = list.map(&format!("{index:04}"))?;
        write_trigger(&mut entry, trigger)?;
    }
    Ok(())
}
