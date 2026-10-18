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

use boltffi::{data, export};
use lumenna_host::Lumenna;
use lumenna_records::{LumennaError, PairingWords};

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

/// Every sentence and button of pairing, in the web's capitals: its buttons and headings in
/// sentence case, its introduction the core's own sentence-case one. A browser calls itself
/// "this browser", and cannot be found on a network by itself: it pairs by code.
#[export]
pub fn pairing_words() -> PairingWords {
    let mut words = lumenna_host::pairing_words("this browser".to_owned(), false);
    for word in [&mut words.title, &mut words.wait, &mut words.join, &mut words.match_title, &mut words.match_yes, &mut words.match_no, &mut words.copy_code] {
        *word = lumenna_host::sentence_case(std::mem::take(word));
    }
    words.intro = std::mem::take(&mut words.intro_sentence);
    words
}

/// Fixed text — a button, a question — in sentence case, the web's: never a name.
#[export]
pub fn sentence_case(text: String) -> String {
    lumenna_host::sentence_case(text)
}

/// One keyboard command the desktop apps share (`lumenna_desktop::keys`), in the web's
/// sentence case.
#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    /// What the web matches its own binding to: "undo".
    pub id: String,
    /// What it does: "Mark done or not done".
    pub title: String,
    /// The keys as a person reads them, the first the one to show.
    pub keys: Vec<String>,
}

/// One heading of the keyboard help, and its commands in order.
#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutGroup {
    /// Which group: "file", "edit", "view", "task", "day", "lists", "help".
    pub id: String,
    /// Its heading: "In a list".
    pub title: String,
    /// Its commands, in the order a help lists them.
    pub shortcuts: Vec<Shortcut>,
}

/// The keyboard help's groups.
#[data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcuts {
    /// Every group, in the menus' order, then lists, then help.
    pub groups: Vec<ShortcutGroup>,
}

/// The keyboard commands every desktop app shares, grouped as their menus are, so the web's
/// help names a command as theirs do. The web lists only those it binds, and adds its own.
#[export]
pub fn keyboard_shortcuts() -> Shortcuts {
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
    Shortcuts { groups }
}
