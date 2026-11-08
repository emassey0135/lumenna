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
}
