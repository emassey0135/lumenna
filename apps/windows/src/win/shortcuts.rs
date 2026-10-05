//! The shortcuts from anywhere, as this PC keeps them: changeable, or off, in the
//! current user's registry — this PC's own, as the Mac keeps its in that Mac's defaults.

use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetClassNameW};
use windows::core::BOOL;

use super::system;
use super::tray::CLASS_PREFIX;
use crate::shortcut::{self, Kind, Shortcut};

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

/// Whether this copy takes the shortcuts at all: not when started with `--no-shortcuts`.
static TAKEN_HERE: AtomicBool = AtomicBool::new(true);

/// Leaves the shortcuts to another copy: they are still shown and kept, but not registered.
pub fn leave_to_another_copy() {
    TAKEN_HERE.store(false, Ordering::Relaxed);
}

fn register(main: HWND, kind: Kind, shortcut: Shortcut) -> bool {
    if !TAKEN_HERE.load(Ordering::Relaxed) {
        return true;
    }
    let modifiers = HOT_KEY_MODIFIERS(shortcut.modifiers()) | MOD_NOREPEAT;
    unsafe { RegisterHotKey(Some(main), kind.id(), modifiers, u32::from(shortcut.key)).is_ok() }
}

/// Whether another copy of Lumenna has a window open — one on another profile, since a
/// second copy on this one only shows the first. Windows does not say who holds a shortcut;
/// when Lumenna is open twice, it is all but certainly the other copy.
pub fn another_copy_open(main: HWND) -> bool {
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        unsafe {
            let search = &mut *(lparam.0 as *mut (HWND, bool));
            let mut class = [0u16; 64];
            let length = GetClassNameW(hwnd, &mut class);
            let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
            if hwnd != search.0 && class.starts_with(CLASS_PREFIX) {
                search.1 = true;
                return false.into();
            }
            true.into()
        }
    }
    let mut search = (main, false);
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM((&raw mut search) as isize));
    }
    search.1
}

fn unregister(main: HWND, kind: Kind) {
    unsafe {
        let _ = UnregisterHotKey(Some(main), kind.id());
    }
}

/// Registers every shortcut that is on, returning the descriptions of those something else
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
                return Err(shortcut::refused(&shortcut.describe(), another_copy_open(main)));
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
