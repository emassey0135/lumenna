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

use boltffi::export;
use lumenna_host::Lumenna;
use lumenna_records::LumennaError;
use lumenna_app::words::ShellWords;
// Imported, not named by their paths, for BoltFFI as above.

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

/// What automatic backups to a folder do, in a sentence (`lumenna_host::folder_backup_words`).
#[export]
pub fn folder_backup_words() -> String {
    lumenna_host::folder_backup_words()
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

/// Where backups go while a folder is chosen and allowed.
#[export]
pub fn backing_up_to_folder(folder: String) -> String {
    lumenna_records::fields::backing_up_to_folder(&folder)
}

/// A folder that must be allowed again before this browser backs up to it.
#[export]
pub fn needs_asking_again(folder: String) -> String {
    lumenna_records::fields::needs_asking_again(lumenna_app::words::ThisDevice::Browser.itself(), &folder)
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
    lumenna_records::words::reminders_on_here(on, lumenna_app::words::ThisDevice::Browser.itself())
}

/// The field naming this browser for pairing.
#[export]
pub fn device_name_label() -> String {
    lumenna_records::words::device_name_label(lumenna_app::words::ThisDevice::Browser.itself())
}

/// Where the level changes among rows shown at `depths`, in order: "level 2", or empty.
#[export]
pub fn levels(depths: Vec<u32>) -> Vec<String> {
    lumenna_app::lines::levels(&depths).into_iter().map(Option::unwrap_or_default).collect()
}
/// Every fixed word the web draws, in its sentence case, as a browser calls itself: asked for
/// once, as it starts.
#[export]
pub fn shell_words() -> ShellWords {
    lumenna_app::words::shell_words(lumenna_app::Capitals::Sentence, lumenna_app::words::ThisDevice::Browser)
}
