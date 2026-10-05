//! Lumenna's core for the web client (§16.12): the command surface, exported to JavaScript.
//!
//! The browser runs the same core every other client does — the same store, SQLite and
//! Automerge, the same parsers, the same rules — compiled to WebAssembly. Nothing here
//! decides anything: each export is one call on [`Lumenna`], with the records it takes and
//! returns typed in TypeScript by `tsify`, derived from the surface's own records as UniFFI
//! derives Swift's. Reshaping a record breaks the web build as it breaks every other.
//!
//! The store lives in OPFS through SQLite's sync access handles, which only a dedicated
//! worker may use: this module is loaded by the app's worker, never by the page itself.
//! Values cross as `tsify::Ts`, converted inside each function, so a malformed value is an
//! ordinary error rather than a leak at the boundary.

use std::path::Path;

use lumenna_desktop::places::{self, Entry, Place};
use lumenna_desktop::speech;
use lumenna_surface::{Lumenna, LumennaError, Syntax, TaskDetail, TaskEdit, TaskFields};
use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// What every export returns: the record, or the core's sentence for what went wrong.
type Out<T> = Result<Ts<T>, JsError>;

fn out<T: Tsify + Serialize>(result: lumenna_surface::Result<T>) -> Out<T> {
    let value = result.map_err(error)?;
    Ok(Ts::from_rust(&value)?)
}

/// The core's own sentence, which is written to be read out.
fn error(error: LumennaError) -> JsError {
    JsError::new(error.message())
}

/// One open store.
#[wasm_bindgen]
pub struct Core {
    lumenna: Lumenna,
}

#[wasm_bindgen]
impl Core {
    /// Opens the profile called `name`, creating it on first use: SQLite in OPFS, durable
    /// across reloads, in the browser's persistent storage when it has been granted.
    pub async fn open(name: String) -> Result<Core, JsError> {
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        {
            console_error_panic_hook::set_once();
            let pool = sqlite_wasm_vfs::sahpool::OpfsSAHPoolCfgBuilder::new().directory("lumenna").build();
            sqlite_wasm_vfs::sahpool::install::<sqlite_wasm_rs::WasmOsCallback>(&pool, true)
                .await
                .map_err(|e| JsError::new(&format!("This browser's storage could not be opened: {e}")))?;
        }
        let lumenna = Lumenna::open_at(Path::new(&format!("/{name}"))).map_err(error)?;
        Ok(Core { lumenna })
    }

    /// A number that moves whenever another tab or process has written: what a view
    /// compares to know it should read again.
    #[wasm_bindgen(js_name = outsideVersion)]
    pub fn outside_version(&self) -> Result<f64, JsError> {
        // A JavaScript number holds this exactly: it is SQLite's data_version, a counter.
        Ok(self.lumenna.outside_version().map_err(error)? as f64)
    }

    /// The surface's method of the same name, as `listTasks`.
    #[wasm_bindgen(js_name = listTasks)]
    pub fn list_tasks(&self, query: &str) -> Out<lumenna_surface::Rows> {
        out(self.lumenna.list_tasks(query))
    }

    /// The surface's method of the same name, as `searchTasks`.
    #[wasm_bindgen(js_name = searchTasks)]
    pub fn search_tasks(&self, text: &str) -> Out<lumenna_surface::Rows> {
        out(self.lumenna.search_tasks(text))
    }

    /// The surface's method of the same name, as `showTask`.
    #[wasm_bindgen(js_name = showTask)]
    pub fn show_task(&self, id: &str) -> Out<lumenna_surface::TaskShown> {
        out(self.lumenna.show_task(id))
    }

    /// The surface's method of the same name, as `addTask`.
    #[wasm_bindgen(js_name = addTask)]
    pub fn add_task(&self, text: &str) -> Out<lumenna_surface::Change> {
        out(self.lumenna.add_task(text))
    }

    /// The surface's method of the same name, as `previewTask`.
    #[wasm_bindgen(js_name = previewTask)]
    pub fn preview_task(&self, text: &str) -> Out<lumenna_surface::Preview> {
        out(self.lumenna.preview_task(text))
    }

    /// What could go at `cursor`, a UTF-8 byte offset into `text`.
    #[wasm_bindgen(js_name = completeText)]
    pub fn complete_text(&self, text: &str, cursor: u32, syntax: Ts<Syntax>) -> Out<lumenna_surface::Completions> {
        out(self.lumenna.complete_text(text, cursor, syntax.to_rust()?))
    }

    /// The surface's method of the same name, as `editTask`.
    #[wasm_bindgen(js_name = editTask)]
    pub fn edit_task(&self, id: &str, edit: Ts<TaskEdit>) -> Out<lumenna_surface::Change> {
        out(self.lumenna.edit_task(id, edit.to_rust()?))
    }

    /// The surface's method of the same name, as `completeTask`.
    #[wasm_bindgen(js_name = completeTask)]
    pub fn complete_task(&self, id: &str) -> Out<lumenna_surface::Change> {
        out(self.lumenna.complete_task(id))
    }

    /// The surface's method of the same name, as `uncompleteTask`.
    #[wasm_bindgen(js_name = uncompleteTask)]
    pub fn uncomplete_task(&self, id: &str) -> Out<lumenna_surface::Change> {
        out(self.lumenna.uncomplete_task(id))
    }

    /// The surface's method of the same name, as `trashTask`.
    #[wasm_bindgen(js_name = trashTask)]
    pub fn trash_task(&self, id: &str) -> Out<lumenna_surface::Change> {
        out(self.lumenna.trash_task(id))
    }

    /// The surface's method of the same name, as `restoreTask`.
    #[wasm_bindgen(js_name = restoreTask)]
    pub fn restore_task(&self, id: &str) -> Out<lumenna_surface::Change> {
        out(self.lumenna.restore_task(id))
    }

    /// The surface's method of the same name, as `eraseTask`.
    #[wasm_bindgen(js_name = eraseTask)]
    pub fn erase_task(&self, id: &str) -> Out<lumenna_surface::Change> {
        out(self.lumenna.erase_task(id))
    }

    /// The surface's method of the same name, as `moveTask`.
    #[wasm_bindgen(js_name = moveTask)]
    pub fn move_task(&self, id: &str, to: Ts<lumenna_surface::MoveTarget>) -> Out<lumenna_surface::Change> {
        out(self.lumenna.move_task(id, to.to_rust()?))
    }

    /// The surface's method of the same name, as `undo`.
    pub fn undo(&self) -> Out<lumenna_surface::Change> {
        out(self.lumenna.undo())
    }

    /// The surface's method of the same name, as `redo`.
    pub fn redo(&self) -> Out<lumenna_surface::Change> {
        out(self.lumenna.redo())
    }

    /// The surface's method of the same name, as `listProjects`.
    #[wasm_bindgen(js_name = listProjects)]
    pub fn list_projects(&self) -> Out<lumenna_surface::Rows> {
        out(self.lumenna.list_projects())
    }

    /// The surface's method of the same name, as `listLabels`.
    #[wasm_bindgen(js_name = listLabels)]
    pub fn list_labels(&self) -> Out<lumenna_surface::Rows> {
        out(self.lumenna.list_labels())
    }

    /// The surface's method of the same name, as `listFilters`.
    #[wasm_bindgen(js_name = listFilters)]
    pub fn list_filters(&self) -> Out<lumenna_surface::Filters> {
        out(self.lumenna.list_filters())
    }

    /// A day's blocks and what is assigned to them; today when `date` is absent.
    pub fn plan(&self, date: Option<String>) -> Out<lumenna_surface::Plan> {
        out(self.lumenna.plan(date))
    }
}

/// A task's fields as a form starts from them.
#[wasm_bindgen(js_name = taskFields)]
pub fn task_fields(task: Ts<TaskDetail>) -> Out<TaskFields> {
    out(Ok(lumenna_surface::task_fields(task.to_rust()?)))
}

/// What saving `fields` over `task` sends, or nothing if no field changed — only what
/// changed, so a concurrent edit on another device is not reverted.
#[wasm_bindgen(js_name = taskEdit)]
pub fn task_edit(task: Ts<TaskDetail>, fields: Ts<TaskFields>) -> Result<Option<Ts<TaskEdit>>, JsError> {
    lumenna_surface::task_edit(task.to_rust()?, fields.to_rust()?).map(|edit| Ok(Ts::from_rust(&edit)?)).transpose()
}

// ---------------------------------------------------------------------------------------
// The places and wording the desktop apps share (`crates/desktop`), so the web says what
// they say rather than a copy of it.
// ---------------------------------------------------------------------------------------

/// The sidebar's rows, in order.
#[derive(Serialize, serde::Deserialize, Tsify)]
pub struct Sidebar {
    /// Each row, with what it is and how deep.
    pub entries: Vec<Entry>,
}

#[wasm_bindgen]
impl Core {
    /// Every place, as the sidebar lists them: Today, Tasks, the project tree, labels, saved
    /// filters, Blocks, the trash.
    pub fn sidebar(&self) -> Out<Sidebar> {
        out(Ok(Sidebar { entries: places::sidebar(&self.lumenna) }))
    }
}

/// What a place is called while it is shown.
#[wasm_bindgen(js_name = placeTitle)]
pub fn place_title(place: Ts<Place>) -> Result<String, JsError> {
    Ok(place.to_rust()?.title())
}

/// The query a place's task list starts from.
#[wasm_bindgen(js_name = placeQuery)]
pub fn place_query(place: Ts<Place>) -> Result<String, JsError> {
    Ok(place.to_rust()?.query())
}

/// What a task added while a place is shown starts with, so it lands there.
#[wasm_bindgen(js_name = placeQuickAddPrefix)]
pub fn place_quick_add_prefix(place: Ts<Place>) -> Result<String, JsError> {
    Ok(place.to_rust()?.quick_add_prefix())
}

/// A row's line: its title, then its value and the states that mean something — with
/// `completed` left out where a checkbox says it.
#[wasm_bindgen(js_name = rowText)]
pub fn row_text(row: Ts<lumenna_surface::RowView>, checkbox: bool) -> Result<String, JsError> {
    Ok(speech::row(&row.to_rust()?, checkbox))
}

/// A row in the trash, where saying `deleted` on every one is noise.
#[wasm_bindgen(js_name = trashedText)]
pub fn trashed_text(row: Ts<lumenna_surface::RowView>) -> Result<String, JsError> {
    Ok(speech::trashed(&row.to_rust()?))
}

/// A task's state as its details show it.
#[wasm_bindgen(js_name = taskStateText)]
pub fn task_state_text(task: Ts<TaskDetail>) -> Result<String, JsError> {
    Ok(speech::task_state(&task.to_rust()?))
}

/// A completion as offered: its name first, then its kind — "Work, project".
#[wasm_bindgen(js_name = candidateText)]
pub fn candidate_text(candidate: Ts<lumenna_surface::Candidate>) -> Result<String, JsError> {
    Ok(speech::candidate(&candidate.to_rust()?))
}

/// What a result says: its announcement, then each notice, as a sentence.
#[wasm_bindgen(js_name = announcementText)]
pub fn announcement_text(announcement: &str, notices: Vec<String>) -> String {
    speech::sentence(&speech::announcement(announcement, &notices))
}
