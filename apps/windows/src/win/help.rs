//! The keyboard shortcuts, on F1 and from the Help menu: every key and what it does, grouped
//! as the menus are.
//!
//! The keys every desktop app shares are `desktop::keys`, so this help and the other apps'
//! cannot drift from each other; what only Windows has is added here. It is read-only text,
//! one command a line, so a screen reader reads it line by line with the arrows.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Controls::EM_SETSEL;
use windows::Win32::UI::WindowsAndMessaging::{
    BS_DEFPUSHBUTTON, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, IDCANCEL, WS_BORDER, WS_TABSTOP, WS_VSCROLL,
};

use super::controls;
use super::dialog::{self, Class, Dialog, Template};
use crate::keys::{self, Group, Shortcut};

const TEXT: u16 = 100;

/// What only Windows has, under the group it belongs to.
const WINDOWS: &[Shortcut] = &[
    Shortcut { group: Group::Lists, id: "rename", title: "Rename", keys: &["F2"] },
    Shortcut { group: Group::Lists, id: "properties", title: "Properties: the Task's Details, or the Block's Form", keys: &["Alt+Enter"] },
];

/// A command's name as Windows words it: Windows says Exit where others say Quit.
fn title(shortcut: &Shortcut) -> &'static str {
    if shortcut.id == "quit" { "Exit" } else { shortcut.title }
}

/// Every shortcut, a group's heading and then a command a line: "New Task: Ctrl+N".
pub fn text() -> String {
    let mut lines = Vec::new();
    for group in Group::ALL {
        let mut listed = keys::in_group(group).chain(WINDOWS.iter().filter(|s| s.group == group)).peekable();
        if listed.peek().is_none() {
            continue;
        }
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(group.title().to_owned());
        lines.extend(listed.map(|shortcut| format!("{}: {}", title(shortcut), shortcut.keys.join(" or "))));
    }
    lines.join("\r\n")
}

struct Help;

impl Dialog for Help {
    fn template(&self) -> Template {
        let text = (ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL) as u32 | WS_VSCROLL.0 | WS_BORDER.0 | WS_TABSTOP.0;
        Template::new("Keyboard Shortcuts", 300, 230)
            .item(Class::Static, "&Shortcuts:", u16::MAX, 0, 7, 7, 286, 9)
            .item(Class::Edit, "", TEXT, text, 7, 17, 286, 186)
            .item(Class::Button, "Close", IDCANCEL.0 as u16, BS_DEFPUSHBUTTON as u32 | WS_TABSTOP.0, 243, 209, 50, 14)
    }

    fn init(&self, hwnd: HWND) -> bool {
        let field = dialog::item(hwnd, TEXT);
        controls::set_text(field, &text());
        // From the top, nothing selected, so the first line is read first.
        controls::send(field, EM_SETSEL, 0, 0);
        controls::focus(field);
        true
    }

    fn command(&self, _hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        (i32::from(id) == IDCANCEL.0).then_some(0)
    }
}

/// Shows the keyboard shortcuts until closed.
pub fn show(owner: HWND) {
    dialog::run(Some(owner), &Help);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_lists_every_shared_key_and_windows_own() {
        let text = text();
        for shortcut in keys::SHARED.iter().chain(WINDOWS) {
            assert!(text.contains(&format!("{}: {}", title(shortcut), shortcut.keys[0])), "{} is missing", shortcut.id);
        }
        assert!(text.contains("Exit: Ctrl+Q"), "{text}");
    }
}
