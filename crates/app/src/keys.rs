//! The keyboard commands the apps with a PC keyboard share — Windows, GTK, the web, and Android with a
//! keyboard — for each app to bind and its keyboard help to list, so neither the keys nor the
//! help can drift apart between them. Android and the web take them across the FFI
//! ([`grouped`]).
//!
//! Keys are written as a person reads them, "Ctrl+Shift+N"; each app binds them in its own
//! toolkit's terms and adds what only it has (F2 on Windows, say). A command is named by an
//! `id` an app can match its own binding to, and a `title` in Title Case, which an app whose
//! convention differs may word its own way ("Exit" on Windows, "Quit" on GNOME). What a key
//! does to a row is the table's too ([`runs`]), which every app hands on as `Event::Key`, the
//! Apple apps and BTSpeak included, whose keys are their own.

use lumenna_records::ActionKind;

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
    key(Group::View, "go-history", "History", &["Ctrl+5"]),
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
    key(Group::Lists, "delete", "Delete, Move to Trash, Delete from the Trash, or Unassign", &["Delete"]),
    key(Group::Lists, "actions", "Everything That Can Be Done to the Row", &["Shift+F10", "Applications key"]),
    key(Group::Help, "keyboard-help", "Keyboard Shortcuts", &["F1"]),
];

/// What a command does when a shell hands it on (`Event::Key`, `DetailsEvent::Key`): which of
/// a row's actions it runs, which of the day's, something of the app's own, or something only
/// the shell can do (quit, move between panes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runs {
    /// The first of these the row in hand offers, refused in the first's words when it offers
    /// none.
    Row(&'static [ActionKind]),
    /// The first of these the day shown offers of its own, whatever row is in hand.
    Day(&'static [ActionKind]),
    /// Something of the app's own: quick add, a place, undo, a day.
    App,
    /// The shell's own: the app does nothing with it.
    Shell,
}

/// What the command `id` runs (`Runs`). One the table does not know is the shell's.
#[must_use]
pub fn what_runs(id: &str) -> Runs {
    use ActionKind as K;
    match id {
        "delete" => Runs::Row(&[K::Delete, K::DeleteForGood, K::Unassign, K::Unpair]),
        "toggle" => Runs::Row(&[K::MarkDone, K::MarkNotDone, K::Restore, K::StartTimer, K::PauseTimer, K::ResumeTimer]),
        "open" => Runs::Row(&[K::Edit, K::EditTask, K::AddBlock, K::RestoreDay]),
        "open-task" | "properties" => Runs::Row(&[K::Edit, K::EditTask]),
        "mark-done" => Runs::Row(&[K::MarkDone, K::MarkNotDone]),
        "put-in-block" => Runs::Row(&[K::PutInBlock]),
        "move-to-project" => Runs::Row(&[K::MoveToProject]),
        "make-subtask" => Runs::Row(&[K::MakeSubtaskOf]),
        "move-to-top" => Runs::Row(&[K::MoveToTopLevel]),
        "wait-for" => Runs::Row(&[K::WaitFor]),
        "stop-waiting" => Runs::Row(&[K::StopWaiting]),
        "trash-task" => Runs::Row(&[K::Delete]),
        "restore-task" => Runs::Row(&[K::Restore]),
        "erase-task" => Runs::Row(&[K::DeleteForGood]),
        // A menu's, never a key's: it asks first, and is offered only after Delete from Trash.
        "erase-permanently" => Runs::Row(&[K::ErasePermanently]),
        "rename" => Runs::Row(&[K::Rename]),
        "expand" => Runs::Row(&[K::Expand]),
        "collapse" => Runs::Row(&[K::Collapse]),
        "plan-day" => Runs::Day(&[K::PlanDay]),
        "replan" => Runs::Day(&[K::Replan]),
        "what-now" => Runs::Day(&[K::WhatNow]),
        "end-day" => Runs::Day(&[K::EndDay]),
        "clear-day" => Runs::Day(&[K::ClearDay]),
        "new-task" | "new-block" | "new-project" | "new-label" | "new-filter" | "sync-now" | "settings" | "undo" | "redo"
        | "go-today" | "go-tasks" | "go-blocks" | "go-trash" | "go-history" | "save-task" | "previous-day" | "next-day"
        | "go-to-now" | "go-to-day" | "back-up" => Runs::App,
        _ => Runs::Shell,
    }
}

/// What a key or command of the table does to the row in hand, by its `id`, where that is the
/// row's own action: the first of these kinds the row offers (`Event::Key`), refused in the
/// first's words when it offers none. The same on every platform, the Mac's Delete and the
/// iPad's Command-Delete included, so no app decides what its delete key does: Delete from the
/// Trash is what it does to a trashed task, and Erase Permanently is no key's.
#[must_use]
pub fn runs(id: &str) -> Option<&'static [ActionKind]> {
    match what_runs(id) {
        Runs::Row(kinds) => Some(kinds),
        Runs::Day(_) | Runs::App | Runs::Shell => None,
    }
}

/// One command a menu or the tray lists: its `id` (as `SHARED`'s, where it has a key), its
/// name in a menu, and whether it asks something first, which a platform marks its own way
/// ("…", "...").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// What it is: "new-task".
    pub id: &'static str,
    /// Its name in a menu, Title Case: "New Task".
    pub title: &'static str,
    /// Whether it asks something before it is done.
    pub asks: bool,
}

const fn command(id: &'static str, title: &'static str, asks: bool) -> Command {
    Command { id, title, asks }
}

/// Every command a menu lists: the menus (`MENUS`) and the tray (`TRAY`) name them by `id`.
/// Mark Done's name is the row in hand's own once one is in hand (`ViewModel::commands`): Mark
/// Not Done on a task done.
pub const COMMANDS: &[Command] = &[
    command("show", "Show Lumenna", false),
    command("new-task", "New Task", true),
    command("new-block", "New Block", true),
    command("new-project", "New Project", true),
    command("new-label", "New Label", true),
    command("new-filter", "New Saved Filter", true),
    command("sync-now", "Sync Now", false),
    command("devices", "Devices and Pairing", true),
    command("back-up", "Back Up Now", false),
    command("restore-backup", "Restore from a Backup", true),
    command("export-import", "Export and Import", true),
    command("properties", "Properties", false),
    command("settings", "Settings", true),
    command("close-window", "Close Window", false),
    command("quit", "Quit", false),
    command("undo", "Undo", false),
    command("redo", "Redo", false),
    command("filter", "Filter Tasks", false),
    command("go-today", "Today", false),
    command("go-tasks", "Tasks", false),
    command("go-blocks", "Blocks", false),
    command("go-trash", "Trash", false),
    command("go-history", "History", false),
    command("next-pane", "Next Pane", false),
    command("previous-pane", "Previous Pane", false),
    command("mark-done", "Mark Done", false),
    command("open-task", "Edit Details", false),
    command("save-task", "Save Changes", false),
    command("put-in-block", "Put in a Block", true),
    command("move-to-project", "Move to Project", true),
    command("make-subtask", "Make Subtask Of", true),
    command("move-to-top", "Move to Top Level", false),
    command("wait-for", "Wait For", true),
    command("stop-waiting", "Stop Waiting", false),
    command("trash-task", "Move to Trash", false),
    command("restore-task", "Restore from Trash", false),
    command("erase-task", "Delete from Trash", true),
    command("erase-permanently", "Erase Permanently", true),
    command("previous-day", "Previous Day", false),
    command("next-day", "Next Day", false),
    command("go-to-now", "Go to Now", false),
    command("go-to-day", "Go to Day", true),
    command("plan-day", "Plan the Day", true),
    command("replan", "Replan from Now", true),
    command("what-now", "What Should I Work On?", true),
    command("end-day", "End the Day", true),
    command("clear-day", "Clear the Rest of the Day", true),
    command("keyboard-help", "Keyboard Shortcuts", false),
    command("about", "About Lumenna", false),
];

/// The command `id`, as menus list it.
#[must_use]
pub fn command_of(id: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.id == id)
}

/// A separator between a menu's items.
pub const SEPARATOR: &str = "";

/// One menu of a menu bar: its group (whose title it takes) and its items by command `id`, a
/// [`SEPARATOR`] between groups of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu {
    /// Which menu: its title is the group's.
    pub group: Group,
    /// Its items, in order.
    pub items: &'static [&'static str],
}

/// A menu bar's menus, in order: every desktop app's (Windows, GTK, the Mac's), which adds only
/// what is its platform's alone (Cut, Copy and Paste where a menu offers them).
pub const MENUS: &[Menu] = &[
    Menu {
        group: Group::File,
        items: &[
            "new-task", "new-block", "new-project", "new-label", "new-filter", SEPARATOR, "sync-now", "devices", "back-up",
            "restore-backup", "export-import", SEPARATOR, "properties", "settings", SEPARATOR, "close-window", "quit",
        ],
    },
    Menu { group: Group::Edit, items: &["undo", "redo", SEPARATOR, "filter"] },
    Menu {
        group: Group::View,
        items: &["go-today", "go-tasks", "go-blocks", "go-trash", "go-history", SEPARATOR, "next-pane", "previous-pane"],
    },
    Menu {
        group: Group::Task,
        items: &[
            "mark-done", "open-task", "save-task", SEPARATOR, "put-in-block", "move-to-project", "make-subtask", "move-to-top",
            "wait-for", SEPARATOR, "trash-task", "restore-task", "erase-task", "erase-permanently",
        ],
    },
    Menu {
        group: Group::Day,
        items: &[
            "previous-day", "next-day", "go-to-now", "go-to-day", SEPARATOR, "new-block", SEPARATOR, "plan-day", "replan",
            "what-now", "end-day", "clear-day",
        ],
    },
    Menu { group: Group::Help, items: &["keyboard-help", "about"] },
];

/// The menu of Lumenna's icon in the tray, the status bar or the menu bar, in order.
pub const TRAY: &[&str] = &["show", "new-task", "sync-now", SEPARATOR, "quit"];

/// One item of a menu, as an app across the FFI takes it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// The command: "new-task"; empty for a separator.
    pub id: String,
    /// Its name, in the app's capitals; empty for a separator.
    pub title: String,
    /// The keys that run it as a person reads them, the first the one to show.
    pub keys: Vec<String>,
    /// Whether it asks something first, which the platform marks its own way.
    pub asks: bool,
}

/// One menu of the menu bar, as an app across the FFI takes it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuGroup {
    /// Which menu: "file", "edit", ….
    pub id: String,
    /// Its title, in the app's capitals.
    pub title: String,
    /// Its items, in order.
    pub items: Vec<MenuItem>,
}

/// `text` in sentence case where `sentence`.
fn cased(text: &str, sentence: bool) -> String {
    if sentence { lumenna_records::fields::sentence_case(text.to_owned()) } else { text.to_owned() }
}

/// The item for `id`, its title in sentence case where `sentence`.
fn item(id: &str, sentence: bool) -> MenuItem {
    let command = command_of(id);
    MenuItem {
        id: id.to_owned(),
        title: command.map(|c| cased(c.title, sentence)).unwrap_or_default(),
        keys: SHARED.iter().find(|s| s.id == id).map(|s| s.keys.iter().map(|&k| k.to_owned()).collect()).unwrap_or_default(),
        asks: command.is_some_and(|c| c.asks),
    }
}

/// The menu bar (`MENUS`), worded, in sentence case where `sentence`.
#[must_use]
pub fn menus(sentence: bool) -> Vec<MenuGroup> {
    MENUS
        .iter()
        .map(|menu| MenuGroup {
            id: menu.group.id().to_owned(),
            title: cased(menu.group.title(), sentence),
            items: menu.items.iter().map(|id| item(id, sentence)).collect(),
        })
        .collect()
}

/// The tray's menu (`TRAY`), worded, in sentence case where `sentence`.
#[must_use]
pub fn tray(sentence: bool) -> Vec<MenuItem> {
    TRAY.iter().map(|id| item(id, sentence)).collect()
}

/// A row or day command as the view offers it now: its name ("Mark Not Done" on a task done),
/// and whether anything in hand offers it, for a menu to draw and to grey out.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CommandState {
    /// The command.
    pub id: String,
    /// Its name now, in the shell's capitals.
    pub title: String,
    /// Whether the row in hand, or the day shown, offers it.
    pub enabled: bool,
}

/// Every row and day command's state, given the actions of the row in hand and the day's own.
/// Its name is the menu's, but for Mark Done, which takes the row's own (Mark Not Done).
#[must_use]
pub fn states(row: &[lumenna_records::Action], day: &[lumenna_records::Action], sentence: bool) -> Vec<CommandState> {
    COMMANDS
        .iter()
        .filter_map(|command| {
            let (kinds, actions) = match what_runs(command.id) {
                Runs::Row(kinds) => (kinds, row),
                Runs::Day(kinds) => (kinds, day),
                Runs::App | Runs::Shell => return None,
            };
            let found = kinds.iter().find_map(|kind| actions.iter().find(|a| a.kind == *kind));
            let title = match found {
                Some(action) if action.kind == ActionKind::MarkNotDone => {
                    if sentence && !action.sentence.is_empty() { action.sentence.clone() } else { action.title.clone() }
                }
                _ => cased(command.title, sentence),
            };
            Some(CommandState { id: command.id.to_owned(), title, enabled: found.is_some() })
        })
        .collect()
}

/// The shared keys of `group`, in order.
pub fn in_group(group: Group) -> impl Iterator<Item = &'static Shortcut> {
    SHARED.iter().filter(move |shortcut| shortcut.group == group)
}

/// One command and its keys, as an app across the FFI takes them: Android's and the web's.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCommand {
    /// What the app matches its own binding to: "undo".
    pub id: String,
    /// What it does, in the app's capitals: "Mark done or not done".
    pub title: String,
    /// The keys as a person reads them, the first the one to show; any others also work.
    pub keys: Vec<String>,
}

/// One heading of the keyboard help, and its commands in order.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGroup {
    /// Which group: "file", "edit", "view", "task", "day", "lists", "help".
    pub id: String,
    /// Its heading: "In a list".
    pub title: String,
    /// Its commands, in the order a help lists them.
    pub commands: Vec<KeyCommand>,
}

impl Group {
    /// The group's identifier, for an app across the FFI.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Group::File => "file",
            Group::Edit => "edit",
            Group::View => "view",
            Group::Task => "task",
            Group::Day => "day",
            Group::Lists => "lists",
            Group::Help => "help",
        }
    }
}

/// Every shared command, grouped as the menus are, its titles in sentence case where the app
/// writes that (Android, the web) and Title Case otherwise.
#[must_use]
pub fn grouped(sentence: bool) -> Vec<KeyGroup> {
    let cased = |text: &str| if sentence { lumenna_records::fields::sentence_case(text.to_owned()) } else { text.to_owned() };
    Group::ALL
        .iter()
        .map(|&group| KeyGroup {
            id: group.id().to_owned(),
            title: cased(group.title()),
            commands: in_group(group)
                .map(|shortcut| KeyCommand {
                    id: shortcut.id.to_owned(),
                    title: cased(shortcut.title),
                    keys: shortcut.keys.iter().map(|&key| key.to_owned()).collect(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_erases_permanently_and_delete_deletes_from_the_trash() {
        for shortcut in SHARED {
            let kinds = runs(shortcut.id).unwrap_or_default();
            assert!(!kinds.contains(&ActionKind::ErasePermanently), "{}", shortcut.id);
            assert!(!kinds.contains(&ActionKind::EraseEverythingDeleted), "{}", shortcut.id);
        }
        assert_eq!(runs("delete").map(|kinds| kinds[..2].to_vec()), Some(vec![ActionKind::Delete, ActionKind::DeleteForGood]));
    }

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
