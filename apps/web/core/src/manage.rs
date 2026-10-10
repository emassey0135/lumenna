//! What the desktop apps offer beyond tasks and the day, for the browser: a saved filter's
//! form, settings, the sync status, backups and exports, and every block setting. What is done
//! to a project, label, filter or task is the core's actions (`act`). Each export is one
//! surface call; nothing is decided here.

use lumenna_surface::{
    BlockDefaults, BlockEdit, BlockFields, BlockShown, Change, ExportFormat, Exported, Imported,
    NewBlock, PlanBlock, SettingList, SyncStatus,
};
use tsify::Ts;
use wasm_bindgen::prelude::*;

use crate::{Core, Out, error, js, out};

#[wasm_bindgen]
impl Core {
    // Saved filters -------------------------------------------------------------------------

    /// The surface's method of the same name, as `addFilter`.
    #[wasm_bindgen(js_name = addFilter)]
    pub fn add_filter(&self, name: &str, query: &str) -> Out<Change> {
        out(self.lumenna.add_filter(name, query))
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
    /// What its button says, as the desktop apps word it, in the web's sentence case — without
    /// their "...", since a download asks nothing more.
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
            label: lumenna_surface::sentence_case(export.label.trim_end_matches("...").to_owned()),
            name: lumenna_desktop::devices::export_name(export.format, today),
        })
        .collect();
    js(&ExportChoices { exports })
}
