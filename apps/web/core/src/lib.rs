//! Lumenna's core for the web client: the store and the app beside it, exported to TypeScript
//! through BoltFFI, as Android's is to Kotlin.
//!
//! The browser runs the same core every other client does — the same store, SQLite and
//! Automerge, the same parsers, the same rules, the same app (`lumenna-app`) — compiled to
//! WebAssembly. Nothing here decides anything: [`app`] runs the app over the store and words its
//! view in the browser's clock; the rest is what a shell asks outside a turn. Records are typed
//! from the crates' own, so reshaping one breaks the web build as it breaks every other.
//!
//! The store lives in OPFS through SQLite's sync access handles, which only a dedicated worker
//! may use: this module is loaded by the app's worker, never by the page itself.

use std::path::Path;
// Imported, not named by its path: BoltFFI finds another crate's type only so.
use lumenna_app::keys::KeyGroup;

use boltffi::export;
use lumenna_host::Lumenna;
use lumenna_records::{LumennaError, PickerWords};
// Imported, not named by their paths, for BoltFFI as above.
use lumenna_records::fields::{DetailsWords, FolderWords, QuickAddWords};
use lumenna_records::words::AppMetadata;

mod app;
mod clock;

pub use app::{App, Listener, Turn, open};

/// The store called `name`: SQLite in OPFS, durable across reloads, in the browser's persistent
/// storage when it has been granted.
async fn open_store(name: &str) -> Result<Lumenna, LumennaError> {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        console_error_panic_hook::set_once();
        let pool = sqlite_wasm_vfs::sahpool::OpfsSAHPoolCfgBuilder::new().directory("lumenna").build();
        sqlite_wasm_vfs::sahpool::install::<sqlite_wasm_rs::WasmOsCallback>(&pool, true)
            .await
            .map_err(|e| LumennaError::new(format!("This browser's storage could not be opened: {e}")))?;
    }
    Lumenna::open_at(Path::new(&format!("/{name}")))
}


/// Fixed text — a button, a question — in sentence case, the web's: never a name.
#[export]
pub fn sentence_case(text: String) -> String {
    lumenna_host::sentence_case(text)
}

/// The keyboard commands every desktop app shares (`lumenna_app::keys`), grouped as their
/// menus are, in the web's sentence case, so the web's help names a command as theirs do. The
/// web lists only those it binds, and adds its own.
#[export]
pub fn keyboard_shortcuts() -> Vec<KeyGroup> {
    lumenna_app::keys::grouped(true)
}

/// What automatic backups to a folder do, in a sentence (`lumenna_host::folder_backup_words`).
#[export]
pub fn folder_backup_words() -> String {
    lumenna_host::folder_backup_words()
}

/// What the web's own day and time pickers say, beside the line each fills, in sentence case.
#[export]
pub fn picker_words() -> PickerWords {
    let words = lumenna_records::picker_words();
    let case = lumenna_records::sentence_case;
    PickerWords {
        day: case(words.day),
        time: case(words.time),
        choose: case(words.choose),
        cancel: case(words.cancel),
        hours: case(words.hours),
        minutes: case(words.minutes),
    }
}

/// What quick add and a list's filter say, every label and button, in sentence case.
#[export]
pub fn quick_add_words() -> QuickAddWords {
    lumenna_records::fields::quick_add_words(true)
}

/// What the task details say around their form, every label and button, in sentence case.
#[export]
pub fn details_words() -> DetailsWords {
    lumenna_records::fields::details_words(true)
}

/// What is said once a backup is taken into a folder of the person's: "Backed up to Documents
/// as lumenna-2026-10-26.lumbak".
#[export]
pub fn backed_up_to(folder: String, name: String) -> String {
    lumenna_records::fields::backed_up_to(&folder, &name)
}

/// What is said when a backup failed, its reason after where there is one.
#[export]
pub fn backup_failed(reason: String) -> String {
    lumenna_records::words::backup_failed(&reason)
}

/// What a browser calls itself when it pairs, from the names its user agent gives: "Firefox on
/// Windows".
#[export]
pub fn browser_device_name(browser: String, system: String) -> String {
    lumenna_records::words::browser_device_name(&browser, &system)
}

/// How many a pick offers: "1 choice", "3 choices".
#[export]
pub fn choice_count(count: u32) -> String {
    lumenna_records::words::choice_count(count as usize)
}

/// What a pick with nothing in it says.
#[export]
pub fn nothing_to_choose() -> String {
    lumenna_records::words::nothing_to_choose()
}

/// What backing up into a folder of the person's says, in this browser's words and sentence case.
#[export]
pub fn folder_words() -> FolderWords {
    lumenna_records::fields::folder_words("this browser", true)
}

/// Where backups go while a folder is chosen and allowed.
#[export]
pub fn backing_up_to_folder(folder: String) -> String {
    lumenna_records::fields::backing_up_to_folder(&folder)
}

/// A folder that must be allowed again before this browser backs up to it.
#[export]
pub fn needs_asking_again(folder: String) -> String {
    lumenna_records::fields::needs_asking_again("this browser", &folder)
}

/// The button that allows backups into `folder` again, in sentence case.
#[export]
pub fn allow_backups_to(folder: String) -> String {
    lumenna_records::fields::allow_backups_to(&folder, true)
}

/// Said once backups go into `folder`.
#[export]
pub fn backing_up_to(folder: String) -> String {
    lumenna_records::fields::backing_up_to(&folder)
}

/// Whether reminders are given in this browser, said once changed.
#[export]
pub fn reminders_on_here(on: bool) -> String {
    lumenna_records::words::reminders_on_here(on, "this browser")
}

/// What a tab adds to the reminders setting's hint: they come only while it is open.
#[export]
pub fn reminders_while_open() -> String {
    lumenna_records::words::reminders_while_open()
}

/// What a browser says when it may clear Lumenna's data short of space.
#[export]
pub fn storage_may_be_cleared() -> String {
    lumenna_records::words::storage_may_be_cleared()
}

/// What shows while the store opens.
#[export]
pub fn opening() -> String {
    lumenna_records::words::opening()
}

/// What a second tab says while another has the store open.
#[export]
pub fn open_elsewhere() -> String {
    lumenna_records::words::open_elsewhere()
}

/// The list of reminders due, as its heading names it, in sentence case.
#[export]
pub fn reminders_due() -> String {
    lumenna_records::words::reminders_due(true)
}

/// The day's own actions, as a menu button names them, in sentence case.
#[export]
pub fn day_actions() -> String {
    lumenna_records::words::day_actions(true)
}

/// A place's own actions, as a menu button names them, in sentence case.
#[export]
pub fn place_actions() -> String {
    lumenna_records::words::place_actions(true)
}

/// What the devices list says Delete does there.
#[export]
pub fn delete_unpairs() -> String {
    lumenna_records::words::delete_unpairs()
}

/// The field naming this browser for pairing.
#[export]
pub fn device_name_label() -> String {
    lumenna_records::words::device_name_label("this browser")
}

/// What the field naming this browser is for.
#[export]
pub fn device_name_hint() -> String {
    lumenna_records::words::device_name_hint()
}

/// What is said when a pairing code could not be copied.
#[export]
pub fn copy_failed() -> String {
    lumenna_records::words::copy_failed()
}

/// What a pick says when answered with nothing chosen.
#[export]
pub fn choose_one_first() -> String {
    lumenna_records::words::choose_one_first()
}

/// A dialog's Cancel, in sentence case.
#[export]
pub fn cancel() -> String {
    lumenna_records::words::cancel(true)
}

/// A dialog's Close, in sentence case.
#[export]
pub fn close() -> String {
    lumenna_records::words::close(true)
}

/// What a line that completes says of it, after its own hint.
#[export]
pub fn completion_hint() -> String {
    lumenna_records::words::completion_hint()
}

/// Where the level changes among rows shown at `depths`, in order: "level 2", or empty.
#[export]
pub fn levels(depths: Vec<u32>) -> Vec<String> {
    lumenna_app::lines::levels(&depths).into_iter().map(Option::unwrap_or_default).collect()
}

/// What Lumenna is, from the workspace's one source.
#[export]
pub fn app_metadata() -> AppMetadata {
    lumenna_records::words::metadata()
}
