//! A braille device's letters: every braille device's keys run the same thing for the same
//! letter, and how a letter is pressed is each app's own: it maps its presses onto `Event::Letter`
//! and `Event::FindLetter`, and nothing here assumes which. (A BTSpeak's plain letter is the
//! command and a capital, Dot 7 with the letter, moves by first letter; on the Orbit Speak and the
//! BrailleSense, Space with the letter is the command and the letter alone moves, Enter (the
//! Speak's Dot 8) with the letter where the device keeps Space with it: [`speak_letter`].) What a
//! shell shows beside a row's action, a settings page or a reminder's button is asked for here.
//! Nothing a shell keeps.
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
use crate::keys::{Runs, what_runs};
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
    &[("sync-now", "n"), ("pair", "p"), ("back-up", "b"), ("restore-backup", "r"), ("import", "i")];

/// The letter of each of a reminder's buttons, by `DueButton::id`. Dismiss is the delete keys'.
const DUE_LETTERS: &[(&str, &str)] = &[("carry", "f"), ("replan", "p"), ("snooze", "z")];

/// The letter of each of a screen's own actions (`screen_actions`), by the shared table's `id`
/// where it has one, and of Home's places. No two on one screen share a letter (a test holds
/// them to it): the screens are each place's and Home.
const COMMAND_LETTERS: &[(&str, &str)] = &[
    ("undo", "u"),
    ("redo", "y"),
    ("new-task", "a"),
    ("filter", "/"),
    ("new-block", "a"),
    ("what-now", "w"),
    ("previous-day", "p"),
    ("next-day", "n"),
    ("go-to-day", "g"),
    ("go-to-now", "t"),
    ("new-project-inside", "p"),
    // Home.
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
    ("keyboard-help", "e"),
    ("about", "o"),
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

/// The letter of each menu of a menu bar, by `keys::Group`'s `id`.
const GROUP_LETTERS: &[(&str, &str)] = &[("file", "f"), ("edit", "e"), ("view", "v"), ("task", "t"), ("day", "d"), ("help", "h")];

/// A command's letter in a menu: its own, or the letter of the row's action it runs.
fn menu_letter(id: &str) -> Option<&'static str> {
    command_letter(id).or_else(|| {
        ACTION_COMMANDS.iter().find(|(_, command)| *command == id).and_then(|(kind, _)| action_letter(*kind))
    })
}

/// An item of a menu and the letter it would have.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuShortcut {
    /// Its name.
    pub title: String,
    /// The letter the core gives it, if any.
    pub letter: Option<String>,
}

/// The letter each of a menu's items shows, empty for one left with none: what the core gives
/// them first, so a name's own first letter never takes one of those, then for the rest the
/// first letter of its name still free. One that has none left shows none: a letter that ran
/// another item would mislead.
#[must_use]
pub fn menu_shortcuts(items: &[MenuShortcut]) -> Vec<String> {
    let mut taken = std::collections::BTreeSet::new();
    let mut letters: Vec<String> = items
        .iter()
        .map(|item| item.letter.clone().filter(|letter| taken.insert(letter.clone())).unwrap_or_default())
        .collect();
    for (item, letter) in items.iter().zip(&mut letters) {
        if letter.is_empty()
            && let Some(free) = item.title.to_lowercase().chars().find(|c| c.is_alphanumeric() && !taken.contains(&c.to_string()))
        {
            *letter = free.to_string();
            taken.insert(letter.clone());
        }
    }
    letters
}

/// One item of a braille device's menu bar.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetteredItem {
    /// The command: "new-task".
    pub id: String,
    /// Its name; a shell takes `ViewModel::commands`' over it where that has one.
    pub title: String,
    /// Its letter in its menu, empty for none.
    pub letter: String,
    /// Whether it asks something first.
    pub asks: bool,
    /// Whether its letter also runs it from the screen under the menu, so is a hot key there.
    pub hot_key: bool,
}

/// One menu of a braille device's menu bar.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetteredMenu {
    /// Which menu: "file", "edit", ….
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its letter in the bar.
    pub letter: String,
    /// Its items, in order.
    pub items: Vec<LetteredItem>,
}

/// What a device with no windows or panes has no use for from the shared menu bar. Saving a
/// task is its form's own button.
const NOT_ON_A_BRAILLE_DEVICE: &[&str] = &["show", "next-pane", "previous-pane", "new-window", "save-task"];

/// What only a day shown can do, and what only a list being shown can.
const DAY_MOVES: &[&str] = &["previous-day", "next-day", "go-to-now", "go-to-day"];

/// The groups of places a device without a sidebar reaches from the menu bar, after the places.
const BRAILLE_VIEW_EXTRAS: &[&str] = &["projects", "labels", "filters", "reminders-due"];

/// What Home's places run, by their letters' ids: they are hot keys on Home.
const HOME_PLACES: &[&str] = &["go-today", "go-tasks", "go-blocks", "go-history", "go-trash", "projects", "labels", "filters"];

/// The shared menu bar (`keys::MENUS`) on a braille device: the same menus, commands and order
/// as every other platform's, each with its letter. `place` is the place shown under the menu
/// (`None` over anything else), whose own actions (`screen_actions`) are all there, keep the
/// letters they share with another and are real hot keys; on `home` (Home shown) Home's are,
/// its places' among them. What the table lacks that a device with no sidebar needs is added
/// here, and the Day menu is there while a day is, and on Home with What Should I Work On?.
#[must_use]
pub fn menu_bar(place: Option<&Place>, home: bool, sentence: bool) -> Vec<LetteredMenu> {
    let place = place.filter(|_| !home);
    let mut own: Vec<&str> = crate::screen_actions::screen_action_ids(place, home, true);
    if home {
        own.extend(HOME_PLACES);
    }
    let on_list = own.contains(&"filter");
    let is_day = place == Some(&Place::Today);
    let on_rows = place.is_some();

    let groups = crate::keys::menus(sentence).into_iter().filter(|group| group.id != "day" || is_day || home).map(|mut group| {
        let day = group.id == "day";
        let mut ids: Vec<(String, String, bool)> = group
            .items
            .drain(..)
            .filter(|item| !item.id.is_empty() && !NOT_ON_A_BRAILLE_DEVICE.contains(&item.id.as_str()))
            .filter(|item| (item.id != "filter" || on_list) && (is_day || !DAY_MOVES.contains(&item.id.as_str())))
            // Off a day, its menu holds only what this screen offers of its own.
            .filter(|item| is_day || !day || own.contains(&item.id.as_str()))
            .map(|item| (item.id, item.title, item.asks))
            .collect();
        if group.id == "view" {
            ids.extend(BRAILLE_VIEW_EXTRAS.iter().map(|id| ((*id).to_owned(), command_title(id, sentence), false)));
        }
        if group.id == "file" && own.contains(&"new-project-inside") {
            ids.insert(1, ("new-project-inside".to_owned(), command_title("new-project-inside", sentence), true));
        }
        (group, ids)
    });

    let groups: Vec<_> = groups.collect();
    let bar = menu_shortcuts(
        &groups
            .iter()
            .map(|(group, _)| MenuShortcut {
                title: group.title.clone(),
                letter: found(GROUP_LETTERS, &group.id).map(str::to_owned),
            })
            .collect::<Vec<_>>(),
    );
    groups
        .into_iter()
        .zip(bar)
        .map(|((group, ids), letter)| {
            // A letter this screen's own command has is that command's, in this menu as on the row.
            let theirs: Vec<&str> = ids.iter().filter(|(id, ..)| own.contains(&id.as_str())).filter_map(|(id, ..)| command_letter(id)).collect();
            let shortcuts = menu_shortcuts(
                &ids.iter()
                    .map(|(id, title, _)| MenuShortcut {
                        title: title.clone(),
                        letter: menu_letter(id)
                            .filter(|letter| own.contains(&id.as_str()) || !theirs.contains(letter))
                            .map(str::to_owned),
                    })
                    .collect::<Vec<_>>(),
            );
            let items = ids
                .into_iter()
                .zip(shortcuts)
                .map(|((id, title, asks), letter)| {
                    let row = on_rows && ACTION_COMMANDS.iter().any(|(_, command)| *command == id);
                    // On a day, the day's own actions run by their letters too.
                    let day = is_day && matches!(what_runs(&id), Runs::Day(kinds) if action_letter(kinds[0]) == Some(letter.as_str()));
                    let hot_key = !letter.is_empty() && menu_letter(&id) == Some(letter.as_str()) && (own.contains(&id.as_str()) || row || day);
                    LetteredItem { id, title, letter, asks, hot_key }
                })
                .collect();
            LetteredMenu { id: group.id, title: group.title, letter, items }
        })
        .collect()
}

fn command_for(kind: ActionKind) -> Option<&'static str> {
    ACTION_COMMANDS.iter().find(|(lettered, _)| *lettered == kind).map(|(_, id)| *id)
}

/// A command's name by its `id`, the shared table's where it has one.
fn command_title(id: &str, sentence: bool) -> String {
    let title = match id {
        "projects" => "Projects",
        "labels" => "Labels",
        "filters" => "Saved Filters",
        other => return crate::screen_actions::action_title(other, sentence),
    };
    if sentence { lumenna_records::fields::sentence_case(title.to_owned()) } else { title.to_owned() }
}

/// How a letter is pressed on the Orbit Speak.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakLetter {
    /// Space with it.
    Space,
    /// Dot 8 (Enter) with it, as the Speak's own apps press their commands.
    Enter,
    /// Neither reaches the app: its row of a menu says no key.
    Neither,
}

/// Space with these is the Speak's while Lumenna is in front, by Orbit's screen reader and its user
/// guide: the screen reader keeps C (read the line) and T; on every screen A and B are Up and
/// Shift-Tab, E and Z go back, L goes to the top and M opens the menu. What Orbit's own apps do
/// with Space and a letter is theirs alone, in front only when they are.
const SPEAK_ENTER: &str = "abcelmtz";

/// Neither Space nor Dot 8 with G reaches an app: the screen reader keeps both (the input mode,
/// reading on from the caret). What G names is a menu away.
const SPEAK_NEITHER: &str = "g";

/// How a letter is pressed on the HIMS BrailleSense.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SenseLetter {
    /// Space with it.
    Space,
    /// Enter with it, as the device's own hotkeys are pressed.
    Enter,
}

/// Space with these is already a command on every screen of the BrailleSense, so a letter is
/// pressed with Enter there: Space-1 is A (Up), Space-1-2 B (Shift-Tab), Space-1-3 K (the line's
/// start), Space-1-2-3 L (the first row), and C, D, E, F, H, M, O, R and Z are the manual's own
/// (Repeat, Delete, Escape, Find, Help, Menu, Options, Repeat, Exit).
const SENSE_ENTER: &str = "abcdefhklmorz";

/// How `letter` is pressed on the BrailleSense; none for one that is not a letter, which has no
/// dots to press (`,`, `.`, `/`).
#[must_use]
pub fn braillesense_letter(letter: &str) -> Option<SenseLetter> {
    let first = letter.chars().next().filter(|first| first.is_ascii_lowercase() && letter.len() == 1)?;
    Some(if SENSE_ENTER.contains(first) { SenseLetter::Enter } else { SenseLetter::Space })
}

/// A letter's press on the BrailleSense as the device names a key, for a menu to show beside the
/// item: "Space-u", "Enter-c"; none for one with no press.
#[must_use]
pub fn braillesense_press(letter: &str) -> Option<String> {
    match braillesense_letter(letter)? {
        SenseLetter::Space => Some(format!("Space-{letter}")),
        SenseLetter::Enter => Some(format!("Enter-{letter}")),
    }
}

/// The letters the BrailleSense presses with Enter, as its key help names them: "A, B, C … and Z".
#[must_use]
pub fn braillesense_enter_letters() -> String {
    let letters: Vec<String> = SENSE_ENTER.chars().map(|c| c.to_ascii_uppercase().to_string()).collect();
    match letters.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => letters.concat(),
    }
}

/// How `letter` is pressed on the Orbit Speak.
#[must_use]
pub fn speak_letter(letter: &str) -> SpeakLetter {
    match letter.chars().next().filter(|first| first.is_ascii_lowercase() && letter.len() == 1) {
        None => SpeakLetter::Neither,
        Some(first) if SPEAK_NEITHER.contains(first) => SpeakLetter::Neither,
        Some(first) if SPEAK_ENTER.contains(first) => SpeakLetter::Enter,
        Some(_) => SpeakLetter::Space,
    }
}

/// How a letter's press is said after what it runs on the Speak ("dot 8 c", "space u"), none for
/// one with no press.
#[must_use]
pub fn speak_press(letter: &str) -> Option<String> {
    match speak_letter(letter) {
        SpeakLetter::Space => Some(format!("space {letter}")),
        SpeakLetter::Enter => Some(format!("dot 8 {letter}")),
        SpeakLetter::Neither => None,
    }
}

/// Space with a letter that takes Dot 8 here, refused: which to press.
#[must_use]
pub fn speak_press_instead(letter: &str) -> String {
    format!("Press dot 8 with {letter}.")
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
        // On Home a letter is its places' and its own actions' first, as in a device's menu.
        if model.home
            && matches!(model.acting, None | Some(Side::Sidebar))
            && let Some(ran) = self.home_letter(&letter, model)
        {
            return ran;
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
        if model.acting.is_none() && model.reminders.is_none() && !model.home {
            if model.place() == Place::Today
                && let Some(day) = model.day_view()
                && let Some(action) = day.actions.iter().find(|action| action_letter(action.kind) == Some(letter.as_str())).cloned()
            {
                return self.perform(action, model);
            }
            let ids = crate::screen_actions::place_action_ids(&model.place());
            // The filter is the shell's own field, which its letter has no dots to reach.
            if let Some(id) = ids.iter().filter(|id| **id != "filter").find(|id| command_letter(id) == Some(letter.as_str())) {
                return self.command(id, model);
            }
        }
        if let Some(id) = ["undo", "redo"].iter().find(|id| command_letter(id) == Some(letter.as_str())) {
            return self.command(id, model);
        }
        self.refuse_letter(&letter, &row, model)
    }

    /// What `letter` runs on Home: what Enter does on the place of that letter (a place opened, a
    /// heading put in hand and folded or unfolded), else one of Home's own actions; none for a
    /// letter Home does not give, or one of an action that is the shell's own, which a shell runs
    /// itself.
    fn home_letter(&self, letter: &str, model: &mut Screen) -> Option<Command<Effect, Event>> {
        let entry = model.store.as_ref().and_then(|store| {
            sidebar(&store.snapshot, &store.now)
                .into_iter()
                .find(|entry| crate::screen_actions::place_letter(&entry.kind) == Some(letter))
        });
        if let Some(entry) = entry {
            let keys = model.side_keys(Side::Sidebar);
            model.sidebar_held.hold(&entry.key, &keys);
            model.acting = Some(Side::Sidebar);
            if matches!(entry.kind, SidebarKind::Group(_)) {
                model.sidebar_held.landing += 1;
            }
            return Some(self.open_held_place(model));
        }
        let action = model.screen_actions().into_iter().find(|action| action.letter == letter && !action.shell)?;
        Some(self.command(&action.id, model))
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
            if let Some(event) = which.and_then(|button| DueEvent::pressed(&id, button)) {
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
    fn the_speak_presses_a_letter_it_keeps_space_for_with_dot_8() {
        assert_eq!(speak_letter("u"), SpeakLetter::Space);
        assert_eq!(speak_letter("c"), SpeakLetter::Enter, "the screen reader keeps Space with C");
        assert_eq!(speak_letter("m"), SpeakLetter::Enter);
        assert_eq!(speak_letter("d"), SpeakLetter::Space, "the Editor's D is the Editor's alone");
        assert_eq!(speak_letter("g"), SpeakLetter::Neither);
        assert_eq!(speak_letter(","), SpeakLetter::Neither);
        assert_eq!(speak_press("u").as_deref(), Some("space u"));
        assert_eq!(speak_press("c").as_deref(), Some("dot 8 c"));
        assert_eq!(speak_press("g"), None);
    }

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
    fn every_screen_action_has_a_letter_and_no_two_on_a_screen_share_one() {
        use crate::screen_actions::{screen_action_ids, every_kind_of_place, place_letter};
        for place in every_kind_of_place() {
            let mut seen = std::collections::BTreeSet::new();
            for id in screen_action_ids(Some(&place), false, true) {
                let letter = command_letter(id).unwrap_or_else(|| panic!("{id} has no letter on {place:?}"));
                assert!(seen.insert(letter), "{letter} is used twice on {place:?}");
            }
        }
        // Home: its places' letters and its own actions' together.
        let mut seen = std::collections::BTreeSet::new();
        let places = [
            SidebarKind::Place(Place::Today),
            SidebarKind::Place(Place::Tasks),
            SidebarKind::Place(Place::Blocks),
            SidebarKind::Place(Place::History),
            SidebarKind::Place(Place::Trash),
            SidebarKind::Group(lumenna_records::places::SidebarGroup::Projects),
            SidebarKind::Group(lumenna_records::places::SidebarGroup::Labels),
            SidebarKind::Group(lumenna_records::places::SidebarGroup::Filters),
        ];
        for kind in &places {
            let letter = place_letter(kind).unwrap_or_else(|| panic!("{kind:?} has no letter on Home"));
            assert!(seen.insert(letter), "{letter} is used twice on Home");
        }
        for id in screen_action_ids(None, true, true) {
            let letter = command_letter(id).unwrap_or_else(|| panic!("{id} has no letter on Home"));
            assert!(seen.insert(letter), "{letter} is used twice on Home");
        }
    }

#[test]    fn a_command_has_one_letter_wherever_it_is_a_button_too() {        for (id, letter) in BUTTON_LETTERS {            if let Some(command) = command_letter(id) {                assert_eq!(command, *letter, "{id} has two letters");            }        }    }
    #[test]
    fn the_braillesense_presses_with_enter_what_space_already_does() {
        assert_eq!(braillesense_letter("u"), Some(SenseLetter::Space));
        assert_eq!(braillesense_letter("a"), Some(SenseLetter::Enter), "Space-1 is Up");
        assert_eq!(braillesense_letter("m"), Some(SenseLetter::Enter), "Space-M is the menu");
        assert_eq!(braillesense_letter("/"), None);
        assert_eq!(braillesense_press("y").as_deref(), Some("Space-y"));
        assert_eq!(braillesense_press("c").as_deref(), Some("Enter-c"));
        assert_eq!(braillesense_enter_letters(), "A, B, C, D, E, F, H, K, L, M, O, R and Z");
    }

    fn shortcut(title: &str, letter: Option<&str>) -> MenuShortcut {
        MenuShortcut { title: title.to_owned(), letter: letter.map(str::to_owned) }
    }

    #[test]
    fn a_menus_letters_are_the_cores_first_then_a_name_and_none_when_none_is_left() {
        let items = [shortcut("Today", None), shortcut("Tasks", Some("t")), shortcut("New", Some("t")), shortcut("o", None)];
        assert_eq!(menu_shortcuts(&items), ["o", "t", "n", ""]);
    }

    fn bar(place: Option<&Place>, main: bool) -> Vec<LetteredMenu> {
        menu_bar(place, main, true)
    }

    #[test]
    fn the_menu_bar_is_the_shared_table_in_its_order() {
        let shared: Vec<String> = crate::keys::menus(true).into_iter().map(|menu| menu.id).collect();
        let ids: Vec<String> = bar(Some(&Place::Today), false).into_iter().map(|menu| menu.id).collect();
        assert_eq!(ids, shared);
        let view = bar(Some(&Place::Today), false).into_iter().find(|menu| menu.id == "view").unwrap();
        let shared_view: Vec<String> = crate::keys::menus(true).into_iter().find(|menu| menu.id == "view").unwrap().items.into_iter()
            .filter(|item| !item.id.is_empty() && !NOT_ON_A_BRAILLE_DEVICE.contains(&item.id.as_str())).map(|item| item.id).collect();
        let mine: Vec<String> = view.items.iter().map(|item| item.id.clone()).take(shared_view.len()).collect();
        assert_eq!(mine, shared_view);
    }

    #[test]
    fn no_two_items_of_a_menu_and_no_two_menus_share_a_letter() {
        for place in [Some(Place::Today), Some(Place::Blocks), Some(Place::Tasks), Some(Place::Project("x".to_owned())), None] {
            let menus = bar(place.as_ref(), place.is_none());
            let mut tops = std::collections::BTreeSet::new();
            for menu in menus {
                assert!(tops.insert(menu.letter.clone()), "{} twice in the bar", menu.letter);
                let mut seen = std::collections::BTreeSet::new();
                for item in menu.items {
                    assert!(item.letter.is_empty() || seen.insert(item.letter.clone()), "{} twice in {}", item.letter, menu.id);
                }
            }
        }
    }

    #[test]
    fn a_screens_own_command_keeps_its_letter_in_the_menu() {
        let today = bar(Some(&Place::Today), false);
        let file = today.iter().find(|menu| menu.id == "file").unwrap();
        assert_eq!(file.items.iter().find(|item| item.id == "new-block").unwrap().letter, "a");
        let tasks = bar(Some(&Place::Tasks), false);
        let file = tasks.iter().find(|menu| menu.id == "file").unwrap();
        assert_eq!(file.items.iter().find(|item| item.id == "new-task").unwrap().letter, "a");
    }

    #[test]
    fn the_day_menu_is_there_only_on_a_day_and_the_filter_only_on_a_list() {
        assert!(bar(Some(&Place::Today), false).iter().any(|menu| menu.id == "day"));
        assert!(!bar(Some(&Place::Tasks), false).iter().any(|menu| menu.id == "day"));
        let has_filter = |menus: Vec<LetteredMenu>| menus.iter().any(|menu| menu.items.iter().any(|item| item.id == "filter"));
        assert!(has_filter(bar(Some(&Place::Tasks), false)));
        assert!(!has_filter(bar(Some(&Place::Today), false)));
        assert!(!has_filter(bar(None, true)));
    }

    #[test]
    fn a_hot_key_is_shown_only_where_the_letter_runs_it() {
        let item = |menus: &[LetteredMenu], id: &str| menus.iter().flat_map(|menu| &menu.items).find(|item| item.id == id).cloned().unwrap();
        let tasks = bar(Some(&Place::Tasks), false);
        assert!(item(&tasks, "undo").hot_key);
        assert!(!item(&tasks, "go-today").hot_key);
        assert!(item(&tasks, "mark-done").hot_key);
        let main = bar(None, true);
        assert!(item(&main, "go-today").hot_key);
        let elsewhere = bar(None, false);
        assert!(!item(&elsewhere, "go-today").hot_key);
        assert!(!item(&elsewhere, "mark-done").hot_key);
    }
}
