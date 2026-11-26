//! How each braille device presses a letter, and its key help: one rule for every device.
//!
//! Every braille device runs the same thing for the same letter (`braille`); what differs is the
//! key that presses it there. A device is described here by that alone: whether a letter's command
//! is the letter by itself (the BTSpeak, whose capital moves by first letter) or Space with it (the
//! rest, where the letter alone moves by first letter), which letters take Enter instead because
//! the device keeps Space with them, and which reach no app at all. Its key help is then made from
//! the view as it is: the device's own keys, Home's places, the screen's own commands and the
//! actions of the rows shown, each by the keys that press its letter there; a command whose letter
//! has no press there is a menu away and left out.

use lumenna_records::ActionKind;
use serde::{Deserialize, Serialize};

use crate::ViewModel;
use crate::words::{DeviceKey, ShellWords, ThisDevice, shell_words};
use crate::Capitals;

/// A braille device, for how its letters are pressed and what its key help says.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDevice {
    /// A BTSpeak: a letter by itself is its command, and a capital (Dot 7 with it) moves.
    BtSpeak,
    /// A HumanWare braille reader with other makers' apps on it (the NLS eReader, the Mantis Q40,
    /// the Chameleon 20, the BrailleOne, the Brailliant BI 20X and 40X).
    Reader,
    /// The HIMS BrailleSense.
    BrailleSense,
    /// The Orbit Speak.
    OrbitSpeak,
    /// The Orbit Player: a phone's keypad and no braille keys, so no letter is pressed on it.
    OrbitPlayer,
}

/// How a letter's command is pressed on a device.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LetterPress {
    /// The letter by itself.
    Alone,
    /// Space with it.
    Space,
    /// Enter with it (the Orbit Speak's Dot 8), where the device keeps Space with it.
    Enter,
    /// No key reaches the app: what it names is a menu away.
    Neither,
}

/// A device's rule for its letters.
struct Rule {
    /// How a letter's command is pressed where nothing else is said.
    command: LetterPress,
    /// The letters pressed with Enter, since the device keeps Space with them.
    enter: &'static str,
    /// The letters no key reaches the app with.
    neither: &'static str,
}

fn rule(device: KeyDevice) -> Rule {
    match device {
        KeyDevice::BtSpeak => Rule { command: LetterPress::Alone, enter: "", neither: "" },
        // Space with these is the firmware's on a list of some reader: A (the line before), G, H
        // (the eReader's menu key, the others' global shortcuts), M (the eReader's main menu, the
        // others' menu key) and O (the Brailliants'). E (Exit) and L are the firmware's with Space
        // and with Enter both.
        KeyDevice::Reader => Rule { command: LetterPress::Space, enter: "aghmo", neither: "el" },
        // Space with these is already a command on every screen of the BrailleSense: Space-1 is A
        // (Up), Space-1-2 B (Shift-Tab), Space-1-3 K (the line's start), Space-1-2-3 L (the first
        // row), and C, D, E, F, H, M, O, R and Z are the manual's own (Repeat, Delete, Escape,
        // Find, Help, Menu, Options, Repeat, Exit).
        KeyDevice::BrailleSense => Rule { command: LetterPress::Space, enter: "abcdefhklmorz", neither: "" },
        // Space with these is the Speak's while Lumenna is in front, by Orbit's screen reader and
        // its user guide: the screen reader keeps C (read the line) and T; on every screen A and B
        // are Up and Shift-Tab, E and Z go back, L goes to the top and M opens the menu. G is the
        // screen reader's with Space and with Dot 8 both (the input mode, reading on).
        KeyDevice::OrbitSpeak => Rule { command: LetterPress::Space, enter: "abcelmtz", neither: "g" },
        KeyDevice::OrbitPlayer => Rule { command: LetterPress::Neither, enter: "", neither: "" },
    }
}

/// How `letter`'s command is pressed on `device`. Only a to z have dots: `,`, `.` and `/` are a
/// menu away on a device that reports dots.
#[must_use]
pub fn letter_press(device: KeyDevice, letter: &str) -> LetterPress {
    let Some(first) = letter.chars().next().filter(|first| first.is_ascii_lowercase() && letter.len() == 1) else {
        return LetterPress::Neither;
    };
    let rule = rule(device);
    if rule.neither.contains(first) {
        LetterPress::Neither
    } else if rule.enter.contains(first) {
        LetterPress::Enter
    } else {
        rule.command
    }
}

/// The keys that press `letter`'s command on `device`, as the device names keys: "u" on a BTSpeak,
/// "Space+U" and "Enter+A" on a reader (as its firmware's own help names them), "Space-U" and
/// "Enter-A" on the BrailleSense (as its manual does), "space u" and "dot 8 a" on the Speak (as its
/// screen reader says them); none where no key reaches the app.
#[must_use]
pub fn press_name(device: KeyDevice, letter: &str) -> Option<String> {
    let capital = letter.to_uppercase();
    match (device, letter_press(device, letter)) {
        (_, LetterPress::Neither) => None,
        (_, LetterPress::Alone) => Some(letter.to_owned()),
        (KeyDevice::Reader, LetterPress::Space) => Some(format!("Space+{capital}")),
        (KeyDevice::Reader, LetterPress::Enter) => Some(format!("Enter+{capital}")),
        (KeyDevice::OrbitSpeak, LetterPress::Space) => Some(format!("space {letter}")),
        (KeyDevice::OrbitSpeak, LetterPress::Enter) => Some(format!("dot 8 {letter}")),
        (_, LetterPress::Space) => Some(format!("Space-{capital}")),
        (_, LetterPress::Enter) => Some(format!("Enter-{capital}")),
    }
}

/// What a device says when Space with `letter` reaches the app though Enter presses its command
/// there: which to press instead. None for a letter pressed otherwise.
#[must_use]
pub fn press_instead(device: KeyDevice, letter: &str) -> Option<String> {
    if letter_press(device, letter) != LetterPress::Enter {
        return None;
    }
    Some(match device {
        KeyDevice::OrbitSpeak => format!("Press dot 8 with {letter}."),
        _ => format!("Press {}.", press_name(device, letter)?),
    })
}

/// `text` (a row's, a menu item's, a button's) with the keys that press its `letter` on `device`
/// after it, as a device that says a letter's press beside the thing says it: "Today, space t";
/// `text` alone where no key presses it.
#[must_use]
pub fn with_press(device: KeyDevice, text: &str, letter: &str) -> String {
    match press_name(device, letter) {
        Some(press) => format!("{text}, {press}"),
        None => text.to_owned(),
    }
}

/// A key help: its sections in order, and the same as lines to read one after another.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyHelp {
    /// Its name: "Key help".
    pub title: String,
    /// Each part under its heading.
    pub sections: Vec<KeySection>,
    /// Every heading and key in order, a line each, for a device that reads one list.
    pub lines: Vec<String>,
}

/// One part of a key help.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySection {
    /// Its heading: "Moving and opening", "Commands".
    pub heading: String,
    /// Its keys, each with what it does.
    pub keys: Vec<DeviceKey>,
}

/// `device`'s key help over what `view` shows, in `capitals`: the device's own keys; on
/// Home its places; the screen's own commands; then what the rows shown offer, each by the keys
/// that press its letter. A part with nothing in it is left out.
#[must_use]
pub fn key_help(device: KeyDevice, view: &ViewModel, capitals: Capitals) -> KeyHelp {
    let words = &shell_words(capitals, ThisDevice::Device);
    let braille = &words.braille;
    let mut sections = vec![KeySection { heading: braille.moving.clone(), keys: device_keys(device, words) }];
    let pressed = |letter: &str, what: &str| press_name(device, letter).map(|keys| DeviceKey::new(keys, what.to_owned()));
    // Letters Home takes from the rows on it: a row's action of one is no key there.
    let mut taken: Vec<&str> = Vec::new();
    if let Some(home) = &view.home {
        let keys = home.places.iter().filter_map(|place| pressed(&place.letter, &place.entry.title)).collect();
        sections.push(KeySection { heading: words.places.clone(), keys });
        taken.extend(home.places.iter().map(|place| place.letter.as_str()));
        taken.extend(home.actions.iter().map(|action| action.letter.as_str()));
    }
    let commands = view.screen_actions.iter().filter_map(|action| pressed(&action.letter, &action.title)).collect();
    sections.push(KeySection { heading: words.screen_actions.clone(), keys: commands });
    sections.push(KeySection { heading: braille.row_actions.clone(), keys: row_actions(device, view, &taken, capitals == Capitals::Sentence) });
    sections.retain(|section| !section.keys.is_empty());
    let lines = sections.iter().flat_map(|section| std::iter::once(section.heading.clone()).chain(section.keys.iter().map(|key| key.line.clone()))).collect();
    KeyHelp { title: braille.key_help.clone(), sections, lines }
}

/// The actions the rows shown offer, by letter in the alphabet's order, each letter's names
/// together: "Space+C: Mark done, Mark not done".
fn row_actions(device: KeyDevice, view: &ViewModel, taken: &[&str], sentence: bool) -> Vec<DeviceKey> {
    let offered = if view.home.is_some() {
        view.sidebar.iter().flat_map(|entry| &entry.offered).collect::<Vec<_>>()
    } else {
        let list = view.list.iter().flat_map(|list| &list.offered);
        let day = view.day.iter().flat_map(|day| day.offered.iter().chain(day.items.iter().flat_map(|item| &item.offered)));
        list.chain(day).collect()
    };
    let mut lettered: Vec<(&str, Vec<&str>)> = Vec::new();
    for offered in offered {
        let Some(letter) = crate::braille::action_letter(offered.kind) else { continue };
        if taken.contains(&letter) || offered.kind == ActionKind::ErasePermanently {
            continue;
        }
        // An action's name in the capitals asked for: its `sentence` is its title in sentence case.
        let title = if sentence { offered.sentence.as_str() } else { offered.title.as_str() };
        match lettered.iter_mut().find(|(seen, _)| *seen == letter) {
            Some((_, titles)) if !titles.contains(&title) => titles.push(title),
            Some(_) => {}
            None => lettered.push((letter, vec![title])),
        }
    }
    lettered.sort_by_key(|(letter, _)| *letter);
    lettered.into_iter().filter_map(|(letter, titles)| press_name(device, letter).map(|keys| DeviceKey::new(keys, titles.join(", ")))).collect()
}

/// The device's own keys: how it moves, opens, goes back, deletes and types, as its own apps and
/// manual name them, which is why each sits here with what it does; then how a row is found by
/// its first letter, which is the letter alone where Space presses a command and a capital where
/// the letter alone does.
fn device_keys(device: KeyDevice, words: &ShellWords) -> Vec<DeviceKey> {
    let row = |keys: &str, what: &str| DeviceKey::new(keys.to_owned(), what.to_owned());
    let mut keys = match device {
        KeyDevice::BtSpeak => vec![
            row("Enter", "Open the row, or press the button"),
            row("M-Chord with Dot 7", "The row's context menu, with everything that can be done to it"),
            row("M-Chord", "The menu bar: the screen's own menu first, then File, Edit, View, Task, Day and Help"),
            row("Tab and Shift-Tab in the menu bar", "The next and previous menu"),
            row("Z-Chord", "Leave the screen, or the menu bar"),
            row("H-Chord", "Help about the screen"),
            row("Control-D or D-Chord", "Delete the row"),
            row("Left and Right, or Dot 7-Chord and Dot 8-Chord", "Fold and unfold the row"),
            row("Control-L in a line", "What could come next where the cursor is"),
        ],
        KeyDevice::Reader => vec![
            // As the readers' own guides name their keys (the NLS eReader's Common Keyboard Commands).
            row("Up and Down, or Space+Dot1 and Space+Dot4", "The row before and after"),
            row("Space+Dots123, Space+Dots456", "The first and the last row"),
            row("Left and Right", "Pan through a long line"),
            row("Enter", "Open the row, or press the button"),
            row("Space+H on the eReader, Space+M on the others", "The row's actions first, then the screen's own menu, then File, Edit, View, Task, Day and Help"),
            row("Space+E", "Go back, close a dialog, or go up a level of a menu; on Home, leave Lumenna"),
            row("Space+D", "Delete the row"),
            row("Enter in a line", "Enter it; a line ending in a name begun, such as #wo, then offers what would finish it"),
        ],
        KeyDevice::BrailleSense => vec![
            row("Space-1 or the Up scroll key", "Previous row or option, or back through a long line"),
            row("Space-4 or the Down scroll key", "Next row or option, or on through a long line"),
            row("Space-1-2-3, Space-4-5-6", "First and last row"),
            row("Space-1-2-6, Space-3-4-5", "Previous and next page of 32 rows"),
            row("Enter", "Open the row, press the button"),
            row("Space-M or F2", "The menu bar"),
            row("Enter-I, or the Application key", "The row's own actions"),
            row("Space-3, Space-6", "Collapse and expand the row"),
            row("Space-E or F4", "Close, go back, or up one level of a menu"),
            row("Space-Z", "Exit Lumenna"),
            row("Space-4-5 or F3, Space-1-2 or Space-F3", "Next and previous control in a dialog"),
            row("Space", "Change a check box, a combo box, a prompt button, or mark a task done"),
            row("Space-F", "Filter the list"),
            row("Space-D", "Delete the row"),
            row("Space-H, or F1 on the QWERTY keyboard", "These keys"),
            row("Space-O", "Settings"),
            row("Space-R or Space-C", "Say the row again"),
            row("Space-3-6, Space-2-5", "In a line of text, say the character and the word at the cursor"),
            row("Enter-B, Enter-A", "In a line of text, start a selection at the cursor, or select everything"),
            row("Enter-C, Enter-X, Enter-V", "Copy, cut and paste"),
            row("Enter-P, Enter-D", "Add the selection to the clipboard, clear the clipboard"),
            row("Backspace-1-2, Enter-U", "Read the selection, and the first line of it"),
            row("Space-D, Backspace-2-5", "In a line of text, delete the character or the word at the cursor"),
            row("Backspace-1-4, Backspace-1-3-4-6", "Delete the line shown at the cursor, or the whole paragraph"),
            row("Space-4 in a line that completes", "What could come next where the cursor is"),
            row("Enter in a multi-line box", "A new line; Tab leaves the box"),
            row("Space-1, Space-4 in a multi-line box", "The line above and below"),
            row("A cell touched and a front Cursor button", "Move the cursor in a line of text to that cell"),
        ],
        KeyDevice::OrbitSpeak => vec![
            row("Up and Down, or Space with dot 1 and Space with dot 4", "Move through a list; held, they go to its ends"),
            row("Space with L, Space with dots 4 5 6", "The top and the bottom of a list"),
            row("Right, OK or dot 8", "Open the row; Right opens a folded row first"),
            row("Left or dot 7", "Go back out of a list; Left folds an open row first, and never leaves a dialog"),
            row("Space with E or Space with Z", "Always go back"),
            row("Space with M", "The menu: the row's actions first, then the screen's, then File, Edit, View, Task, Day and Help"),
            row("Up, Down, Right or dot 8, and Left in the menu", "Move, open a submenu, and go back up one, closing the menu from the top"),
            row("A letter alone in the menu", "The item of that letter; Space with an item's letter runs it from anywhere in the menu"),
            row("Space with D", "Delete the row"),
            row("Space with dots 4 5, Space with dots 1 2", "The next and previous control in a dialog"),
            row("Up and Down on a combo box or a check box", "Change it"),
            row("OK or dot 8 on a button", "Press it, or confirm the dialog"),
            row("Space, dot 7, and OK or dot 8 while typing", "End a word, delete, and finish"),
            row("Space with Z while typing", "Cancel"),
            row("Down while typing", "What could come next"),
            row("Dots 1 2 8 while typing", "Start a selection at the cursor and, pressed again, end it there"),
            row("Dots 1 8 while typing", "Select everything"),
            row("Dots 1 4 8, dots 1 3 4 6 8, dots 1 2 3 6 8 while typing", "Copy, cut and paste; in computer braille, with Space too"),
            row("Space with dots 1 2 7 while typing", "Read what is selected"),
        ],
        KeyDevice::OrbitPlayer => vec![
            row("Up and Down", "Move through the list; held, they go to its ends"),
            row("OK", "Open the row"),
            row("Right and Left", "Unfold a folded row, and fold an open one or go back out of a list, never a dialog"),
            row("Menu", "The menu: the row's actions first, then the screen's, then File, Edit, View, Task, Day and Help"),
            row("Up, Down, Right or OK, Left and Back in the menu", "Move, open a submenu, go back up one, and close it"),
            row("A digit in the menu", "The item of that number"),
            row("Back", "Go back"),
            row("A digit", "Open that row: 2 the second, 1 then 2 the twelfth, 0 the tenth"),
            row("Holding star", "Find rows by letter instead, typed as on a phone"),
            row("A digit while typing", "Its letters; star changes capitals and digits"),
            row("OK while typing", "Finish"),
            row("Back while typing", "Delete; held, cancel"),
        ],
    };
    let finding = match letter_press(device, "u") {
        LetterPress::Alone => Some("A capital letter, the letter with Dot 7"),
        LetterPress::Space => Some(words.braille.letter_alone.as_str()),
        LetterPress::Enter | LetterPress::Neither => None,
    };
    if let Some(finding) = finding {
        keys.push(DeviceKey::new(finding.to_owned(), words.braille.finds_a_row.clone()));
    }
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScreenAction;

    fn action(id: &str, title: &str, letter: &str) -> ScreenAction {
        ScreenAction { id: id.to_owned(), title: title.to_owned(), letter: letter.to_owned(), asks: false, shell: false, enabled: true }
    }

    fn view_with(actions: Vec<ScreenAction>) -> ViewModel {
        ViewModel { screen_actions: actions, ..ViewModel::default() }
    }

    fn commands(help: &KeyHelp, words: &ShellWords) -> Vec<String> {
        help.sections.iter().find(|section| section.heading == words.screen_actions).map(|section| section.keys.iter().map(|key| key.line.clone()).collect()).unwrap_or_default()
    }

    #[test]
    fn a_letter_is_pressed_alone_with_space_or_with_enter_as_its_device_keeps_the_others() {
        assert_eq!(letter_press(KeyDevice::BtSpeak, "c"), LetterPress::Alone);
        assert_eq!(letter_press(KeyDevice::Reader, "r"), LetterPress::Space);
        assert_eq!(letter_press(KeyDevice::Reader, "h"), LetterPress::Enter, "Space with H is the eReader's menu key");
        assert_eq!(letter_press(KeyDevice::Reader, "e"), LetterPress::Neither, "Space with E is Exit, and Enter with E the eReader's");
        assert_eq!(letter_press(KeyDevice::BrailleSense, "a"), LetterPress::Enter, "Space-1 is Up");
        assert_eq!(letter_press(KeyDevice::BrailleSense, "u"), LetterPress::Space);
        assert_eq!(letter_press(KeyDevice::OrbitSpeak, "c"), LetterPress::Enter, "the screen reader keeps Space with C");
        assert_eq!(letter_press(KeyDevice::OrbitSpeak, "d"), LetterPress::Space, "the Editor's D is the Editor's alone");
        assert_eq!(letter_press(KeyDevice::OrbitSpeak, "g"), LetterPress::Neither);
        assert_eq!(letter_press(KeyDevice::OrbitPlayer, "u"), LetterPress::Neither);
        for device in [KeyDevice::BtSpeak, KeyDevice::Reader, KeyDevice::BrailleSense, KeyDevice::OrbitSpeak] {
            assert_eq!(letter_press(device, ","), LetterPress::Neither, "{device:?}");
        }
    }

    #[test]
    fn each_device_names_a_press_as_its_own_help_names_keys() {
        assert_eq!(press_name(KeyDevice::BtSpeak, "u").as_deref(), Some("u"));
        assert_eq!(press_name(KeyDevice::Reader, "u").as_deref(), Some("Space+U"));
        assert_eq!(press_name(KeyDevice::Reader, "a").as_deref(), Some("Enter+A"));
        assert_eq!(press_name(KeyDevice::Reader, "e"), None);
        assert_eq!(press_name(KeyDevice::BrailleSense, "y").as_deref(), Some("Space-Y"));
        assert_eq!(press_name(KeyDevice::BrailleSense, "c").as_deref(), Some("Enter-C"));
        assert_eq!(press_name(KeyDevice::OrbitSpeak, "u").as_deref(), Some("space u"));
        assert_eq!(press_name(KeyDevice::OrbitSpeak, "c").as_deref(), Some("dot 8 c"));
        assert_eq!(press_instead(KeyDevice::OrbitSpeak, "c").as_deref(), Some("Press dot 8 with c."));
        assert_eq!(press_instead(KeyDevice::OrbitSpeak, "u"), None);
    }

    #[test]
    fn a_key_help_names_each_command_by_the_keys_that_press_it_on_that_device() {
        let words = shell_words(Capitals::Sentence, ThisDevice::Device);
        let view = view_with(vec![action("new-task", "New task", "a"), action("undo", "Undo", "u"), action("go-to-day", "Go to day", "g"), action("filter", "Filter", "/")]);
        assert_eq!(commands(&key_help(KeyDevice::Reader, &view, Capitals::Sentence), &words), ["Enter+A: New task", "Space+U: Undo", "Enter+G: Go to day"]);
        assert_eq!(commands(&key_help(KeyDevice::BrailleSense, &view, Capitals::Sentence), &words), ["Enter-A: New task", "Space-U: Undo", "Space-G: Go to day"]);
        assert_eq!(commands(&key_help(KeyDevice::OrbitSpeak, &view, Capitals::Sentence), &words), ["dot 8 a: New task", "space u: Undo"]);
        assert_eq!(commands(&key_help(KeyDevice::BtSpeak, &view, Capitals::Sentence), &words), ["a: New task", "u: Undo", "g: Go to day"]);
        assert!(commands(&key_help(KeyDevice::OrbitPlayer, &view, Capitals::Sentence), &words).is_empty(), "the Player presses no letter");
    }

    #[test]
    fn a_row_is_found_by_a_letter_alone_where_space_presses_commands_and_by_a_capital_where_the_letter_alone_does() {
        let words = shell_words(Capitals::Sentence, ThisDevice::Device);
        let view = view_with(Vec::new());
        let finding = |device| key_help(device, &view, Capitals::Sentence).sections[0].keys.last().map(|key| key.keys.clone());
        assert_eq!(finding(KeyDevice::Reader), Some(words.braille.letter_alone.clone()));
        assert_eq!(finding(KeyDevice::BtSpeak).as_deref(), Some("A capital letter, the letter with Dot 7"));
    }

    #[test]
    fn the_rows_actions_are_named_in_the_capitals_asked_for_each_letter_once() {
        let offered = |kind, title: &str, sentence: &str| lumenna_records::Offered {
            kind,
            subject: lumenna_records::Subject::Task,
            title: title.to_owned(),
            sentence: sentence.to_owned(),
            destructive: false,
            primary: false,
            other: None,
            asks: lumenna_records::Asks::Nothing,
        };
        let list = crate::ListView { offered: vec![offered(ActionKind::MarkDone, "Mark Done", "Mark done"), offered(ActionKind::MarkNotDone, "Mark Not Done", "Mark not done"), offered(ActionKind::MarkDone, "Mark Done", "Mark done")], ..crate::ListView::default() };
        let view = ViewModel { list: Some(list), ..ViewModel::default() };
        let rows = |capitals| key_help(KeyDevice::Reader, &view, capitals).sections.last().map(|section| section.keys.iter().map(|key| key.line.clone()).collect::<Vec<_>>()).unwrap_or_default();
        assert_eq!(rows(Capitals::Sentence), ["Space+C: Mark done, Mark not done"]);
        assert_eq!(rows(Capitals::Title), ["Space+C: Mark Done, Mark Not Done"]);
    }

    #[test]
    fn a_key_help_is_its_headings_and_keys_as_lines_in_order() {
        let words = shell_words(Capitals::Sentence, ThisDevice::Device);
        let help = key_help(KeyDevice::Reader, &view_with(vec![action("undo", "Undo", "u")]), Capitals::Sentence);
        assert_eq!(help.lines.first(), Some(&words.braille.moving));
        assert!(help.lines.contains(&words.screen_actions));
        assert_eq!(help.lines.last().map(String::as_str), Some("Space+U: Undo"));
        assert!(help.sections.iter().all(|section| section.heading != words.braille.row_actions), "no rows, no part for them");
    }
}
