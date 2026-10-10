//! Lumenna's core for the web client: the command surface, exported to JavaScript.
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

mod clock;
mod manage;
mod sync;

use clock::Browser;
use lumenna_surface::places::{self, Place, SidebarEntry};
use lumenna_desktop::speech::{self, Clock};
use lumenna_surface::{Action, Answer, Choice, Lumenna, LumennaError, Syntax, TaskDetail, TaskEdit, TaskFields};
use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// What every export returns: the record, or the core's sentence for what went wrong.
type Out<T> = Result<Ts<T>, JsError>;

fn out<T: Tsify + Serialize>(result: lumenna_surface::Result<T>) -> Out<T> {
    js(&result.map_err(error)?)
}

/// A record as the plain object its TypeScript type describes.
///
/// Not `Ts::from_rust`, which takes the serialiser's settings from the outermost type alone:
/// serde writes a `#[serde(flatten)]` record (`TaskShown`) as a map, and a map not marked
/// otherwise becomes a JavaScript `Map` — whose fields nothing reads, and which the core then
/// cannot read back. Here every map is an object, wherever it sits.
fn js<T: Tsify + Serialize>(value: &T) -> Out<T> {
    let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
    Ok(Ts::new_unchecked(value.serialize(&serializer)?))
}

/// The core's own sentence, which is written to be read out.
fn error(error: LumennaError) -> JsError {
    JsError::new(error.message())
}

/// One open store.
#[wasm_bindgen]
pub struct Core {
    lumenna: std::rc::Rc<Lumenna>,
    syncing: std::rc::Rc<sync::Syncing>,
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
        Ok(Core { lumenna: std::rc::Rc::new(lumenna), syncing: std::rc::Rc::default() })
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

    /// Every block series, by when it starts.
    #[wasm_bindgen(js_name = listBlocks)]
    pub fn list_blocks(&self) -> Out<lumenna_surface::Rows> {
        out(self.lumenna.list_blocks())
    }

    /// One block series, as an editor starts from it.
    #[wasm_bindgen(js_name = showBlock)]
    pub fn show_block(&self, id: &str) -> Out<lumenna_surface::BlockShown> {
        out(self.lumenna.show_block(id))
    }

    /// The surface's method of the same name, as `addBlock`.
    #[wasm_bindgen(js_name = addBlock)]
    pub fn add_block(&self, block: Ts<lumenna_surface::NewBlock>) -> Out<lumenna_surface::Change> {
        out(self.lumenna.add_block(block.to_rust()?))
    }

    /// Changes a block: every occurrence, or one day's alone.
    #[wasm_bindgen(js_name = editBlock)]
    pub fn edit_block(
        &self,
        id: &str,
        edit: Ts<lumenna_surface::BlockEdit>,
        scope: Ts<lumenna_surface::BlockScope>,
    ) -> Out<lumenna_surface::Change> {
        out(self.lumenna.edit_block(id, edit.to_rust()?, scope.to_rust()?))
    }

    /// Runs one of a record's actions with the answer to its question: what every row's
    /// menu, key and button does.
    pub fn act(&self, action: Ts<Action>, answer: Ts<Answer>) -> Out<lumenna_surface::Change> {
        out(self.lumenna.act(action.to_rust()?, answer.to_rust()?))
    }

    /// What a pick offers for `action`, or, when nothing, why.
    pub fn choices(&self, action: Ts<Action>) -> Out<lumenna_surface::Choices> {
        out(self.lumenna.choices(action.to_rust()?))
    }
}

// ---------------------------------------------------------------------------------------
// The day, worded as the desktop apps word it, in this browser's times and days.
// ---------------------------------------------------------------------------------------

/// A block on the day: its times, name, length, kind, and what is in it.
#[wasm_bindgen(js_name = blockText)]
pub fn block_text(block: Ts<lumenna_surface::PlanBlock>) -> Result<String, JsError> {
    Ok(speech::block(&block.to_rust()?, &Browser))
}

/// A sitting: a task in a block for one session.
#[wasm_bindgen(js_name = sittingText)]
pub fn sitting_text(sitting: Ts<lumenna_surface::PlanAssignment>) -> Result<String, JsError> {
    Ok(speech::sitting(&sitting.to_rust()?))
}

/// Free time, which a timeline shows by empty space and a list has to say.
#[wasm_bindgen(js_name = freeText)]
pub fn free_text(start: &str, end: &str, minutes: u32) -> String {
    speech::free(start, end, minutes, &Browser)
}

/// Where the present falls.
#[wasm_bindgen(js_name = nowText)]
pub fn now_text(time: &str) -> String {
    speech::now(time, &Browser)
}

/// A repeating block cancelled for this day alone.
#[wasm_bindgen(js_name = cancelledText)]
pub fn cancelled_text(block: Ts<lumenna_surface::CancelledBlock>) -> Result<String, JsError> {
    Ok(speech::cancelled(&block.to_rust()?, &Browser))
}

/// The day's first row: what a glance at a timeline gives.
#[wasm_bindgen(js_name = summaryText)]
pub fn summary_text(date: &str, summary: &str) -> String {
    speech::summary(date, summary, &Browser)
}

/// An ISO date as a person says it: "Today", or "Monday 5 October".
#[wasm_bindgen(js_name = dayText)]
pub fn day_text(iso: &str) -> String {
    Browser.day(iso)
}

/// `14:30` as this browser says it.
#[wasm_bindgen(js_name = timeText)]
pub fn time_text(clock: &str) -> String {
    Browser.time(clock)
}

/// A task's fields as a form starts from them.
#[wasm_bindgen(js_name = taskFields)]
pub fn task_fields(task: Ts<TaskDetail>) -> Out<TaskFields> {
    out(Ok(lumenna_surface::task_fields(task.to_rust()?)))
}

/// The priorities a task can have, as the details form offers them.
#[derive(Serialize, serde::Deserialize, Tsify)]
pub struct Priorities {
    /// Each one, highest first: `id` is the number.
    pub choices: Vec<Choice>,
}

/// The priorities a task can have, in the core's words.
#[wasm_bindgen]
pub fn priorities() -> Out<Priorities> {
    js(&Priorities { choices: lumenna_surface::priorities() })
}

/// What saving `fields` over `task` sends, or nothing if no field changed — only what
/// changed, so a concurrent edit on another device is not reverted.
#[wasm_bindgen(js_name = taskEdit)]
pub fn task_edit(task: Ts<TaskDetail>, fields: Ts<TaskFields>) -> Result<Option<Ts<TaskEdit>>, JsError> {
    lumenna_surface::task_edit(task.to_rust()?, fields.to_rust()?).map(|edit| js(&edit)).transpose()
}

// ---------------------------------------------------------------------------------------
// The places and wording the desktop apps share (`crates/desktop`), so the web says what
// they say rather than a copy of it.
// ---------------------------------------------------------------------------------------

/// The sidebar's rows, in order.
#[derive(Serialize, serde::Deserialize, Tsify)]
pub struct Sidebar {
    /// Each row, with what it is and how deep.
    pub entries: Vec<SidebarEntry>,
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

/// A row's line: its title, when it is due in the browser's clock, its value and the states
/// that mean something — with `completed` left out where a checkbox says it.
#[wasm_bindgen(js_name = rowText)]
pub fn row_text(row: Ts<lumenna_surface::RowView>, checkbox: bool) -> Result<String, JsError> {
    Ok(speech::row(&row.to_rust()?, checkbox, &Browser))
}

/// A row in the trash, where saying `deleted` on every one is noise.
#[wasm_bindgen(js_name = trashedText)]
pub fn trashed_text(row: Ts<lumenna_surface::RowView>) -> Result<String, JsError> {
    Ok(speech::trashed(&row.to_rust()?, &Browser))
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

/// Something a pick offers, as its line reads: a block as the desktop apps word one
/// ("Tomorrow, 9:00 AM to 11:00 AM, Deep work"), anything else its title and what tells it
/// apart.
#[wasm_bindgen(js_name = choiceText)]
pub fn choice_text(choice: Ts<Choice>) -> Result<String, JsError> {
    Ok(lumenna_desktop::speech::choice(&choice.to_rust()?, &Browser))
}

/// A paired device, as its line in the list reads: "Kitchen Mac, macos, last synced 5 minutes
/// ago" — sync status in words rather than an icon.
#[wasm_bindgen(js_name = deviceText)]
pub fn device_text(device: Ts<lumenna_surface::DeviceView>) -> Result<String, JsError> {
    Ok(lumenna_desktop::devices::line(&device.to_rust()?, jiff::Timestamp::now()))
}
