//! A braille device's letters: every braille device's keys run the same thing for the same
//! letter, and how a letter is pressed is each app's own: it maps its presses onto `Event::Letter`
//! and `Event::FindLetter`, and nothing here assumes which. (A BTSpeak's plain letter is the
//! command and a capital, Dot 7 with the letter, moves by first letter; on the Orbit Speak and the
//! BrailleSense, Space with the letter is the command and the letter alone moves.) What a shell shows beside a row's action, a settings
//! page or a reminder's button is asked for here. Nothing a shell keeps.
//!
//! A letter is the core's own: a device reporting dots turns them into a letter by
//! [`braille_letter`], its own dots for a to z, never the person's table, which reads Z alone as
//! "as".
//!
//! `,`, `.` and `/` have letters here but no dots: the BTSpeak hands its app the characters, and
//! a device that reports dots reaches Move Up, Move Down and filtering from a row's actions or
//! its own key.

use crux_core::Command;
use lumenna_records::places::{Place, SidebarKind, sidebar};
use lumenna_records::{Action, ActionKind};

use crate::asking::{Open, Purpose, refuse};
use crate::keys::{Runs, command_of, what_runs};
use crate::model::Screen;
use crate::{DueEvent, Effect, Event, Lumenna, Side, refused, say};

use ActionKind as K;

/// The letter of each kind of action: the same letter for the same thing on every row. Two
/// kinds share a letter only where no row offers both, so the first a row offers is what its
/// letter runs. Erase Permanently has none: it is a menu's alone.
const ACTION_LETTERS: &[(ActionKind, &str)] = &[
    (K::MarkDone, "c"),
    (K::MarkNotDone, "c"),
    (K::Edit, "e"),
    (K::EditTask, "e"),
    (K::PutInBlock, "b"),
    (K::MoveToProject, "m"),
    (K::MakeSubtaskOf, "s"),
    (K::MoveToTopLevel, "t"),
    (K::WaitFor, "w"),
    (K::StopWaiting, "n"),
    (K::Restore, "r"),
    (K::AssignTask, "i"),
    (K::CancelDay, "x"),
    (K::RestoreDay, "o"),
    (K::StartTimer, "s"),
    (K::PauseTimer, "s"),
    (K::ResumeTimer, "s"),
    (K::StopTimer, "t"),
    (K::PlannedLength, "l"),
    (K::LogMinutes, "m"),
    (K::AddBlock, "a"),
    (K::Rename, "r"),
    (K::NewInside, "n"),
    (K::MoveUnder, "m"),
    (K::MoveUp, ","),
    (K::MoveDown, "."),
    (K::Weight, "w"),
    (K::MergeInto, "m"),
    (K::Colour, "c"),
    (K::ChangeQuery, "q"),
    (K::Reminders, "r"),
    // A sitting's Carry Forward and a block's Carry the Rest Forward: no row offers both.
    (K::CarryForward, "f"),
    (K::CarryRest, "f"),
    // Add Reminder in a reminder list, New Project on a heading.
    (K::New, "a"),
    // The day's own, on its menu: no row of the day uses it.
    (K::WhatNow, "w"),
];

/// The shared table's command a kind runs where the table has one (`keys::what_runs`): a kind
/// not here runs the row's action of that kind.
const ACTION_COMMANDS: &[(ActionKind, &str)] = &[
    (K::MarkDone, "mark-done"),
    (K::MarkNotDone, "mark-done"),
    (K::Edit, "properties"),
    (K::EditTask, "properties"),
    (K::PutInBlock, "put-in-block"),
    (K::MoveToProject, "move-to-project"),
    (K::MakeSubtaskOf, "make-subtask"),
    (K::MoveToTopLevel, "move-to-top"),
    (K::WaitFor, "wait-for"),
    (K::StopWaiting, "stop-waiting"),
    (K::Restore, "restore-task"),
    (K::Rename, "rename"),
];

/// The letter of each settings page, by `SettingsPage::id`.
const PAGE_LETTERS: &[(&str, &str)] =
    &[("general", "g"), ("planning", "p"), ("reminders", "m"), ("devices", "d"), ("backups", "b"), ("export", "e")];

/// The letter of each settings page's button, by `SettingsButton::id`.
const BUTTON_LETTERS: &[(&str, &str)] =
    &[("sync-now", "s"), ("pair", "p"), ("back-up", "b"), ("restore-backup", "r"), ("import", "i")];

/// The letter of each of a reminder's buttons, by `DueButton::id`. Dismiss is the delete keys'.
const DUE_LETTERS: &[(&str, &str)] = &[("carry", "f"), ("replan", "p"), ("snooze", "z")];

/// The letter of each of a screen's own commands, by the shared table's `id` where it has one.
/// An id is on one screen only, so one letter serves it: the screens are [`place_commands`] and
/// [`main_menu`].
const COMMAND_LETTERS: &[(&str, &str)] = &[
    ("undo", "u"),
    ("redo", "y"),
    ("new-task", "a"),
    ("filter", "/"),
    ("new-block", "a"),
    ("previous-day", "p"),
    ("next-day", "n"),
    ("go-to-day", "g"),
    ("go-to-now", "t"),
    ("new-project-inside", "p"),
    // The main menu.
    ("go-today", "t"),
    ("go-tasks", "k"),
    ("go-blocks", "b"),
    ("go-history", "h"),
    ("go-trash", "x"),
    ("projects", "p"),
    ("labels", "l"),
    ("filters", "f"),
    ("reminders-due", "r"),
    ("sync-now", "n"),
    ("settings", "s"),
    // A project's, label's or filter's row, and the BTSpeak's own preference.
    ("add-task-in", "t"),
    ("read-back", "r"),
];

fn found(table: &'static [(&'static str, &'static str)], id: &str) -> Option<&'static str> {
    table.iter().find(|(key, _)| *key == id).map(|(_, letter)| *letter)
}

/// The letter of a kind of action, none for one no letter runs (Erase Permanently, folding).
#[must_use]
pub fn action_letter(kind: ActionKind) -> Option<&'static str> {
    ACTION_LETTERS.iter().find(|(lettered, _)| *lettered == kind).map(|(_, letter)| *letter)
}

/// The letter of a settings page, by its `id`.
#[must_use]
pub fn page_letter(id: &str) -> Option<&'static str> {
    found(PAGE_LETTERS, id)
}

/// The letter of a settings page's button, by its `id`.
#[must_use]
pub fn button_letter(id: &str) -> Option<&'static str> {
    found(BUTTON_LETTERS, id)
}

/// The letter of one of a reminder's buttons, by its `id`.
#[must_use]
pub fn due_letter(id: &str) -> Option<&'static str> {
    found(DUE_LETTERS, id)
}

/// The letter of a screen's own command, by the shared table's `id` where it has one.
#[must_use]
pub fn command_letter(id: &str) -> Option<&'static str> {
    found(COMMAND_LETTERS, id)
}

fn command_for(kind: ActionKind) -> Option<&'static str> {
    ACTION_COMMANDS.iter().find(|(lettered, _)| *lettered == kind).map(|(_, id)| *id)
}

/// The command a letter is, what a shell shows beside it: its `id`, its `letter`, and its name.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetterCommand {
    /// What it is: `new-block`, or, on the main menu, `projects`.
    pub id: String,
    /// Its letter.
    pub letter: String,
    /// Its name, in sentence case where asked for.
    pub title: String,
}

/// A command's name by its `id`, the shared table's where it has one.
fn command_title(id: &str, sentence: bool) -> String {
    let title = match id {
        "projects" => "Projects",
        "labels" => "Labels",
        "filters" => "Saved Filters",
        "new-project-inside" => "New Project Inside",
        "reminders-due" => return lumenna_records::words::reminders_due(sentence),
        other => command_of(other).map_or(other, |command| command.title),
    };
    if sentence { lumenna_records::fields::sentence_case(title.to_owned()) } else { title.to_owned() }
}

fn lettered(ids: &[&str], sentence: bool) -> Vec<LetterCommand> {
    ids.iter()
        .filter_map(|id| {
            Some(LetterCommand { id: (*id).to_owned(), letter: command_letter(id)?.to_owned(), title: command_title(id, sentence) })
        })
        .collect()
}

/// What `place` itself offers beside its rows, each with its letter: what a shell lists on its
/// screen's own menu. New Task's and the day's commands run through `Event::Letter`, the filter
/// is the shell's own field.
#[must_use]
pub fn place_commands(place: &Place, sentence: bool) -> Vec<LetterCommand> {
    lettered(place_command_ids(place), sentence)
}

fn place_command_ids(place: &Place) -> &'static [&'static str] {
    match place {
        Place::Today => &["new-block", "previous-day", "next-day", "go-to-day", "go-to-now", "undo", "redo"],
        Place::Blocks => &["new-block", "undo", "redo"],
        Place::Project(_) => &["new-task", "filter", "new-project-inside", "undo", "redo"],
        _ => &["new-task", "filter", "undo", "redo"],
    }
}

/// The main menu's commands, each with its letter: the places, then adding a task, undo and
/// redo, reminders, syncing and settings. `projects`, `labels` and `filters` are the sidebar's
/// three groups' own; `reminders-due` is the reminders due's screen; the rest the shared table's.
#[must_use]
pub fn main_menu(sentence: bool) -> Vec<LetterCommand> {
    lettered(
        &[
            "go-today", "go-tasks", "new-task", "go-blocks", "go-history", "go-trash", "projects", "labels", "filters", "undo",
            "redo", "reminders-due", "sync-now", "settings",
        ],
        sentence,
    )
}

/// What a line on a device with its own controls is, as the HIMS BrailleSense names them.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRole {
    /// Opens a submenu.
    Menu,
    /// Runs when pressed.
    MenuItem,
    /// A row of a list.
    ListItem,
    /// A line to type in.
    EditBox,
    /// One of several, changed in place.
    ComboBox,
    /// A check box that is checked.
    CheckBoxChecked,
    /// A check box that is not.
    CheckBoxUnchecked,
    /// Runs when pressed.
    Button,
    /// An answer to a question, chosen in place.
    PromptButton,
    /// A line of information.
    StaticBox,
}

/// The device's name for a role: spoken, and its symbol on the cells.
fn role_words(role: ControlRole) -> (&'static str, &'static str) {
    match role {
        ControlRole::Menu => ("Menu", "mn"),
        ControlRole::MenuItem => ("menu item", "mi"),
        ControlRole::ListItem => ("list item", "li"),
        ControlRole::EditBox => ("edit box", "eb"),
        ControlRole::ComboBox => ("combo box", "cb"),
        ControlRole::CheckBoxChecked => ("check box checked", "schb"),
        ControlRole::CheckBoxUnchecked => ("check box unchecked", "uchb"),
        ControlRole::Button => ("button", "bt"),
        ControlRole::PromptButton => ("prompt button", "pb"),
        ControlRole::StaticBox => ("static box", "st"),
    }
}

/// Where a role goes on its line: the device's Control Information settings, one for the cells and
/// one for the voice.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolePlace {
    /// Left out.
    Off,
    /// Ahead of the text.
    Before,
    /// After it.
    After,
}

/// What the person's device settings say a line carries.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSettings {
    /// Where the voice says the role.
    pub voice_role: RolePlace,
    /// Where the cells show it.
    pub braille_role: RolePlace,
    /// Whether the voice says a shortcut letter.
    pub voice_letter: bool,
    /// Whether the cells show it.
    pub braille_letter: bool,
    /// Whether the voice says a hot key.
    pub voice_hot_key: bool,
    /// Whether the cells show it.
    pub braille_hot_key: bool,
    /// Whether a place among several is given: "2/5".
    pub index: bool,
}

/// The cells of the device's own signs in its braille code, as Braille ASCII.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrailleSigns {
    /// What opens a shortcut letter.
    pub open: String,
    /// What closes it.
    pub close: String,
    /// What ends an item that opens a dialog.
    pub dialog: String,
    /// What ends an item that opens a submenu.
    pub pull_down: String,
}

/// One line to word.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceLine {
    /// What it is.
    pub role: ControlRole,
    /// What it is for, ahead of its value; empty for none.
    pub label: String,
    /// Its text as said.
    pub speech: String,
    /// Its text as shown.
    pub text: String,
    /// What follows the letter, said and shown; empty for none.
    pub detail: String,
    /// Its shortcut letter.
    pub letter: Option<String>,
    /// What runs it from anywhere: "Enter-n".
    pub hot_key: Option<String>,
    /// Whether it opens a dialog.
    pub asks: bool,
    /// Which of [`Self::count`] it is, from 1, for a control that shows one of several.
    pub position: u32,
    /// How many there are; 0 for a line that is not one of several.
    pub count: u32,
}

/// A stretch of the cells beside a line's text.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BraillePart {
    /// What is shown.
    pub text: String,
    /// Whether it is Braille ASCII, shown cell for cell, rather than text in the person's table.
    pub computer: bool,
}

/// A line as the device reads it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceReading {
    /// What is said.
    pub speech: String,
    /// What the cells show ahead of [`Self::body`].
    pub before: Vec<BraillePart>,
    /// The text, in the person's table; an edit box puts its own cells here.
    pub body: String,
    /// What the cells show after it.
    pub after: Vec<BraillePart>,
}

/// A line in the device's own order: the role where the settings put it, the name, the shortcut
/// letter in brackets, a hot key, and the sign of what pressing it opens.
#[must_use]
pub fn device_reading(line: &DeviceLine, settings: LineSettings, signs: &BrailleSigns) -> DeviceReading {
    let (say, show) = role_words(line.role);
    let index = (settings.index && line.count > 0).then(|| format!("{}/{}", line.position, line.count));

    let mut speech = String::new();
    if !line.label.is_empty() {
        speech.push_str(&line.label);
        if !line.speech.is_empty() {
            speech.push_str(": ");
        }
    }
    speech.push_str(&line.speech);
    if let Some(index) = &index {
        speech.push(' ');
        speech.push_str(index);
    }
    if settings.voice_letter && let Some(letter) = &line.letter {
        speech.push_str(&format!(" ({letter})"));
    }
    if !line.detail.is_empty() {
        speech.push_str(", ");
        speech.push_str(&line.detail);
    }
    if settings.voice_hot_key && let Some(key) = &line.hot_key {
        speech.push(' ');
        speech.push_str(key);
    }
    if line.role == ControlRole::Menu {
        speech.push_str(" pulldown");
    } else if line.asks {
        speech.push_str(" dialog");
    }
    let speech = match settings.voice_role {
        RolePlace::Off => speech,
        RolePlace::Before => format!("{say} {speech}"),
        RolePlace::After => format!("{speech} {say}"),
    };

    let computer = |text: String| BraillePart { text, computer: true };
    let table = |text: String| BraillePart { text, computer: false };
    let mut before = Vec::new();
    if settings.braille_role == RolePlace::Before {
        before.push(computer(format!("{show} ")));
    }
    if !line.label.is_empty() {
        before.push(table(format!("{}: ", line.label)));
    }
    let mut after = Vec::new();
    if let Some(index) = index {
        after.push(table(format!(" {index}")));
    }
    if settings.braille_letter && let Some(letter) = &line.letter {
        after.push(computer(format!(" {}{letter}{}", signs.open, signs.close)));
    }
    if !line.detail.is_empty() {
        after.push(table(format!(" {}", line.detail)));
    }
    if settings.braille_hot_key && let Some(key) = &line.hot_key {
        after.push(computer(format!(" {}", key.to_lowercase())));
    }
    if line.role == ControlRole::Menu {
        after.push(computer(signs.pull_down.clone()));
    } else if line.asks {
        after.push(computer(format!(" {}", signs.dialog)));
    }
    if settings.braille_role == RolePlace::After {
        after.push(computer(format!(" {show}")));
    }
    DeviceReading { speech, before, body: line.text.clone(), after }
}

/// What opens a dialog says: its title and that it is one.
#[must_use]
pub fn dialog_title(title: &str) -> String {
    format!("{title} dialog")
}

/// The dots of each letter from a to z, Grade 1 braille's: dots 1 to 6 only.
const DOTS: [&[u8]; 26] = [
    &[1],
    &[1, 2],
    &[1, 4],
    &[1, 4, 5],
    &[1, 5],
    &[1, 2, 4],
    &[1, 2, 4, 5],
    &[1, 2, 5],
    &[2, 4],
    &[2, 4, 5],
    &[1, 3],
    &[1, 2, 3],
    &[1, 3, 4],
    &[1, 3, 4, 5],
    &[1, 3, 5],
    &[1, 2, 3, 4],
    &[1, 2, 3, 4, 5],
    &[1, 2, 3, 5],
    &[2, 3, 4],
    &[2, 3, 4, 5],
    &[1, 3, 6],
    &[1, 2, 3, 6],
    &[2, 4, 5, 6],
    &[1, 3, 4, 6],
    &[1, 3, 4, 5, 6],
    &[1, 3, 5, 6],
];

/// The letter of a chord of dots, dot 1 being bit 0 and dot 8 bit 7: the core's own a to z,
/// never the person's table. A chord with dot 7 or dot 8 is no letter: those dots are a
/// device's own keys (Orbit's back and open, a BTSpeak's capitals).
#[must_use]
pub fn braille_letter(dots: u8) -> Option<char> {
    DOTS.iter()
        .enumerate()
        .find(|(_, set)| set.iter().fold(0u8, |bits, dot| bits | (1 << (dot - 1))) == dots)
        .map(|(at, _)| char::from(b'a' + u8::try_from(at).unwrap_or(0)))
}

/// Where the next line starting with `letter` is, from the line after `from` on, wrapping round
/// and ending back at `from`; a line `hidden` marks is passed. For the lists a shell draws itself.
#[must_use]
pub fn first_letter(lines: &[String], hidden: &[bool], from: usize, letter: &str) -> Option<usize> {
    let wanted = letter.trim().to_lowercase();
    let total = lines.len();
    if wanted.is_empty() || total == 0 {
        return None;
    }
    (1..=total)
        .map(|offset| (from + offset) % total)
        .find(|&at| !hidden.get(at).copied().unwrap_or(false) && lines[at].trim_start().to_lowercase().starts_with(&wanted))
}

impl Lumenna {
    /// What `letter` runs on the row in hand: the row's first action of that letter (the shared
    /// table's command where it names one; a row with several of the kind asks which); else what
    /// the reminder list, the day or the place shown offers; else why it runs nothing here.
    pub(crate) fn letter(&self, letter: &str, model: &mut Screen) -> Command<Effect, Event> {
        let letter = letter.trim().to_lowercase();
        if model.acting == Some(Side::Due) {
            return self.due_letter(&letter, model);
        }
        let row = match model.acting {
            Some(side) => model.side_actions(side),
            None => model.actions_in_hand(),
        }
        .unwrap_or_default();
        let matching: Vec<Action> = row.iter().filter(|action| action_letter(action.kind) == Some(letter.as_str())).cloned().collect();
        if let Some(first) = matching.first() {
            let several: Vec<Action> = matching.iter().filter(|action| action.kind == first.kind).cloned().collect();
            if several.len() > 1 {
                model.open = Some(Open::new(Purpose::Which(several)));
                return crux_core::render::render();
            }
            let first = first.clone();
            return match command_for(first.kind) {
                Some(id) => self.command(id, model),
                None => self.perform(first, model),
            };
        }
        // What the list open offers of its own: Add Reminder.
        if model.reminders.is_some()
            && let Some(store) = &model.store
            && let Some(target) = &model.reminders
            && let Ok(list) = lumenna_records::reminders::reminders_of(&store.snapshot, target)
            && let Some(action) = list.actions.iter().find(|action| action_letter(action.kind) == Some(letter.as_str())).cloned()
        {
            return self.perform(action, model);
        }
        // Only the screen in front's own: a side list's stay out of the place behind it.
        if model.acting.is_none() && model.reminders.is_none() {
            if model.place() == Place::Today
                && let Some(day) = model.day_view()
                && let Some(action) = day.actions.iter().find(|action| action_letter(action.kind) == Some(letter.as_str())).cloned()
            {
                return self.perform(action, model);
            }
            let ids = place_command_ids(&model.place());
            if let Some(id) = ids.iter().find(|id| command_letter(id) == Some(letter.as_str())) {
                return self.place_command(id, model);
            }
        }
        if let Some(id) = ["undo", "redo"].iter().find(|id| command_letter(id) == Some(letter.as_str())) {
            return self.command(id, model);
        }
        self.refuse_letter(&letter, &row, model)
    }

    /// A place's own command: New Project Inside is the project row's, the rest the shared table's.
    fn place_command(&self, id: &str, model: &mut Screen) -> Command<Effect, Event> {
        if id == "new-project-inside" {
            let place = model.place();
            let action = model.store.as_ref().and_then(|store| {
                sidebar(&store.snapshot, &store.now)
                    .into_iter()
                    .find(|entry| matches!(&entry.kind, SidebarKind::Place(found) if *found == place))
                    .and_then(|entry| entry.actions.into_iter().find(|action| action.kind == K::NewInside))
            });
            return action.map_or_else(Command::done, |action| self.perform(action, model));
        }
        self.command(id, model)
    }

    /// Why `letter` runs nothing on the row in hand: in the words for the first of its kinds a
    /// row of this list offers (a side list's, not the main one's), so a task's words on a list
    /// of tasks, else the first the table has.
    fn refuse_letter(&self, letter: &str, row: &[Action], model: &mut Screen) -> Command<Effect, Event> {
        let shown: Vec<ActionKind> = match model.acting {
            Some(side) => model.side_kinds(side),
            None => model.rows().map(|rows| rows.shown.into_iter().flat_map(|(_, actions)| actions).map(|action| action.kind).collect()).unwrap_or_default(),
        };
        let kind = shown
            .into_iter()
            .find(|kind| action_letter(*kind) == Some(letter))
            .or_else(|| ACTION_LETTERS.iter().find(|(_, lettered)| *lettered == letter).map(|(kind, _)| *kind));
        match kind {
            Some(kind) => match command_for(kind).map(what_runs) {
                Some(Runs::Row(kinds)) => refuse(kinds[0], row, model),
                _ => refuse(kind, row, model),
            },
            None => refused(lumenna_records::words::nothing_on_letter(letter)),
        }
    }

    /// A reminder's letter: its button by that letter, sent as the button pressed, which refuses
    /// one it lacks in the core's words; else undo and redo.
    fn due_letter(&self, letter: &str, model: &mut Screen) -> Command<Effect, Event> {
        if let Some(id) = model.due_held.key.clone() {
            let which = DUE_LETTERS.iter().find(|(_, lettered)| *lettered == letter).map(|(which, _)| *which);
            let event = match which {
                Some("carry") => Some(DueEvent::Carry(id)),
                Some("replan") => Some(DueEvent::Replan(id)),
                Some("snooze") => Some(DueEvent::Snooze(id)),
                _ => None,
            };
            if let Some(event) = event {
                return self.due(event, model);
            }
        }
        if let Some(id) = ["undo", "redo"].iter().find(|id| command_letter(id) == Some(letter)) {
            return self.command(id, model);
        }
        refused(lumenna_records::words::nothing_on_letter(letter))
    }

    /// Moves the main list's row in hand to the next row, in the order shown and wrapping round,
    /// whose name starts with `letter`, passing folded rows; `landing` rises because the row
    /// moved. The row itself alone matching says its name. None found says so.
    pub(crate) fn find_letter(&self, letter: &str, model: &mut Screen) -> Command<Effect, Event> {
        let Some(rows) = model.names() else { return Command::done() };
        let names: Vec<String> = rows.iter().map(|(_, name)| name.clone()).collect();
        let from = model.focus.as_ref().and_then(|key| rows.iter().position(|(k, _)| k == key));
        match first_letter(&names, &[], from.unwrap_or(names.len().saturating_sub(1)), letter) {
            Some(at) if Some(at) == from => say(names[at].clone()),
            Some(at) => {
                let key = rows[at].0.clone();
                model.hold(&key);
                model.details.shown = None;
                model.moves += 1;
                crux_core::render::render()
            }
            None => refused(lumenna_records::words::no_row_starts_with(letter.trim())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_letter_has_its_dots_and_a_chord_with_dot_7_or_8_is_no_letter() {
        let dots = |list: &[u8]| list.iter().fold(0u8, |bits, dot| bits | (1 << (dot - 1)));
        assert_eq!(braille_letter(dots(&[1])), Some('a'));
        assert_eq!(braille_letter(dots(&[2, 4, 5, 6])), Some('w'));
        assert_eq!(braille_letter(dots(&[1, 3, 5, 6])), Some('z'));
        assert_eq!(braille_letter(dots(&[1, 7])), None);
        assert_eq!(braille_letter(dots(&[1, 8])), None);
        assert_eq!(braille_letter(0), None);
        assert_eq!(braille_letter(dots(&[2])), None, "a comma's dots are no letter");
    }

    #[test]
    fn the_26_letters_each_have_their_own_dots() {
        let mut seen = std::collections::BTreeSet::new();
        for letter in 'a'..='z' {
            let at = usize::from(u8::try_from(letter).unwrap() - b'a');
            let bits = DOTS[at].iter().fold(0u8, |bits, dot| bits | (1 << (dot - 1)));
            assert_eq!(braille_letter(bits), Some(letter));
            assert!(seen.insert(bits), "{letter} shares its dots");
        }
    }

    #[test]
    fn first_letter_wraps_round_and_passes_what_is_folded() {
        let lines = ["Apples", "Bread", "Avocado", "Butter"].map(str::to_owned).to_vec();
        assert_eq!(first_letter(&lines, &[], 0, "a"), Some(2));
        assert_eq!(first_letter(&lines, &[], 2, "a"), Some(0), "wraps round to the first");
        assert_eq!(first_letter(&lines, &[false, false, true, false], 0, "A"), Some(0), "the rest are folded away, so back to itself");
        assert_eq!(first_letter(&lines, &[], 0, "z"), None);
        assert_eq!(first_letter(&[], &[], 0, "a"), None);
    }

    #[test]
    fn erase_permanently_and_erasing_everything_have_no_letter() {
        assert_eq!(action_letter(K::ErasePermanently), None);
        assert_eq!(action_letter(K::EraseEverythingDeleted), None);
        for (kind, _) in ACTION_LETTERS {
            assert_ne!(*kind, K::ErasePermanently);
        }
    }

    #[test]
    fn every_action_with_a_command_has_a_letter_and_every_command_it_runs_is_the_rows() {
        for (kind, id) in ACTION_COMMANDS {
            assert!(action_letter(*kind).is_some(), "{kind:?} has a command and no letter");
            assert!(matches!(what_runs(id), Runs::Row(_)), "{id} is not a row's command");
        }
    }

    #[test]
    fn a_place_command_has_a_letter_and_no_two_commands_on_a_screen_share_one() {
        for place in [Place::Today, Place::Blocks, Place::Tasks, Place::Project("x".to_owned()), Place::Trash, Place::History] {
            let mut seen = std::collections::BTreeSet::new();
            for command in place_commands(&place, true) {
                assert!(seen.insert(command.letter.clone()), "{} is used twice on {place:?}", command.letter);
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for command in main_menu(true) {
            assert!(seen.insert(command.letter.clone()), "{} is used twice on the main menu", command.letter);
        }
    }

    fn settings(voice_role: RolePlace, braille_role: RolePlace) -> LineSettings {
        LineSettings { voice_role, braille_role, voice_letter: true, braille_letter: true, voice_hot_key: false, braille_hot_key: false, index: true }
    }

    fn signs() -> BrailleSigns {
        BrailleSigns { open: "\"<".to_owned(), close: "\">".to_owned(), dialog: "\"\"\"".to_owned(), pull_down: " 3o".to_owned() }
    }

    fn line(role: ControlRole, text: &str) -> DeviceLine {
        DeviceLine {
            role,
            label: String::new(),
            speech: text.to_owned(),
            text: text.to_owned(),
            detail: String::new(),
            letter: None,
            hot_key: None,
            asks: false,
            position: 0,
            count: 0,
        }
    }

    fn shown(reading: &DeviceReading) -> String {
        reading.before.iter().map(|p| p.text.as_str()).chain([reading.body.as_str()]).chain(reading.after.iter().map(|p| p.text.as_str())).collect()
    }

    #[test]
    fn a_menu_item_names_its_role_where_each_setting_puts_it() {
        let item = DeviceLine { letter: Some("n".to_owned()), ..line(ControlRole::MenuItem, "New") };
        let reading = device_reading(&item, settings(RolePlace::After, RolePlace::Before), &signs());
        assert_eq!(reading.speech, "New (n) menu item");
        assert_eq!(shown(&reading), "mi New \"<n\">");
        assert!(reading.before[0].computer);
    }

    #[test]
    fn an_item_that_opens_a_dialog_says_so_before_its_role_and_ends_in_the_devices_sign() {
        let item = DeviceLine { letter: Some("r".to_owned()), asks: true, ..line(ControlRole::MenuItem, "Rename") };
        let reading = device_reading(&item, settings(RolePlace::After, RolePlace::Before), &signs());
        assert_eq!(reading.speech, "Rename (r) dialog menu item");
        assert_eq!(shown(&reading), "mi Rename \"<r\"> \"\"\"");
    }

    #[test]
    fn a_submenu_is_a_pulldown_and_only_a_submenu_is() {
        let item = DeviceLine { letter: Some("f".to_owned()), ..line(ControlRole::Menu, "File") };
        let reading = device_reading(&item, settings(RolePlace::After, RolePlace::Before), &signs());
        assert_eq!(reading.speech, "File (f) pulldown Menu");
        assert_eq!(shown(&reading), "mn File \"<f\"> 3o");
        let plain = device_reading(&line(ControlRole::MenuItem, "New"), settings(RolePlace::After, RolePlace::Before), &signs());
        assert!(!plain.speech.contains("pulldown"));
    }

    #[test]
    fn an_edit_box_shows_its_label_ahead_of_its_text() {
        let field = DeviceLine { label: "Title".to_owned(), ..line(ControlRole::EditBox, "Buy milk") };
        let reading = device_reading(&field, settings(RolePlace::After, RolePlace::Before), &signs());
        assert_eq!(reading.speech, "Title: Buy milk edit box");
        assert_eq!(reading.body, "Buy milk");
        assert_eq!(shown(&reading), "eb Title: Buy milk");
    }

    #[test]
    fn a_combo_box_gives_its_place_only_when_the_device_reports_indexes() {
        let combo = DeviceLine { label: "Priority".to_owned(), position: 2, count: 5, ..line(ControlRole::ComboBox, "High") };
        let reading = device_reading(&combo, settings(RolePlace::After, RolePlace::Before), &signs());
        assert_eq!(reading.speech, "Priority: High 2/5 combo box");
        assert_eq!(shown(&reading), "cb Priority: High 2/5");
        let quiet = device_reading(&combo, LineSettings { index: false, ..settings(RolePlace::After, RolePlace::Before) }, &signs());
        assert_eq!(quiet.speech, "Priority: High combo box");
    }

    #[test]
    fn roles_set_off_leave_the_line_alone_and_after_puts_them_last() {
        let row = line(ControlRole::ListItem, "Today");
        assert_eq!(device_reading(&row, settings(RolePlace::Off, RolePlace::Off), &signs()).speech, "Today");
        assert_eq!(shown(&device_reading(&row, settings(RolePlace::Off, RolePlace::Off), &signs())), "Today");
        assert_eq!(device_reading(&row, settings(RolePlace::Before, RolePlace::After), &signs()).speech, "list item Today");
        assert_eq!(shown(&device_reading(&row, settings(RolePlace::Before, RolePlace::After), &signs())), "Today li");
    }

    #[test]
    fn a_letter_comes_after_the_name_and_before_the_detail() {
        let row = DeviceLine { letter: Some("t".to_owned()), detail: "3 tasks".to_owned(), ..line(ControlRole::ListItem, "Today") };
        let reading = device_reading(&row, settings(RolePlace::Off, RolePlace::Off), &signs());
        assert_eq!(reading.speech, "Today (t), 3 tasks");
        assert_eq!(shown(&reading), "Today \"<t\"> 3 tasks");
    }
}
