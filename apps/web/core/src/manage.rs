//! What the desktop apps offer beyond tasks and the day, for the browser: projects, labels
//! and saved filters to manage, tasks to wait for, settings, the sync status, backups and
//! exports, and every block setting with pausing timers. Each export is one surface call;
//! nothing is decided here.

use lumenna_surface::{
    BlockDefaults, BlockEdit, BlockFields, BlockShown, Change, Direction, ExportFormat, Exported, Imported,
    NewBlock, PlanBlock, SettingList, SyncStatus, Timer, Weight,
};
use tsify::Ts;
use wasm_bindgen::prelude::*;

use crate::{Core, Out, error, js, out};

#[wasm_bindgen]
impl Core {
    // Projects ------------------------------------------------------------------------------

    /// A new project, at the top level or inside `parent`.
    #[wasm_bindgen(js_name = addProject)]
    pub fn add_project(&self, name: &str, parent: Option<String>) -> Out<Change> {
        out(self.lumenna.add_project(name, parent))
    }

    /// The surface's method of the same name, as `renameProject`.
    #[wasm_bindgen(js_name = renameProject)]
    pub fn rename_project(&self, name: &str, to: &str) -> Out<Change> {
        out(self.lumenna.rename_project(name, to))
    }

    /// Moves a project under another, or to the top level without one.
    #[wasm_bindgen(js_name = moveProject)]
    pub fn move_project(&self, name: &str, parent: Option<String>) -> Out<Change> {
        out(self.lumenna.move_project(name, parent))
    }

    /// Moves a project one place among its siblings.
    #[wasm_bindgen(js_name = reorderProject)]
    pub fn reorder_project(&self, name: &str, direction: Ts<Direction>) -> Out<Change> {
        out(self.lumenna.reorder_project(name, direction.to_rust()?))
    }

    /// How much a project's whole area matters now.
    #[wasm_bindgen(js_name = weighProject)]
    pub fn weigh_project(&self, name: &str, weight: Ts<Weight>) -> Out<Change> {
        out(self.lumenna.weigh_project(name, weight.to_rust()?))
    }

    /// Archives a project, or unarchives it if it was.
    #[wasm_bindgen(js_name = archiveProject)]
    pub fn archive_project(&self, name: &str) -> Out<Change> {
        out(self.lumenna.archive_project(name))
    }

    /// Deletes a project; its tasks go to the trash with it, or to the Inbox when kept.
    #[wasm_bindgen(js_name = deleteProject)]
    pub fn delete_project(&self, name: &str, keep_tasks: bool) -> Out<Change> {
        out(self.lumenna.delete_project(name, keep_tasks))
    }

    // Labels --------------------------------------------------------------------------------

    /// The surface's method of the same name, as `addLabel`.
    #[wasm_bindgen(js_name = addLabel)]
    pub fn add_label(&self, name: &str) -> Out<Change> {
        out(self.lumenna.add_label(name))
    }

    /// The surface's method of the same name, as `renameLabel`.
    #[wasm_bindgen(js_name = renameLabel)]
    pub fn rename_label(&self, name: &str, to: &str) -> Out<Change> {
        out(self.lumenna.rename_label(name, to))
    }

    /// Moves one label's tasks onto another, and removes it.
    #[wasm_bindgen(js_name = mergeLabels)]
    pub fn merge_labels(&self, from: &str, into: &str) -> Out<Change> {
        out(self.lumenna.merge_labels(from, into))
    }

    /// A label's colour, by name, or none.
    #[wasm_bindgen(js_name = recolourLabel)]
    pub fn recolour_label(&self, name: &str, colour: Option<String>) -> Out<Change> {
        out(self.lumenna.recolour_label(name, colour))
    }

    /// Moves a label one place.
    #[wasm_bindgen(js_name = reorderLabel)]
    pub fn reorder_label(&self, name: &str, direction: Ts<Direction>) -> Out<Change> {
        out(self.lumenna.reorder_label(name, direction.to_rust()?))
    }

    /// The surface's method of the same name, as `deleteLabel`.
    #[wasm_bindgen(js_name = deleteLabel)]
    pub fn delete_label(&self, name: &str) -> Out<Change> {
        out(self.lumenna.delete_label(name))
    }

    // Saved filters -------------------------------------------------------------------------

    /// The surface's method of the same name, as `addFilter`.
    #[wasm_bindgen(js_name = addFilter)]
    pub fn add_filter(&self, name: &str, query: &str) -> Out<Change> {
        out(self.lumenna.add_filter(name, query))
    }

    /// Renames a saved filter, changes its query, or both.
    #[wasm_bindgen(js_name = editFilter)]
    pub fn edit_filter(&self, name: &str, rename: Option<String>, query: Option<String>) -> Out<Change> {
        out(self.lumenna.edit_filter(name, rename, query))
    }

    /// Moves a saved filter one place.
    #[wasm_bindgen(js_name = reorderFilter)]
    pub fn reorder_filter(&self, name: &str, direction: Ts<Direction>) -> Out<Change> {
        out(self.lumenna.reorder_filter(name, direction.to_rust()?))
    }

    /// The surface's method of the same name, as `deleteFilter`.
    #[wasm_bindgen(js_name = deleteFilter)]
    pub fn delete_filter(&self, name: &str) -> Out<Change> {
        out(self.lumenna.delete_filter(name))
    }

    // Waiting for other tasks ----------------------------------------------------------------

    /// Makes task `id` wait for task `on`.
    #[wasm_bindgen(js_name = addDependency)]
    pub fn add_dependency(&self, id: &str, on: &str) -> Out<Change> {
        out(self.lumenna.add_dependency(id, on))
    }

    /// Stops task `id` waiting for task `on`.
    #[wasm_bindgen(js_name = removeDependency)]
    pub fn remove_dependency(&self, id: &str, on: &str) -> Out<Change> {
        out(self.lumenna.remove_dependency(id, on))
    }

    // Settings, sync status, backups --------------------------------------------------------

    /// Every setting, or one by key.
    pub fn settings(&self, key: Option<String>) -> Out<SettingList> {
        out(self.lumenna.settings(key))
    }

    /// The surface's method of the same name, as `setSetting`.
    #[wasm_bindgen(js_name = setSetting)]
    pub fn set_setting(&self, key: &str, value: &str) -> Out<Change> {
        out(self.lumenna.set_setting(key, value))
    }

    /// How syncing is going: this browser's loop, and each device.
    #[wasm_bindgen(js_name = syncStatus)]
    pub fn sync_status(&self) -> Out<SyncStatus> {
        out(self.lumenna.sync_status_with(self.sync_running()))
    }

    /// A backup taken now, history and trash included, for the page to save as a download:
    /// `{ name, bytes, said }`.
    #[wasm_bindgen(js_name = backupFile)]
    pub fn backup_file(&self) -> Result<js_sys::Object, JsError> {
        let file = self.lumenna.backup_file().map_err(error)?;
        let object = js_sys::Object::new();
        let set = |key: &str, value: &JsValue| js_sys::Reflect::set(&object, &JsValue::from_str(key), value);
        let said = lumenna_desktop::speech::announcement(&file.announcement, &file.notices);
        set("name", &JsValue::from_str(&file.name)).map_err(|_| JsError::new("could not hand over the backup"))?;
        set("bytes", &js_sys::Uint8Array::from(file.bytes.as_slice())).map_err(|_| JsError::new("could not hand over the backup"))?;
        set("said", &JsValue::from_str(&said)).map_err(|_| JsError::new("could not hand over the backup"))?;
        Ok(object)
    }

    /// Reads a JSON export or a backup the person chose, by its name and contents.
    #[wasm_bindgen(js_name = importBytes)]
    pub fn import_bytes(&self, name: &str, bytes: Vec<u8>) -> Out<Imported> {
        out(self.lumenna.import_bytes(name, bytes))
    }

    /// The current state as `format`, returned rather than written, for a download.
    pub fn export(&self, format: Ts<ExportFormat>) -> Out<Exported> {
        out(self.lumenna.export(format.to_rust()?, None, false))
    }

    // Blocks and timers ---------------------------------------------------------------------

    /// Pauses a sitting's timer, keeping the time so far.
    #[wasm_bindgen(js_name = pauseTimer)]
    pub fn pause_timer(&self, assignment: &str) -> Out<Timer> {
        out(self.lumenna.pause_timer(assignment))
    }
}

/// What a kind of block has unless set apart, or nothing for a word that is not a kind.
#[wasm_bindgen(js_name = blockDefaults)]
pub fn block_defaults(kind: String) -> Result<Option<Ts<BlockDefaults>>, JsError> {
    lumenna_surface::block_defaults(kind).map(|d| js(&d)).transpose()
}

/// A series' fields as the form for every occurrence starts from them.
#[wasm_bindgen(js_name = blockFields)]
pub fn block_fields(block: Ts<BlockShown>) -> Out<BlockFields> {
    js(&lumenna_surface::block_fields(block.to_rust()?))
}

/// One day's block as the form for that day alone starts from it.
#[wasm_bindgen(js_name = dayBlockFields)]
pub fn day_block_fields(block: Ts<PlanBlock>) -> Out<BlockFields> {
    js(&lumenna_surface::day_block_fields(block.to_rust()?))
}

/// What saving `after` over `before` sends — only what changed — or nothing.
#[wasm_bindgen(js_name = blockEdit)]
pub fn block_edit(before: Ts<BlockFields>, after: Ts<BlockFields>) -> Result<Option<Ts<BlockEdit>>, JsError> {
    lumenna_surface::block_edit(before.to_rust()?, after.to_rust()?).map_err(error)?.map(|e| js(&e)).transpose()
}

/// What a new block's form sends, starting on `date`.
#[wasm_bindgen(js_name = newBlock)]
pub fn new_block(fields: Ts<BlockFields>, date: Option<String>) -> Out<NewBlock> {
    out(lumenna_surface::new_block(fields.to_rust()?, date))
}

/// One export Settings offers: its format, what its button says, and the file it downloads as.
#[derive(serde::Serialize, serde::Deserialize, tsify::Tsify)]
pub struct ExportChoice {
    /// What it writes.
    pub format: ExportFormat,
    /// What its button says, as the desktop apps word it — without their "...", since a
    /// download asks nothing more.
    pub label: String,
    /// The file it is offered as, dated today: "Lumenna 2026-10-05.json".
    pub name: String,
}

/// Every export, in the order Settings offers them.
#[derive(serde::Serialize, serde::Deserialize, tsify::Tsify)]
pub struct ExportChoices {
    /// Each one.
    pub exports: Vec<ExportChoice>,
}

/// The exports Settings offers.
#[wasm_bindgen(js_name = exportChoices)]
pub fn export_choices() -> Out<ExportChoices> {
    let today = jiff::Zoned::now().date();
    let exports = lumenna_desktop::devices::EXPORTS
        .iter()
        .map(|export| ExportChoice {
            format: export.format,
            label: export.label.trim_end_matches("...").to_owned(),
            name: lumenna_desktop::devices::export_name(export.format, today),
        })
        .collect();
    js(&ExportChoices { exports })
}

/// A project's weight as typed: a number above zero, or `inherit`. Refuses anything else.
#[wasm_bindgen(js_name = parseWeight)]
pub fn parse_weight(text: String) -> Out<lumenna_surface::Weight> {
    out(lumenna_surface::parse_weight(text))
}
