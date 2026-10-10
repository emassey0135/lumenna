//! Keyboard help, on F1 and Ctrl+?: every key, from the table the desktop apps share
//! (`lumenna_desktop::keys`) with this app's own added and worded as GNOME words them.
//!
//! A dialog of ours, a tree of the groups and their keys, on every libadwaita. Not
//! libadwaita 1.8's `AdwShortcutsDialog`: under Orca its items were nameless list items read
//! by their titles alone ("New Task."), never their keys, and its groups toggle buttons
//! "not pressed".

use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use lumenna_desktop::keys::{self, Group};

use crate::tree::{Item, Tree};
use crate::window::App;

/// One line of the help.
struct Entry {
    group: Group,
    title: String,
    keys: Vec<String>,
}

/// Every key, in the shared order, with this app's own where they belong.
fn entries() -> Vec<Entry> {
    let mut entries = Vec::new();
    for group in Group::ALL {
        for shortcut in keys::in_group(group) {
            // GNOME's words, and GNOME's key first where it has its own.
            let (title, keys): (&str, Vec<&str>) = match shortcut.id {
                "settings" => ("Preferences", shortcut.keys.to_vec()),
                "redo" => ("Redo", vec!["Ctrl+Shift+Z", "Ctrl+Y"]),
                "keyboard-help" => (shortcut.title, vec!["F1", "Ctrl+?"]),
                _ => (shortcut.title, shortcut.keys.to_vec()),
            };
            entries.push(Entry { group, title: title.to_owned(), keys: keys.into_iter().map(str::to_owned).collect() });
        }
        let own: &[(&str, &[&str])] = match group {
            Group::Edit => &[("Properties: a Task's Details or a Block's Form", &["Alt+Enter", "Ctrl+I"])],
            Group::Help => &[("Menu Bar", &["F10"])],
            _ => &[],
        };
        for (title, keys) in own {
            entries.push(Entry { group, title: (*title).to_owned(), keys: keys.iter().map(|k| (*k).to_owned()).collect() });
        }
    }
    entries
}

/// Opens the keyboard help: the groups as a tree's top level, each key under its group as
/// "<title>, <keys>", read as every list here is.
pub fn show(app: &Rc<App>) {
    let tree = Tree::new("Keyboard shortcuts");
    tree.widget.set_min_content_height(360);
    let mut items = Vec::new();
    for group in Group::ALL {
        items.push(Item { key: format!("group:{}", group.title()), text: group.title().to_owned(), depth: 0 });
        for (index, entry) in entries().iter().filter(|e| e.group == group).enumerate() {
            items.push(Item {
                key: format!("{}:{index}", group.title()),
                text: format!("{}, {}", entry.title, entry.keys.join(" or ")),
                depth: 1,
            });
        }
    }
    if tree.set(items) {
        tree.select_key_or_near(None, Some(0));
    }
    let close = gtk::Button::with_mnemonic("_Close");
    close.set_halign(gtk::Align::End);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();
    body.append(&tree.widget);
    body.append(&close);
    let window = gtk::Window::builder()
        .title("Keyboard Shortcuts")
        .modal(true)
        .transient_for(&app.window)
        .destroy_with_parent(true)
        .default_width(520)
        .child(&body)
        .build();
    window.set_default_widget(Some(&close));
    {
        let window = window.downgrade();
        close.connect_clicked(move |_| {
            if let Some(window) = window.upgrade() {
                window.close();
            }
        });
    }
    let escape = gtk::EventControllerKey::new();
    {
        let window = window.downgrade();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape
                && let Some(window) = window.upgrade()
            {
                window.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(escape);
    window.present();
    tree.focus();
    // The tree lives as long as its window: the window holds its widget, the tree its rows.
    window.connect_destroy(move |_| {
        let _ = &tree;
    });
}
