//! Lumenna's core for the web client: the store and the app beside it, exported to JavaScript.
//!
//! The browser runs the same core every other client does — the same store, SQLite and
//! Automerge, the same parsers, the same rules, the same app (`lumenna-app`) — compiled to
//! WebAssembly. Nothing here decides anything: [`app`] runs the app over the store, and the
//! rest is the wording the desktop apps share, for each row's line in the browser's own clock.
//! Records are typed in TypeScript by `tsify`, derived from the crates' own: reshaping one
//! breaks the web build as it breaks every other.
//!
//! The store lives in OPFS through SQLite's sync access handles, which only a dedicated
//! worker may use: this module is loaded by the app's worker, never by the page itself.
//! Values cross as `tsify::Ts`, converted inside each function, so a malformed value is an
//! ordinary error rather than a leak at the boundary.

use std::path::Path;

mod app;
mod clock;

use clock::Browser;
use lumenna_desktop::speech::{self, Clock};
use lumenna_host::{Choice, Lumenna, LumennaError, PairingWords, TaskDetail};
use serde::Serialize;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

/// What every export returns: the record, or the core's sentence for what went wrong.
type Out<T> = Result<Ts<T>, JsError>;


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
    lumenna: std::sync::Arc<Lumenna>,
    syncing: std::rc::Rc<app::Syncing>,
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
        Ok(Core { lumenna: std::sync::Arc::new(lumenna), syncing: std::rc::Rc::default() })
    }

    /// A number that moves whenever another tab or process has written: what a view
    /// compares to know it should read again.
    #[wasm_bindgen(js_name = outsideVersion)]
    pub fn outside_version(&self) -> Result<f64, JsError> {
        // A JavaScript number holds this exactly: it is SQLite's data_version, a counter.
        Ok(self.lumenna.outside_version().map_err(error)? as f64)
    }

}

// ---------------------------------------------------------------------------------------
// The day, worded as the desktop apps word it, in this browser's times and days.
// ---------------------------------------------------------------------------------------

/// A block on the day: its times, name, length, kind, and what is in it.
#[wasm_bindgen(js_name = blockText)]
pub fn block_text(block: Ts<lumenna_host::PlanBlock>) -> Result<String, JsError> {
    Ok(speech::block(&block.to_rust()?, &Browser))
}

/// A sitting: a task in a block for one session.
#[wasm_bindgen(js_name = sittingText)]
pub fn sitting_text(sitting: Ts<lumenna_host::PlanAssignment>) -> Result<String, JsError> {
    Ok(speech::sitting(&sitting.to_rust()?))
}

/// Free time, which a timeline shows by empty space and a list has to say: its title and
/// details as the core words them, then its span in this browser's times.
#[wasm_bindgen(js_name = freeText)]
pub fn free_text(title: &str, details: Vec<String>, start: &str, end: &str) -> String {
    speech::free_time(title, &details, start, end, &Browser)
}

/// Where the present falls: the core's title for it, then the time in this browser's clock.
#[wasm_bindgen(js_name = nowText)]
pub fn now_text(title: &str, time: &str) -> String {
    format!("{title}, {}", Browser.time(time))
}

/// A repeating block cancelled for this day alone.
#[wasm_bindgen(js_name = cancelledText)]
pub fn cancelled_text(block: Ts<lumenna_host::CancelledBlock>) -> Result<String, JsError> {
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

/// Every sentence and button of pairing. A browser calls itself "this browser", and cannot
/// be found on a network by itself: it pairs by code.
#[wasm_bindgen(js_name = pairingWords)]
pub fn pairing_words() -> Out<PairingWords> {
    js(&lumenna_host::pairing_words("this browser".to_owned(), false))
}

/// Fixed text — a button, a question — in sentence case, the web's: never a name.
#[wasm_bindgen(js_name = sentenceCase)]
pub fn sentence_case(text: String) -> String {
    lumenna_host::sentence_case(text)
}

// ---------------------------------------------------------------------------------------
// The places and wording the desktop apps share (`crates/desktop`), so the web says what
// they say rather than a copy of it.
// ---------------------------------------------------------------------------------------

/// A row's line: its title, when it is due in the browser's clock, its value and the states
/// that mean something — with `completed` left out where a checkbox says it.
#[wasm_bindgen(js_name = rowText)]
pub fn row_text(row: Ts<lumenna_host::RowView>, checkbox: bool) -> Result<String, JsError> {
    Ok(speech::row(&row.to_rust()?, checkbox, &Browser))
}

/// A row in the trash, where saying `deleted` on every one is noise.
#[wasm_bindgen(js_name = trashedText)]
pub fn trashed_text(row: Ts<lumenna_host::RowView>) -> Result<String, JsError> {
    Ok(speech::trashed(&row.to_rust()?, &Browser))
}

/// A task's state as its details show it.
#[wasm_bindgen(js_name = taskStateText)]
pub fn task_state_text(task: Ts<TaskDetail>) -> Result<String, JsError> {
    Ok(speech::task_state(&task.to_rust()?))
}

/// A completion as offered: its name first, then its kind — "Work, project".
#[wasm_bindgen(js_name = candidateText)]
pub fn candidate_text(candidate: Ts<lumenna_host::Candidate>) -> Result<String, JsError> {
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
pub fn device_text(device: Ts<lumenna_host::DeviceView>) -> Result<String, JsError> {
    Ok(lumenna_desktop::devices::line(&device.to_rust()?, jiff::Timestamp::now()))
}

/// One keyboard command the desktop apps share (`lumenna_desktop::keys`), in the web's
/// sentence case.
#[derive(Serialize, serde::Deserialize, Tsify)]
pub struct Shortcut {
    /// What the web matches its own binding to: "undo".
    pub id: String,
    /// What it does: "Mark done or not done".
    pub title: String,
    /// The keys as a person reads them, the first the one to show.
    pub keys: Vec<String>,
}

/// One heading of the keyboard help, and its commands in order.
#[derive(Serialize, serde::Deserialize, Tsify)]
pub struct ShortcutGroup {
    /// Which group: "file", "edit", "view", "task", "day", "lists", "help".
    pub id: String,
    /// Its heading: "In a list".
    pub title: String,
    /// Its commands, in the order a help lists them.
    pub shortcuts: Vec<Shortcut>,
}

/// The keyboard help's groups.
#[derive(Serialize, serde::Deserialize, Tsify)]
pub struct Shortcuts {
    /// Every group, in the menus' order, then lists, then help.
    pub groups: Vec<ShortcutGroup>,
}

/// The keyboard commands every desktop app shares, grouped as their menus are, so the web's
/// help names a command as theirs do. The web lists only those it binds, and adds its own.
#[wasm_bindgen(js_name = keyboardShortcuts)]
pub fn keyboard_shortcuts() -> Out<Shortcuts> {
    use lumenna_desktop::keys::{self, Group};
    let groups = Group::ALL
        .iter()
        .map(|&group| ShortcutGroup {
            id: match group {
                Group::File => "file",
                Group::Edit => "edit",
                Group::View => "view",
                Group::Task => "task",
                Group::Day => "day",
                Group::Lists => "lists",
                Group::Help => "help",
            }
            .to_owned(),
            title: lumenna_host::sentence_case(group.title().to_owned()),
            shortcuts: keys::in_group(group)
                .map(|shortcut| Shortcut {
                    id: shortcut.id.to_owned(),
                    title: lumenna_host::sentence_case(shortcut.title.to_owned()),
                    keys: shortcut.keys.iter().map(|&key| key.to_owned()).collect(),
                })
                .collect(),
        })
        .collect();
    js(&Shortcuts { groups })
}
