//! The menu bar, from the core's (`keys::menus`): every desktop app's menus and items, in its
//! order and words, each item the window's action `win.<id>`, with the shared keys beside it as
//! GTK's accelerators. What is this app's: the mnemonics, GNOME's own extra keys, and which keys
//! are shown beside an item but answered by the list with focus.
//!
//! Mark Done's name is the row in hand's (`ViewModel::commands`: Mark Not Done on a task done),
//! so the bar is made again when a name changes.

use std::cell::RefCell;

use gtk::gio;
use gtk::prelude::*;
use lumenna_app::keys::{self, CommandState};

/// Each menu's and item's mnemonic, by id: the letter GTK marks with an underscore. One per
/// menu, and none of the menu bar's own letters within a menu.
const MNEMONICS: &[(&str, char)] = &[
    ("file", 'F'),
    ("edit", 'E'),
    ("view", 'V'),
    ("task", 'T'),
    ("day", 'D'),
    ("help", 'H'),
    ("show", 'S'),
    ("new-task", 'N'),
    ("new-block", 'B'),
    ("new-project", 'P'),
    ("new-label", 'L'),
    ("new-filter", 'F'),
    ("sync-now", 'y'),
    ("devices", 'D'),
    ("back-up", 'U'),
    ("restore-backup", 'R'),
    ("export-import", 'x'),
    ("properties", 'o'),
    ("settings", 't'),
    ("close-window", 'C'),
    ("quit", 'Q'),
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
    // The tray's timer control.
    ("timer", 'T'),
];

/// GNOME's own keys beside the shared ones: Properties as Files has it, help as GNOME's apps.
const GNOME_KEYS: &[(&str, &[&str])] = &[("properties", &["<Alt>Return", "<Control>i"]), ("keyboard-help", &["<Control>question"])];

/// Items whose key the list with focus answers itself, so it is shown beside them and never
/// bound: the key of the shared command named.
const ANSWERED: &[(&str, &str)] = &[("open-task", "open"), ("trash-task", "delete")];

thread_local! {
    /// The names the bar was last made with, by command.
    static NAMED: RefCell<Option<Vec<(String, String)>>> = const { RefCell::new(None) };
}

/// `title` with the mnemonic of `id` marked, and "…" when it asks something first.
pub fn label(id: &str, title: &str, asks: bool) -> String {
    let title = match MNEMONICS.iter().find(|(of, _)| *of == id) {
        Some((_, letter)) => lumenna_app::devices::marked(title, *letter, '_'),
        None => title.replace('_', "__"),
    };
    if asks { format!("{title}…") } else { title }
}

/// Makes the menu bar, and again whenever a command's name in `states` has changed since.
pub fn follow(application: &gtk::Application, states: &[CommandState]) {
    let named: Vec<(String, String)> = states.iter().map(|state| (state.id.clone(), state.title.clone())).collect();
    if NAMED.with_borrow(|last| last.as_ref() == Some(&named)) {
        return;
    }
    application.set_menubar(Some(&bar(&named)));
    NAMED.set(Some(named));
}

/// The menu bar, each command named as `named` says where it does, else as the menus do.
fn bar(named: &[(String, String)]) -> gio::Menu {
    let bar = gio::Menu::new();
    for group in keys::menus(false) {
        let menu = gio::Menu::new();
        let mut section = gio::Menu::new();
        for item in &group.items {
            if item.id.is_empty() {
                menu.append_section(None, &section);
                section = gio::Menu::new();
                continue;
            }
            let title = named.iter().find(|(id, _)| *id == item.id).map_or(item.title.as_str(), |(_, title)| title.as_str());
            let entry = gio::MenuItem::new(Some(&label(&item.id, title, item.asks)), Some(&format!("win.{}", item.id)));
            let answered = ANSWERED.iter().find(|(id, _)| *id == item.id).and_then(|(_, of)| shared_key(of));
            if let Some(key) = answered {
                entry.set_attribute_value("accel", Some(&key.to_variant()));
            }
            section.append_item(&entry);
        }
        menu.append_section(None, &section);
        bar.append_submenu(Some(&label(&group.id, &group.title, false)), &menu);
    }
    bar
}

/// The first key of the shared command `id`, as GTK writes an accelerator.
fn shared_key(id: &str) -> Option<String> {
    keys::SHARED.iter().find(|shortcut| shortcut.id == id)?.keys.first().and_then(|key| accelerator(key))
}

/// Binds every command's shared keys, and GNOME's own, to its action.
pub fn accelerators(application: &gtk::Application) {
    for command in keys::COMMANDS {
        let mut accels: Vec<String> = keys::SHARED
            .iter()
            .filter(|shortcut| shortcut.id == command.id)
            .flat_map(|shortcut| shortcut.keys.iter().filter_map(|key| accelerator(key)))
            .collect();
        if let Some((_, own)) = GNOME_KEYS.iter().find(|(id, _)| *id == command.id) {
            accels.extend(own.iter().map(|&key| key.to_owned()));
        }
        if !accels.is_empty() {
            let accels: Vec<&str> = accels.iter().map(String::as_str).collect();
            application.set_accels_for_action(&format!("win.{}", command.id), &accels);
        }
    }
}

/// A key as the shared table reads it ("Ctrl+Shift+N", "Ctrl+Page Up", "F5") as GTK writes an
/// accelerator ("<Control><Shift>n"); none for one that is no single key ("Applications key").
fn accelerator(key: &str) -> Option<String> {
    let mut parts: Vec<&str> = key.split('+').collect();
    let last = parts.pop()?;
    let mut accel = String::new();
    for modifier in parts {
        accel.push_str(match modifier {
            "Ctrl" => "<Control>",
            "Shift" => "<Shift>",
            "Alt" => "<Alt>",
            _ => return None,
        });
    }
    let name = match last {
        "Comma" => "comma".to_owned(),
        "Page Up" => "Page_Up".to_owned(),
        "Page Down" => "Page_Down".to_owned(),
        "Enter" => "Return".to_owned(),
        "Space" => "space".to_owned(),
        "Delete" => "Delete".to_owned(),
        letter if letter.chars().count() == 1 => letter.to_lowercase(),
        function if function.starts_with('F') && function[1..].parse::<u8>().is_ok() => function.to_owned(),
        _ => return None,
    };
    accel.push_str(&name);
    Some(accel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shared_key_a_menu_item_has_is_a_gtk_accelerator() {
        for group in keys::menus(false) {
            for item in group.items.iter().filter(|item| !item.id.is_empty()) {
                for key in &item.keys {
                    let accel = accelerator(key).unwrap_or_else(|| panic!("{} has {key}, which GTK cannot bind", item.id));
                    let name = accel.rsplit('>').next().unwrap_or(&accel);
                    assert!(gtk::gdk::Key::from_name(name).is_some(), "{name} for {key} is no key GTK knows");
                }
            }
        }
        assert_eq!(accelerator("Ctrl+Shift+N").as_deref(), Some("<Control><Shift>n"));
        assert_eq!(accelerator("Ctrl+Page Down").as_deref(), Some("<Control>Page_Down"));
        assert_eq!(accelerator("Applications key"), None);
    }

    #[test]
    fn every_menu_item_and_menu_has_a_mnemonic_of_its_own_in_its_menu() {
        for group in keys::menus(false) {
            assert!(label(&group.id, &group.title, false).contains('_'), "{} has no mnemonic", group.id);
            let mut letters = Vec::new();
            for item in group.items.iter().filter(|item| !item.id.is_empty()) {
                let marked = label(&item.id, &item.title, item.asks);
                let at = marked.find('_').unwrap_or_else(|| panic!("{} has no mnemonic in {marked:?}", item.id));
                let letter = marked[at + 1..].chars().next().unwrap().to_ascii_lowercase();
                assert!(!letters.contains(&letter), "{} repeats {letter} in the {} menu", item.id, group.id);
                letters.push(letter);
            }
        }
    }
}
