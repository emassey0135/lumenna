//! Generations: the store's history begun again, without what a real erasure removed.
//!
//! An Automerge document keeps every change it was ever given, so a record purged from the
//! current state is still in its history, in every replica and every backup. The only way to
//! erase it for good is to write a new history: every document begun again from its genesis,
//! holding the current state as one change, with the erased records left out. That is a
//! **generation**: a UUIDv7, newest wins, under which every document syncs as `core@<id>`
//! (`SyncName`). An older build reads no such name, so it can never send the old history back
//! into the new one, and the sync session's protocol says the same to one that would mix them.
//!
//! A device that meets a newer generation **adopts** it: it syncs the newer documents into
//! documents held aside, then writes into them, field by field, whatever it changed that the
//! rebuild never saw (`Doc::fold`, against each document as it stood at the heads the
//! rebuild started from), and replaces its own. Nothing erased comes back that way: each
//! generation names everything erased so far (`erased`, at core's root), and a record named
//! there, or one belonging to a task named there, is never written into a generation.
//!
//! Two devices erasing at once make two generations from one. The newer is adopted by both;
//! a device whose own erasure the newer one lacks erases again on top of it, so the result
//! holds both.
//!
//! Each generation's documents say at their root which generation they are (`generation`),
//! what they were rebuilt from (`rebuilt_from`: the generation before and its heads), and in
//! core what has been erased and how often each task's date was put off, which the history
//! the tally reads from no longer holds (`postponed`).

use std::collections::{BTreeMap, BTreeSet};

use automerge::ChangeHash;

use crate::doc::{DocId, Documents, Raw};
use crate::error::Result;

/// A new generation, after `after` if there is one: identifiers are time-ordered, and newest
/// wins, so one made on a clock behind another device's is still the newer.
pub(crate) fn mint(after: Option<&str>) -> String {
    loop {
        let minted = uuid::Uuid::now_v7().to_string();
        if after.is_none_or(|after| minted.as_str() > after) {
            return minted;
        }
    }
}

/// What every document of a generation says of where it came from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RebuiltFrom {
    /// The generation it was rebuilt from: `None` for the first.
    pub generation: Option<String>,
    /// That generation's heads of the same document, when the rebuild had it.
    pub heads: Vec<ChangeHash>,
}

impl RebuiltFrom {
    fn encode(&self) -> String {
        serde_json::json!({
            "generation": self.generation,
            "heads": self.heads.iter().map(ToString::to_string).collect::<Vec<_>>(),
        })
        .to_string()
    }

    fn decode(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        Some(Self {
            generation: value.get("generation")?.as_str().map(str::to_owned),
            heads: value.get("heads")?.as_array()?.iter().filter_map(|h| h.as_str()?.parse().ok()).collect(),
        })
    }
}

/// What a generation has erased, as `<collection>/<key>`.
pub(crate) type Erased = BTreeSet<String>;

/// The generation a set of documents is, as its `core` says; `None` for the first.
pub(crate) fn of(docs: &Documents) -> Option<String> {
    docs.core_ref().root_text("generation")
}

/// What `docs` has erased so far.
pub(crate) fn erased(docs: &Documents) -> Erased {
    docs.core_ref()
        .root_text("erased")
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

/// How often each task's date was put off before this generation began, by task.
pub(crate) fn postponed_before(docs: &Documents) -> BTreeMap<String, u32> {
    docs.core_ref().root_text("postponed").and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

/// What goes with what was erased, worked out once from the documents: the tasks erased, and
/// the reminders that go with them or were erased themselves, whose acknowledgements go too.
pub(crate) struct Gone<'a> {
    erased: &'a Erased,
    tasks: BTreeSet<String>,
    reminders: BTreeSet<String>,
}

impl<'a> Gone<'a> {
    /// What goes from `docs` with `erased`: every reminder of an erased task, its own and the
    /// defaults its acknowledgements are kept under (`remind::default_key`, from the default
    /// reminders set now).
    pub(crate) fn of(erased: &'a Erased, docs: &Documents) -> Self {
        let tasks: BTreeSet<String> = erased.iter().filter_map(|key| key.strip_prefix("tasks/")).map(str::to_owned).collect();
        let mut reminders: BTreeSet<String> =
            erased.iter().filter_map(|key| key.strip_prefix("reminders/")).map(str::to_owned).collect();
        let core = docs.core_ref();
        for (key, raw) in core.entries("reminders").unwrap_or_default() {
            if raw.text_at("target_kind") == Some("task") && raw.text_at("target_id").is_some_and(|task| tasks.contains(task)) {
                reminders.insert(key);
            }
        }
        let defaults = core.settings().default_task_reminders;
        for task in &tasks {
            let Ok(task) = task.parse() else { continue };
            let target = lumenna_core::model::ReminderTarget::Task(task);
            reminders.extend(defaults.iter().map(|trigger| lumenna_core::remind::default_key(&target, trigger).to_string()));
        }
        Self { erased, tasks, reminders }
    }

    /// Whether the reminder `id` stays: not erased, nor one of an erased task's.
    pub(crate) fn keeps_reminder(&self, id: &str) -> bool {
        !self.reminders.contains(id)
    }

    /// Whether a record goes into a generation that has erased what this names: not one named
    /// there, nor a completion, sitting or reminder of a task named there, nor an
    /// acknowledgement of a reminder that went.
    pub(crate) fn keeps(&self, collection: &str, key: &str, raw: &Raw) -> bool {
        // An inline record's fields are keys of their own, `<record>/<field>`.
        let record = key.split_once('/').map_or(key, |(record, _)| record);
        if self.erased.contains(&format!("{collection}/{record}")) {
            return false;
        }
        match collection {
            "completions" | "assignments" => raw.text_at("task_id").is_none_or(|task| !self.tasks.contains(task)),
            "reminders" => !self.reminders.contains(record),
            // Keyed `<reminder>@<date>`.
            "acks" => !record.split_once('@').is_some_and(|(reminder, _)| self.reminders.contains(reminder)),
            _ => true,
        }
    }
}

/// Every loaded document of `docs`, begun again as generation `generation` holding what each
/// holds now, less `erased` (which names everything erased so far): the documents a real
/// erasure leaves. `postponed` is the tally the old history held.
pub(crate) fn rebuild(
    docs: &mut Documents,
    generation: &str,
    erased: &Erased,
    postponed: &BTreeMap<String, u32>,
) -> Result<Documents> {
    let from = of(docs);
    let gone = Gone::of(erased, docs);
    let keep = |collection: &str, key: &str, raw: &Raw| gone.keeps(collection, key, raw);
    let mut fresh = Documents::new();
    for doc in docs.iter_mut() {
        let id = doc.id();
        let rebuilt_from = RebuiltFrom { generation: from.clone(), heads: doc.heads() };
        let target = fresh.get_mut(id);
        target.ensure_schema()?;
        // Changes every device makes alike, a newer build's schema among them, are carried as
        // they are: made again here, under this device's actor, they would be made twice.
        target.load_incremental(&doc.genesis_changes())?;
        doc.copy_into(target, &keep)?;
        target.set_root_text("generation", generation)?;
        target.set_root_text("rebuilt_from", &rebuilt_from.encode())?;
        if id == DocId::Core {
            target.set_root_text("erased", &serde_json::to_string(&erased.iter().collect::<Vec<_>>()).unwrap_or_default())?;
            target.set_root_text("postponed", &serde_json::to_string(postponed).unwrap_or_default())?;
        }
    }
    Ok(fresh)
}

/// Writes into `newer`, a newer generation synced from a peer, what `mine` changed that the
/// rebuild never saw, leaving out everything `newer` has erased.
///
/// Each document's base is where the two histories part: the heads its rebuild started from,
/// when it was rebuilt from this device's own generation, else where this device's
/// generation began (`began`), since the newer one was rebuilt from some other. A document
/// the rebuild never had is all this device's.
pub(crate) fn fold_in(newer: &mut Documents, mine: &mut Documents, began: &BTreeMap<DocId, Vec<ChangeHash>>) -> Result<()> {
    let my_generation = of(mine);
    let generation = of(newer).unwrap_or_default();
    let erased = erased(newer);
    let gone = Gone::of(&erased, mine);
    let keep = |collection: &str, key: &str, raw: &Raw| gone.keeps(collection, key, raw);
    for doc in mine.iter_mut() {
        let id = doc.id();
        let theirs = newer.get_mut(id);
        theirs.ensure_schema()?;
        // As in a rebuild: what every device makes alike goes across as it is.
        theirs.load_incremental(&doc.genesis_changes())?;
        let rebuilt = theirs.root_text("rebuilt_from").and_then(|text| RebuiltFrom::decode(&text));
        let base = match rebuilt {
            Some(rebuilt) if rebuilt.generation == my_generation => rebuilt.heads,
            Some(_) => began.get(&id).cloned().unwrap_or_default(),
            None => Vec::new(),
        };
        let mut base = doc.at(&base)?;
        theirs.fold(&mut base, doc, &keep, &generation)?;
    }
    Ok(())
}
