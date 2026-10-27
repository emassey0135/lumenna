//! The menu bar, the tray's menu and their shortcuts, from the core's tables (`keys::menus`,
//! `keys::tray`, `keys::SHARED`): a native `HMENU` and a standard accelerator table.
//!
//! Every command is in the menu bar with its shortcut written beside it, because the menu
//! bar is how a Windows user — and a screen reader user above all, with Alt — finds out what
//! an app can do. What is Windows' own here: each command's number, its access key (avoiding
//! letters the main window's fields use, since the dialog manager gives Alt+letter to a field
//! before the menu bar), Exit for Quit, the ellipsis on a command that asks first, the edit
//! commands of a text field, and the keys a control answers itself written beside their
//! commands — Enter, Delete, Alt+Enter.
//!
//! Some shortcuts are written in the menu but not in the accelerator table, because the
//! control in focus answers them itself: Space, Enter and Delete on a list, and the edit
//! commands in a text field. Taking them as accelerators would take them from the controls.

use lumenna_app::keys::{self, Group, MenuItem};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_F1, VK_NEXT, VK_OEM_COMMA, VK_PRIOR, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    ACCEL, ACCEL_VIRT_FLAGS, AppendMenuW, CreateAcceleratorTableW, CreateMenu, CreatePopupMenu, FALT, FCONTROL, FSHIFT,
    FVIRTKEY, HACCEL, HMENU, MF_POPUP, MF_SEPARATOR, MF_STRING,
};
use windows::core::HSTRING;

use crate::devices;

// The edit commands of a text field, Windows' own. Never 1 or 2: the dialog manager sends those
// for Enter and Escape.
pub const CUT: u16 = 122;
pub const COPY: u16 = 123;
pub const PASTE: u16 = 124;
pub const SELECT_ALL: u16 = 125;

/// Each of the core's commands as the menu sends it, by its id in `keys::COMMANDS`; the tray's
/// timer after them.
const NUMBERS: &[(&str, u16)] = &[
    ("new-task", 100),
    ("new-block", 101),
    ("new-project", 102),
    ("new-label", 103),
    ("new-filter", 104),
    ("sync-now", 105),
    ("back-up", 106),
    ("close-window", 107),
    ("quit", 108),
    ("export-import", 109),
    ("devices", 110),
    ("settings", 111),
    ("restore-backup", 112),
    ("properties", 113),
    ("undo", 120),
    ("redo", 121),
    ("filter", 126),
    ("go-today", 140),
    ("go-tasks", 141),
    ("go-blocks", 142),
    ("go-trash", 143),
    ("next-pane", 144),
    ("previous-pane", 145),
    ("go-history", 146),
    ("mark-done", 160),
    ("open-task", 161),
    ("save-task", 162),
    ("move-to-project", 163),
    ("make-subtask", 164),
    ("move-to-top", 165),
    ("trash-task", 166),
    ("restore-task", 167),
    ("erase-task", 168),
    ("put-in-block", 169),
    ("wait-for", 170),
    ("erase-permanently", 171),
    ("stop-waiting", 172),
    ("previous-day", 180),
    ("next-day", 181),
    ("go-to-now", 182),
    ("go-to-day", 183),
    ("plan-day", 184),
    ("replan", 185),
    ("what-now", 186),
    ("end-day", 187),
    ("clear-day", 188),
    ("show", 200),
    ("about", 201),
    ("keyboard-help", 202),
];

/// The tray's timer: the timer's control (`Device::toggle_timer`), named as it is now.
pub const TIMER: u16 = 210;

/// Each command's access key: the letter of its name marked, as Windows marks one.
const LETTERS: &[(&str, char)] = &[
    ("new-task", 'N'),
    ("new-block", 'B'),
    ("new-project", 'P'),
    ("new-label", 'L'),
    ("new-filter", 'F'),
    ("sync-now", 'S'),
    ("devices", 'D'),
    ("back-up", 'U'),
    ("restore-backup", 'R'),
    ("export-import", 'E'),
    ("properties", 'i'),
    ("settings", 't'),
    ("close-window", 'C'),
    ("quit", 'x'),
    ("undo", 'U'),
    ("redo", 'R'),
    ("filter", 'F'),
    ("go-today", 'T'),
    ("go-tasks", 'a'),
    ("go-blocks", 'B'),
    ("go-trash", 'r'),
    ("go-history", 'H'),
    ("next-pane", 'N'),
    ("previous-pane", 'P'),
    ("mark-done", 'M'),
    ("open-task", 'E'),
    ("save-task", 'S'),
    ("put-in-block", 'B'),
    ("move-to-project", 'P'),
    ("make-subtask", 'u'),
    ("move-to-top", 'T'),
    ("wait-for", 'W'),
    ("stop-waiting", 'g'),
    ("trash-task", 'r'),
    ("restore-task", 'o'),
    ("erase-task", 'D'),
    ("erase-permanently", 'a'),
    ("previous-day", 'P'),
    ("next-day", 'N'),
    ("go-to-now", 'o'),
    ("go-to-day", 'G'),
    ("plan-day", 'l'),
    ("replan", 'R'),
    ("what-now", 'W'),
    ("end-day", 'E'),
    ("clear-day", 'C'),
    ("keyboard-help", 'K'),
    ("about", 'A'),
    ("show", 'S'),
];

/// The tray's own letters, where the menu bar's would meet another of its items'.
const TRAY_LETTERS: &[(&str, char)] = &[("sync-now", 'y')];

/// The keys a control answers itself, written beside the command they run: Windows' own.
const CONTROLS_KEYS: &[(&str, &str)] = &[("open-task", "Enter"), ("trash-task", "Delete"), ("properties", "Alt+Enter")];

/// The menus' titles' access keys, by the core's menu id.
const MENU_LETTERS: &[(&str, char)] = &[("file", 'F'), ("edit", 'E'), ("view", 'V'), ("task", 'T'), ("day", 'D'), ("help", 'H')];

fn find<T: Copy>(table: &[(&str, T)], id: &str) -> Option<T> {
    table.iter().find(|(key, _)| *key == id).map(|(_, value)| *value)
}

/// The number the menu sends for the command `id`.
pub fn number(id: &str) -> Option<u16> {
    find(NUMBERS, id)
}

/// The command a menu's number stands for.
pub fn command(number: u16) -> Option<&'static str> {
    NUMBERS.iter().find(|(_, n)| *n == number).map(|(id, _)| *id)
}

/// A command's name as Windows words it: Windows says Exit where others say Quit.
pub fn windows_title<'a>(id: &str, title: &'a str) -> &'a str {
    if id == "quit" { "Exit" } else { title }
}

/// A button that runs a command, as its menu item names it: an ellipsis when it asks first, and
/// without the menu's access key or keys.
pub fn button(id: &str) -> String {
    let Some(command) = keys::command_of(id) else { return String::new() };
    let title = windows_title(id, command.title);
    if command.asks { format!("{title}...") } else { title.to_owned() }
}

/// A menu item's text: its name as Windows words it with its access key marked, an ellipsis when
/// it asks first, and its keys after a tab.
pub fn text(id: &str, title: &str, asks: bool, keys: &[String], letters: &[(&str, char)]) -> String {
    let title = windows_title(id, title);
    let letter = find(letters, id).or_else(|| find(LETTERS, id));
    let mut text = letter.map_or_else(|| title.replace('&', "&&"), |letter| devices::marked(title, letter, '&'));
    if asks {
        text.push_str("...");
    }
    let keys = keys.first().map(String::as_str).or_else(|| find(CONTROLS_KEYS, id));
    if let Some(keys) = keys {
        text.push('\t');
        text.push_str(keys);
    }
    text
}

/// A menu item's text from the core's item.
fn item_text(item: &MenuItem, letters: &[(&str, char)]) -> String {
    text(&item.id, &item.title, item.asks, &item.keys, letters)
}

/// The menu bar, as `(title, items)` where an item is `(command, text)` and command 0 a
/// separator: the core's menus, the Edit menu with a text field's own commands too.
pub fn menus() -> Vec<(String, Vec<(u16, String)>)> {
    keys::menus(false)
        .into_iter()
        .map(|menu| {
            let title = find(MENU_LETTERS, &menu.id).map_or_else(|| menu.title.clone(), |letter| devices::marked(&menu.title, letter, '&'));
            let mut items = Vec::new();
            for item in &menu.items {
                if item.id.is_empty() {
                    items.push((0, String::new()));
                    continue;
                }
                let Some(number) = number(&item.id) else { continue };
                items.push((number, item_text(item, &[])));
                // A text field's own edit commands, after Redo.
                if item.id == "redo" {
                    items.push((0, String::new()));
                    for (number, text) in [(CUT, "Cu&t\tCtrl+X"), (COPY, "&Copy\tCtrl+C"), (PASTE, "&Paste\tCtrl+V"), (SELECT_ALL, "Select &All\tCtrl+A")] {
                        items.push((number, text.to_owned()));
                    }
                }
            }
            (title, items)
        })
        .collect()
}

/// The tray's menu, as `(command, text)`: the core's, with the timer's control before Quit
/// (`timer`, its name now), as the phones' controls offer it.
pub fn tray(timer: Option<&str>) -> Vec<(u16, String)> {
    let mut items: Vec<(u16, String)> = Vec::new();
    for item in keys::tray(false) {
        if item.id.is_empty() {
            items.push((0, String::new()));
            continue;
        }
        if item.id == "quit"
            && let Some(timer) = timer
        {
            items.push((TIMER, devices::marked(timer, 'T', '&')));
            items.push((0, String::new()));
        }
        if let Some(number) = number(&item.id) {
            items.push((number, text(&item.id, &item.title, item.asks, &[], TRAY_LETTERS)));
        }
    }
    items
}

/// Builds the menu bar.
pub fn bar() -> HMENU {
    unsafe {
        let bar = CreateMenu().unwrap_or_default();
        for (title, items) in menus() {
            let items: Vec<(u16, &str)> = items.iter().map(|(id, text)| (*id, text.as_str())).collect();
            let popup = popup(&items);
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

/// The accelerator for keys as a person reads them ("Ctrl+Shift+N", "F5", "Ctrl+Page Up").
fn accelerator(keys: &str, command: u16) -> Option<ACCEL> {
    let mut flags = FVIRTKEY;
    let mut parts: Vec<&str> = keys.split('+').collect();
    let key = parts.pop()?;
    for modifier in parts {
        flags |= match modifier {
            "Ctrl" => FCONTROL,
            "Shift" => FSHIFT,
            "Alt" => FALT,
            _ => return None,
        };
    }
    let key = match key {
        "Comma" => VK_OEM_COMMA.0,
        "Page Up" => VK_PRIOR.0,
        "Page Down" => VK_NEXT.0,
        "Enter" => VK_RETURN.0,
        function if function.len() > 1 && function.starts_with('F') => VK_F1.0 + function[1..].parse::<u16>().ok()?.checked_sub(1)?,
        letter if letter.len() == 1 && letter.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) => u16::from(letter.as_bytes()[0]),
        _ => return None,
    };
    Some(ACCEL { fVirt: flags as ACCEL_VIRT_FLAGS, key, cmd: command })
}

/// Every accelerator: the shared keys of each command the window answers wherever focus is in
/// it, and Alt+Enter for Properties. A list's keys are its controls' own.
fn accelerators_table() -> Vec<ACCEL> {
    let mut table: Vec<ACCEL> = keys::SHARED
        .iter()
        .filter(|shortcut| shortcut.group != Group::Lists)
        .filter_map(|shortcut| number(shortcut.id).map(|command| (shortcut, command)))
        .flat_map(|(shortcut, command)| shortcut.keys.iter().filter_map(move |keys| accelerator(keys, command)))
        .collect();
    table.extend(number("properties").and_then(|command| accelerator("Alt+Enter", command)));
    table
}

/// The shortcuts the window answers wherever focus is in it.
pub fn accelerators() -> HACCEL {
    unsafe { CreateAcceleratorTableW(&accelerators_table()).unwrap_or_default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The letter after `&`, which Alt and that letter reaches.
    fn mnemonic(text: &str) -> Option<char> {
        text.split_once('&').and_then(|(_, rest)| rest.chars().next()).map(|c| c.to_ascii_lowercase())
    }

    #[test]
    fn no_menu_lists_anything_twice() {
        for (title, items) in menus() {
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
        let tray: Vec<(u16, String)> = tray(Some("Start Timer"));
        for (title, items) in menus().into_iter().chain([("tray".to_owned(), tray)]) {
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
        let letters: Vec<char> = menus().iter().filter_map(|(title, _)| mnemonic(title)).collect();
        let mut unique = letters.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), letters.len(), "{letters:?}");
    }

    #[test]
    fn every_command_the_core_lists_has_a_number_and_no_number_is_one_or_two() {
        for command in keys::COMMANDS {
            assert!(number(command.id).is_some_and(|n| n > 2), "{} has no number of its own", command.id);
        }
        let mut numbers: Vec<u16> = NUMBERS.iter().map(|(_, n)| *n).chain([CUT, COPY, PASTE, SELECT_ALL, TIMER]).collect();
        numbers.sort_unstable();
        let count = numbers.len();
        numbers.dedup();
        assert_eq!(numbers.len(), count, "two commands share a number");
    }

    #[test]
    fn every_shared_key_but_a_lists_is_an_accelerator() {
        let table = accelerators_table();
        for shortcut in keys::SHARED.iter().filter(|s| s.group != Group::Lists) {
            let command = number(shortcut.id).unwrap_or_else(|| panic!("{} has no number", shortcut.id));
            for keys in shortcut.keys {
                assert!(accelerator(keys, command).is_some(), "{keys} is not read");
            }
            assert!(table.iter().any(|accel| accel.cmd == command), "{} has no accelerator", shortcut.id);
        }
    }

    #[test]
    fn a_command_that_asks_first_says_so_and_writes_its_keys_after_a_tab() {
        assert_eq!(text("new-task", "New Task", true, &["Ctrl+N".to_owned()], &[]), "&New Task...\tCtrl+N");
        assert_eq!(text("quit", "Quit", false, &["Ctrl+Q".to_owned()], &[]), "E&xit\tCtrl+Q");
        assert_eq!(text("properties", "Properties", false, &[], &[]), "Propert&ies\tAlt+Enter");
    }
}
