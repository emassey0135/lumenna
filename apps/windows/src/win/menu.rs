//! The menu bar and its shortcuts: a native `HMENU` and a standard accelerator table.
//!
//! Every command is in the menu bar with its shortcut written beside it, because the menu
//! bar is how a Windows user — and a screen reader user above all, with Alt — finds out what
//! an app can do. The mnemonics avoid letters the main window's fields use, since the dialog
//! manager gives Alt+letter to a field before the menu bar.
//!
//! Some shortcuts are written in the menu but not in the accelerator table, because the
//! control in focus answers them itself: Space and Delete on a list, and the edit commands
//! in a text field. Taking them as accelerators would take them from the controls.

use windows::Win32::UI::Input::KeyboardAndMouse::{VK_F5, VK_F6, VK_NEXT, VK_OEM_COMMA, VK_PRIOR};
use windows::Win32::UI::WindowsAndMessaging::{
    ACCEL, ACCEL_VIRT_FLAGS, AppendMenuW, CreateAcceleratorTableW, CreateMenu, CreatePopupMenu,
    FCONTROL, FSHIFT, FVIRTKEY, HACCEL, HMENU, MF_POPUP, MF_SEPARATOR, MF_STRING,
};
use windows::core::HSTRING;

// Never 1 or 2: the dialog manager sends those for Enter and Escape.
pub const NEW_TASK: u16 = 100;
pub const NEW_BLOCK: u16 = 101;
pub const NEW_PROJECT: u16 = 102;
pub const NEW_LABEL: u16 = 103;
pub const NEW_FILTER: u16 = 104;
pub const SYNC_NOW: u16 = 105;
pub const BACK_UP: u16 = 106;
pub const CLOSE_WINDOW: u16 = 107;
pub const EXIT: u16 = 108;
pub const EXPORT_IMPORT: u16 = 109;
pub const SETTINGS: u16 = 111;
pub const RESTORE_BACKUP: u16 = 112;

pub const UNDO: u16 = 120;
pub const REDO: u16 = 121;
pub const CUT: u16 = 122;
pub const COPY: u16 = 123;
pub const PASTE: u16 = 124;
pub const SELECT_ALL: u16 = 125;
pub const FILTER: u16 = 126;

pub const GO_TODAY: u16 = 140;
pub const GO_TASKS: u16 = 141;
pub const GO_BLOCKS: u16 = 142;
pub const GO_TRASH: u16 = 143;
pub const NEXT_PANE: u16 = 144;
pub const PREVIOUS_PANE: u16 = 145;

pub const MARK_DONE: u16 = 160;
pub const OPEN_TASK: u16 = 161;
pub const SAVE_TASK: u16 = 162;
pub const MOVE_TO_PROJECT: u16 = 163;
pub const MAKE_SUBTASK: u16 = 164;
pub const MOVE_TO_TOP: u16 = 165;
pub const TRASH_TASK: u16 = 166;
pub const RESTORE_TASK: u16 = 167;
pub const ERASE_TASK: u16 = 168;
pub const PUT_IN_BLOCK: u16 = 169;
pub const WAIT_FOR: u16 = 170;
/// The first of a run of commands, one per task the selected one waits for: "Stop Waiting for …".
pub const STOP_WAITING: u16 = 900;

pub const PREVIOUS_DAY: u16 = 180;
pub const NEXT_DAY: u16 = 181;
pub const GO_TO_NOW: u16 = 182;
pub const GO_TO_DAY: u16 = 183;

pub const SHOW_WINDOW: u16 = 200;
pub const ABOUT: u16 = 201;

/// The menu bar: each menu's title, and its items as `(command, text)`, where command 0 is a
/// separator. A table, so a test can check it: no item twice, no mnemonic letter twice.
pub const MENUS: [(&str, &[(u16, &str)]); 6] = [
    ("&File", &[
        (NEW_TASK, "&New Task...\tCtrl+N"),
        (NEW_BLOCK, "New &Block...\tCtrl+Shift+N"),
        (NEW_PROJECT, "New &Project..."),
        (NEW_LABEL, "New &Label..."),
        (NEW_FILTER, "New Saved &Filter..."),
        (0, ""),
        (SYNC_NOW, "&Sync Now\tF5"),
        (BACK_UP, "Back &Up Now"),
        (RESTORE_BACKUP, "&Restore From a Backup..."),
        (EXPORT_IMPORT, "&Export and Import..."),
        (0, ""),
        (SETTINGS, "Se&ttings...\tCtrl+,"),
        (0, ""),
        (CLOSE_WINDOW, "&Close Window\tCtrl+W"),
        (EXIT, "E&xit\tCtrl+Q"),
    ]),
    ("&Edit", &[
        (UNDO, "&Undo\tCtrl+Z"),
        (REDO, "&Redo\tCtrl+Y"),
        (0, ""),
        (CUT, "Cu&t\tCtrl+X"),
        (COPY, "&Copy\tCtrl+C"),
        (PASTE, "&Paste\tCtrl+V"),
        (SELECT_ALL, "Select &All\tCtrl+A"),
        (0, ""),
        (FILTER, "&Filter Tasks\tCtrl+F"),
    ]),
    ("&View", &[
        (GO_TODAY, "&Today\tCtrl+1"),
        (GO_TASKS, "T&asks\tCtrl+2"),
        (GO_BLOCKS, "&Blocks\tCtrl+3"),
        (GO_TRASH, "T&rash\tCtrl+4"),
        (0, ""),
        (NEXT_PANE, "&Next Pane\tF6"),
        (PREVIOUS_PANE, "&Previous Pane\tShift+F6"),
    ]),
    ("&Task", &[
        (MARK_DONE, "&Mark Done\tCtrl+K"),
        (OPEN_TASK, "&Edit Details\tEnter"),
        (SAVE_TASK, "&Save Changes\tCtrl+S"),
        (0, ""),
        (PUT_IN_BLOCK, "Put in a &Block...\tCtrl+B"),
        (MOVE_TO_PROJECT, "Move to &Project...\tCtrl+Shift+M"),
        (MAKE_SUBTASK, "Make S&ubtask Of..."),
        (MOVE_TO_TOP, "Move to &Top Level"),
        (WAIT_FOR, "&Wait For..."),
        (0, ""),
        (TRASH_TASK, "Move to T&rash\tDelete"),
        (RESTORE_TASK, "Rest&ore From Trash"),
        (ERASE_TASK, "&Delete from Trash..."),
    ]),
    ("&Day", &[
        (PREVIOUS_DAY, "&Previous Day\tCtrl+Page Up"),
        (NEXT_DAY, "&Next Day\tCtrl+Page Down"),
        (GO_TO_NOW, "Go to N&ow\tCtrl+T"),
        (GO_TO_DAY, "&Go to Day...\tCtrl+G"),
        (0, ""),
        (NEW_BLOCK, "&Add Block...\tCtrl+Shift+N"),
    ]),
    ("&Help", &[(ABOUT, "&About Lumenna")]),
];

/// Builds the menu bar.
pub fn bar() -> HMENU {
    unsafe {
        let bar = CreateMenu().unwrap_or_default();
        for (title, items) in MENUS {
            let popup = popup(items);
            let _ = AppendMenuW(bar, MF_POPUP, popup.0 as usize, &HSTRING::from(title));
        }
        bar
    }
}

/// A popup menu of `(command, text)`, where command 0 is a separator.
pub fn popup(items: &[(u16, &str)]) -> HMENU {
    unsafe {
        let menu = CreatePopupMenu().unwrap_or_default();
        for (id, text) in items {
            if *id == 0 {
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            } else {
                let _ = AppendMenuW(menu, MF_STRING, usize::from(*id), &HSTRING::from(*text));
            }
        }
        menu
    }
}

/// The shortcuts the window answers wherever focus is in it.
pub fn accelerators() -> HACCEL {
    let control = FVIRTKEY | FCONTROL;
    let key = |flags: ACCEL_VIRT_FLAGS, key: u16, cmd: u16| ACCEL { fVirt: flags, key, cmd };
    let letter = |c: char| c as u16;
    let table = [
        key(control, letter('N'), NEW_TASK),
        key(control | FSHIFT, letter('N'), NEW_BLOCK),
        key(FVIRTKEY, VK_F5.0, SYNC_NOW),
        key(control, letter('W'), CLOSE_WINDOW),
        key(control, letter('Q'), EXIT),
        key(control, letter('Z'), UNDO),
        key(control, letter('Y'), REDO),
        key(control, letter('F'), FILTER),
        key(control, letter('1'), GO_TODAY),
        key(control, letter('2'), GO_TASKS),
        key(control, letter('3'), GO_BLOCKS),
        key(control, letter('4'), GO_TRASH),
        key(FVIRTKEY, VK_F6.0, NEXT_PANE),
        key(FVIRTKEY | FSHIFT, VK_F6.0, PREVIOUS_PANE),
        key(control, letter('K'), MARK_DONE),
        key(control, letter('S'), SAVE_TASK),
        key(control | FSHIFT, letter('M'), MOVE_TO_PROJECT),
        key(control, letter('B'), PUT_IN_BLOCK),
        key(control, VK_OEM_COMMA.0, SETTINGS),
        key(control, VK_PRIOR.0, PREVIOUS_DAY),
        key(control, VK_NEXT.0, NEXT_DAY),
        key(control, letter('T'), GO_TO_NOW),
        key(control, letter('G'), GO_TO_DAY),
    ];
    unsafe { CreateAcceleratorTableW(&table).unwrap_or_default() }
}

#[cfg(test)]
mod tests {
    use super::MENUS;

    /// The letter after `&`, which Alt and that letter reaches.
    fn mnemonic(text: &str) -> Option<char> {
        text.split_once('&').and_then(|(_, rest)| rest.chars().next()).map(|c| c.to_ascii_lowercase())
    }

    #[test]
    fn no_menu_lists_anything_twice() {
        for (title, items) in MENUS {
            let mut seen: Vec<&str> = Vec::new();
            for (id, text) in items.iter().filter(|(id, _)| *id != 0) {
                let name = text.split('\t').next().unwrap_or(text);
                assert!(!seen.contains(&name), "{title} lists {name} twice (command {id})");
                seen.push(name);
            }
        }
    }

    #[test]
    fn no_two_items_in_a_menu_share_a_letter() {
        for (title, items) in MENUS {
            let mut seen: Vec<char> = Vec::new();
            for (_, text) in items.iter().filter(|(id, _)| *id != 0) {
                let letter = mnemonic(text).unwrap_or_else(|| panic!("{text} in {title} has no letter"));
                assert!(!seen.contains(&letter), "{text} in {title} shares {letter} with another item");
                seen.push(letter);
            }
        }
    }

    #[test]
    fn no_two_menus_share_a_letter() {
        let letters: Vec<char> = MENUS.iter().filter_map(|(title, _)| mnemonic(title)).collect();
        let mut unique = letters.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), letters.len(), "{letters:?}");
    }
}
