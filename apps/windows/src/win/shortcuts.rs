//! The shortcuts from anywhere (§16.2), as this PC keeps them: changeable, or off, in the
//! current user's registry — this PC's own, as the Mac keeps its in that Mac's defaults.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};

use super::system;
use crate::shortcut::{Kind, Shortcut};

const KEY: &str = "Software\\Lumenna\\Shortcuts";

/// What a shortcut is now: its standard keys until the person changes them, or `None` when
/// they turned it off.
pub fn current(kind: Kind) -> Option<Shortcut> {
    match system::read_u32(KEY, kind.value_name()) {
        None => Some(kind.standard()),
        Some(value) => Shortcut::decode(value),
    }
}

/// How it is described in Settings: its keys, or "Off".
pub fn describe(kind: Kind) -> String {
    current(kind).map_or_else(|| "Off".to_owned(), Shortcut::describe)
}

fn register(main: HWND, kind: Kind, shortcut: Shortcut) -> bool {
    let modifiers = HOT_KEY_MODIFIERS(shortcut.modifiers()) | MOD_NOREPEAT;
    unsafe { RegisterHotKey(Some(main), kind.id(), modifiers, u32::from(shortcut.key)).is_ok() }
}

fn unregister(main: HWND, kind: Kind) {
    unsafe {
        let _ = UnregisterHotKey(Some(main), kind.id());
    }
}

/// Registers every shortcut that is on, returning the descriptions of those another program
/// already has.
pub fn register_all(main: HWND) -> Vec<String> {
    Kind::ALL
        .into_iter()
        .filter_map(|kind| current(kind).filter(|shortcut| !register(main, kind, *shortcut)))
        .map(Shortcut::describe)
        .collect()
}

pub fn unregister_all(main: HWND) {
    for kind in Kind::ALL {
        unregister(main, kind);
    }
}

/// Changes a shortcut, or turns it off with `None`, and keeps it. A combination another
/// program already has is refused, and the old one stays.
pub fn change(main: HWND, kind: Kind, to: Option<Shortcut>) -> Result<String, String> {
    unregister(main, kind);
    match to {
        Some(shortcut) => {
            if !register(main, kind, shortcut) {
                if let Some(old) = current(kind) {
                    register(main, kind, old);
                }
                return Err(format!("Another program already uses {}. Choose other keys.", shortcut.describe()));
            }
            system::write_u32(KEY, kind.value_name(), shortcut.encode());
            Ok(format!("{} is {}", kind.name(), shortcut.describe()))
        }
        None => {
            system::write_u32(KEY, kind.value_name(), 0);
            Ok(format!("{} is off", kind.name()))
        }
    }
}

/// Which shortcut a `WM_HOTKEY` is.
pub fn kind_of(id: i32) -> Option<Kind> {
    Kind::ALL.into_iter().find(|kind| kind.id() == id)
}
