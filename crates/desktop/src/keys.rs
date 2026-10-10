//! The keyboard commands the desktop apps share — Windows, GTK, and Android with a keyboard —
//! for each app's keyboard help to list, so the help and the keys cannot drift apart.
//!
//! Keys are written as a person reads them, "Ctrl+Shift+N"; each app binds them in its own
//! toolkit's terms and adds what only it has (F2 on Windows, say). A command is named by an
//! `id` an app can match its own binding to, and a `title` in Title Case, which an app whose
//! convention differs may word its own way ("Exit" on Windows, "Quit" on GNOME).

/// Where a command is listed: the menus, then what keys do in a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Making things, syncing, settings, the window.
    File,
    /// Undo and filtering.
    Edit,
    /// Going to a place, and between panes.
    View,
    /// What is done to the task in hand.
    Task,
    /// Moving through the planner's days.
    Day,
    /// What keys do on the row that has focus.
    Lists,
    /// Finding out what the keys are.
    Help,
}

impl Group {
    /// Every group, in the order a help lists them: the menus' order, then lists.
    pub const ALL: [Group; 7] = [Group::File, Group::Edit, Group::View, Group::Task, Group::Day, Group::Lists, Group::Help];

    /// The group's heading.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Group::File => "File",
            Group::Edit => "Edit",
            Group::View => "View",
            Group::Task => "Task",
            Group::Day => "Day",
            Group::Lists => "In a List",
            Group::Help => "Help",
        }
    }
}

/// One command and the keys that run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    /// Where it is listed.
    pub group: Group,
    /// What an app matches its own binding to: "new-task".
    pub id: &'static str,
    /// What it does: "New Task".
    pub title: &'static str,
    /// The keys, as read, the first being the one to show; any others also work.
    pub keys: &'static [&'static str],
}

const fn key(group: Group, id: &'static str, title: &'static str, keys: &'static [&'static str]) -> Shortcut {
    Shortcut { group, id, title, keys }
}

/// The keys every desktop app has, in the order a help lists them.
pub const SHARED: &[Shortcut] = &[
    key(Group::File, "new-task", "New Task", &["Ctrl+N"]),
    key(Group::File, "new-block", "New Block", &["Ctrl+Shift+N"]),
    key(Group::File, "sync-now", "Sync Now", &["F5"]),
    key(Group::File, "settings", "Settings", &["Ctrl+Comma"]),
    key(Group::File, "close-window", "Close Window", &["Ctrl+W"]),
    key(Group::File, "quit", "Quit", &["Ctrl+Q"]),
    key(Group::Edit, "undo", "Undo", &["Ctrl+Z"]),
    key(Group::Edit, "redo", "Redo", &["Ctrl+Y", "Ctrl+Shift+Z"]),
    key(Group::Edit, "filter", "Filter Tasks", &["Ctrl+F"]),
    key(Group::View, "go-today", "Today", &["Ctrl+1"]),
    key(Group::View, "go-tasks", "Tasks", &["Ctrl+2"]),
    key(Group::View, "go-blocks", "Blocks", &["Ctrl+3"]),
    key(Group::View, "go-trash", "Trash", &["Ctrl+4"]),
    key(Group::View, "next-pane", "Next Pane", &["F6"]),
    key(Group::View, "previous-pane", "Previous Pane", &["Shift+F6"]),
    key(Group::Task, "mark-done", "Mark Done or Not Done", &["Ctrl+K"]),
    key(Group::Task, "save-task", "Save Changes", &["Ctrl+S"]),
    key(Group::Task, "put-in-block", "Put in a Block", &["Ctrl+B"]),
    key(Group::Task, "move-to-project", "Move to Project", &["Ctrl+Shift+M"]),
    key(Group::Day, "previous-day", "Previous Day", &["Ctrl+Page Up"]),
    key(Group::Day, "next-day", "Next Day", &["Ctrl+Page Down"]),
    key(Group::Day, "go-to-now", "Go to Now", &["Ctrl+T"]),
    key(Group::Day, "go-to-day", "Go to Day", &["Ctrl+G"]),
    key(Group::Lists, "open", "Open the Task, or Change the Block", &["Enter"]),
    key(Group::Lists, "toggle", "Mark Done, Restore from the Trash, or Start, Pause or Resume a Timer", &["Space"]),
    key(Group::Lists, "delete", "Delete, Move to Trash, or Unassign", &["Delete"]),
    key(Group::Lists, "actions", "Everything That Can Be Done to the Row", &["Shift+F10", "Applications key"]),
    key(Group::Help, "keyboard-help", "Keyboard Shortcuts", &["F1"]),
];

/// The shared keys of `group`, in order.
pub fn in_group(group: Group) -> impl Iterator<Item = &'static Shortcut> {
    SHARED.iter().filter(move |shortcut| shortcut.group == group)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_runs_two_commands() {
        let mut seen: Vec<&str> = Vec::new();
        for shortcut in SHARED {
            for key in shortcut.keys {
                assert!(!seen.contains(key), "{key} is bound twice");
                seen.push(key);
            }
        }
    }

    #[test]
    fn every_command_is_in_a_group_a_help_lists() {
        for shortcut in SHARED {
            assert!(Group::ALL.contains(&shortcut.group), "{} is in no listed group", shortcut.id);
            assert!(!shortcut.keys.is_empty(), "{} has no key", shortcut.id);
        }
    }
}
