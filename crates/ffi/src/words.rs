//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals, a
//! completion's line, and Go to Day's question for a date picker of its own.

use boltffi::export;
use lumenna_records::places::{Place, SidebarGroup, SidebarKind};
use lumenna_app::glance::GlanceEntry;
use lumenna_app::Capitals;
use lumenna_app::hotkeys::HotKind;
use lumenna_app::words::{self as w, ShellWords, ThisDevice};
use lumenna_app::keys::{MenuGroup, MenuItem};
use lumenna_app::braille::{LetterCommand, SpeakLetter};
use lumenna_records::fields::SettingsPage;
use lumenna_records::words::PhoneSync;
use lumenna_records::{ActionKind, Candidate};

/// `text` in sentence case: for a platform whose convention it is, and for braille.
#[export]
pub fn sentence_case(text: String) -> String {
    lumenna_records::fields::sentence_case(text)
}

/// A completion as offered, name first: "Work, project", not the core's "project Work".
#[export]
pub fn candidate_line(candidate: Candidate) -> String {
    lumenna_app::speech::candidate(&candidate)
}

/// What identifies a sidebar row of this kind, as the app names its row in hand.
#[export]
pub fn sidebar_key(kind: SidebarKind) -> String {
    lumenna_records::places::SidebarEntry::key_of(&kind)
}

/// How many places a sidebar group holds, in words: "no projects", "1 label".
#[export]
pub fn group_count(group: SidebarGroup, count: u32) -> String {
    group.count(count as usize)
}

/// What a sidebar group's own list says with nothing in it: "No labels.".
#[export]
pub fn group_empty(group: SidebarGroup) -> String {
    group.empty()
}

/// What a place is called before the app has shown it.
#[export]
pub fn place_title(place: Place) -> String {
    place.title()
}


/// The menu bar every desktop app shows (`lumenna_app::keys::MENUS`), worded, in sentence case
/// where `sentence`: each menu's items by command, a separator an item with no `id`.
#[export]
pub fn menus(sentence: bool) -> Vec<MenuGroup> {
    lumenna_app::keys::menus(sentence)
}

/// The menu of Lumenna's icon in the tray, the status bar or the menu bar, worded.
#[export]
pub fn tray_menu(sentence: bool) -> Vec<MenuItem> {
    lumenna_app::keys::tray(sentence)
}

/// Every settings page in order, each setting on one (`SettingsView::pages` has the same): a
/// shell that follows its platform's clock passes `own_clock` and is not offered the clock.
#[export]
pub fn settings_pages(own_clock: bool, sentence: bool) -> Vec<SettingsPage> {
    lumenna_records::fields::settings_pages(own_clock, sentence)
}

/// What is said once a backup is taken into a folder of the person's.
#[export]
pub fn backed_up_to(folder: String, name: String) -> String {
    lumenna_records::fields::backed_up_to(&folder, &name)
}

/// What an app says when its automatic backup failed, its reason after.
#[export]
pub fn backup_failed(reason: String) -> String {
    lumenna_records::words::backup_failed(&reason)
}

/// A completion's button or menu item: "Complete with Work, project".
#[export]
pub fn complete_with(candidate: Candidate) -> String {
    lumenna_records::fields::complete_with(&lumenna_app::speech::candidate(&candidate))
}

/// What a browser calls itself when it pairs: "Firefox on Windows".
#[export]
pub fn browser_device_name(browser: String, system: String) -> String {
    lumenna_records::words::browser_device_name(&browser, &system)
}

/// A watch's sync with its phone, said, `phone` being what the phone is called ("the iPhone").
#[export]
pub fn phone_sync(phone: String, status: PhoneSync) -> String {
    lumenna_records::words::phone_sync(&phone, &status)
}

/// How many a pick offers: "1 choice", "3 choices".
#[export]
pub fn choice_count(count: u32) -> String {
    lumenna_records::words::choice_count(count as usize)
}

/// A row with rows under it, as its state is said where the platform's control does not say it.
#[export]
pub fn fold_state(expanded: bool) -> String {
    lumenna_records::words::fold_state(expanded)
}

/// How deep a row sits, as said where it changes from the row before: "level 2" for `depth` 1.
/// For a shell that lists part of a tree on its own (BTSpeak's projects, labels and filters, one
/// heading's places to a screen), whose levels count from that part.
#[export]
pub fn level_words(depth: u32) -> String {
    lumenna_records::words::level(depth)
}

/// The BrailleSense's keys, for its key help.
#[export]
pub fn braillesense_keys() -> Vec<w::DeviceKey> {
    w::braillesense_keys()
}

/// How long, in words: "1 hour 30 minutes".
#[export]
pub fn duration_words(minutes: u32) -> String {
    lumenna_records::words::duration(minutes)
}

/// A shortcut from anywhere's name, in the shell's capitals.
#[export]
pub fn hotkey_name(kind: HotKind, sentence: bool) -> String {
    kind.name(sentence)
}

/// The letter a shortcut from anywhere takes until the person changes it, with the platform's
/// own modifiers (Control-Command on the Mac).
#[export]
pub fn hotkey_letter(kind: HotKind) -> String {
    kind.letter().to_string()
}

/// The entry of a glance timeline to show at `now` (RFC 3339): the last whose moment has come,
/// else the first.
#[export]
pub fn glance_current(entries: Vec<GlanceEntry>, now: String) -> Option<GlanceEntry> {
    let now = now.parse().unwrap_or_else(|_| jiff::Timestamp::now());
    lumenna_app::glance::current(&entries, now).cloned()
}

/// When a widget showing `entries` at `now` is next to be drawn again (RFC 3339): the next
/// moment, or a running timer's next minute. None when nothing more changes.
#[export]
pub fn glance_redraw_at(entries: Vec<GlanceEntry>, now: String) -> Option<String> {
    let now = now.parse().unwrap_or_else(|_| jiff::Timestamp::now());
    lumenna_app::glance::redraw_at(&entries, now).map(|at| at.to_string())
}

/// How long a timer counting from `counting_from` has run at `now`, said.
#[export]
pub fn glance_logged(counting_from: String, now: String) -> Option<String> {
    let now = now.parse().unwrap_or_else(|_| jiff::Timestamp::now());
    lumenna_app::glance::logged(&counting_from, now)
}

/// Where the level changes among rows shown at `depths`, in order: "level 2", or empty where it
/// does not. For a list a shell narrows or cuts out itself, so no shell counts levels.
#[export]
pub fn levels(depths: Vec<u32>) -> Vec<String> {
    lumenna_app::lines::levels(&depths).into_iter().map(Option::unwrap_or_default).collect()
}

/// What a shell says of an action whose form it has not drawn yet, `action` its name.
#[export]
pub fn no_form_here(action: String) -> String {
    lumenna_records::words::no_form_here(&action)
}

/// What a shell says of a question it cannot ask yet, `action` its name.
#[export]
pub fn cannot_ask_here(action: String) -> String {
    lumenna_records::words::cannot_ask_here(&action)
}

/// A shell's own menu of what can be done where the person is: "Day menu".
#[export]
pub fn menu_of(name: String) -> String {
    lumenna_records::words::menu_of(&name)
}

/// What an empty list says of the key that adds to it, `key` the platform's: "Press a to add one."
#[export]
pub fn press_to_add(key: String) -> String {
    lumenna_records::words::press_to_add(&key)
}

/// What an empty list that can be filtered says of its keys.
#[export]
pub fn press_to_add_or_filter(key: String, filter: String) -> String {
    lumenna_records::words::press_to_add_or_filter(&key, &filter)
}

/// What an empty list of devices says of the key that pairs one.
#[export]
pub fn press_to_pair(key: String) -> String {
    lumenna_records::words::press_to_pair(&key)
}

/// The field naming this device for pairing, `this_device` how it calls itself.
#[export]
pub fn device_name_label(this_device: String) -> String {
    lumenna_records::words::device_name_label(&this_device)
}

/// What is said when the shortcuts from anywhere cannot be had here, and the platform's why.
#[export]
pub fn hotkeys_unavailable(reason: String) -> String {
    lumenna_records::words::hotkeys_unavailable(&reason)
}

/// A field's placeholder showing the shape of an answer: "For example, tomorrow".
#[export]
pub fn for_example(example: String) -> String {
    lumenna_records::words::for_example(&example)
}

/// What a place is, as the heading over its own actions says it; empty for the app's own places.
#[export]
pub fn place_heading(place: Place, sentence: bool) -> String {
    lumenna_records::words::place_heading(&place, sentence).unwrap_or_default()
}

/// A widget's button marking `task` done, as a screen reader names it.
#[export]
pub fn mark_done_described(mark_done: String, task: String) -> String {
    lumenna_records::words::mark_done_described(&mark_done, &task)
}

/// A timer control, as a screen reader names it; a locked device leaves out what runs.
#[export]
pub fn timer_described(name: String, action: String, detail: String, locked: bool) -> String {
    lumenna_records::words::timer_described(&name, &action, &detail, locked)
}

/// The question a line read back asks: "… Add it?"
#[export]
pub fn add_it(readback: String) -> String {
    lumenna_records::fields::add_it(&readback)
}

/// Whether reminders are given on this device, said once changed.
#[export]
pub fn reminders_on_here(on: bool, this_device: String) -> String {
    lumenna_records::words::reminders_on_here(on, &this_device)
}

/// Every fixed word a shell draws, in its `capitals`, for a device that is `device`: asked for
/// once, as it starts.
#[export]
pub fn shell_words(capitals: Capitals, device: ThisDevice) -> ShellWords {
    w::shell_words(capitals, device)
}

/// What another device calls this one: `host` without `.local`, else what it is.
#[export]
pub fn device_named(host: String, device: ThisDevice) -> String {
    w::device_named(&host, device)
}

/// What answers a client through an app: "Lumenna for Windows".
#[export]
pub fn lumenna_for(system: String) -> String {
    w::lumenna_for(&system)
}

/// Where About says the store is.
#[export]
pub fn data_folder(path: String) -> String {
    w::data_folder(&path)
}

/// A file that could not be read in, the error's sentence after it.
#[export]
pub fn could_not_read(what: String, reason: String) -> String {
    w::could_not_read(&what, &reason)
}

/// A file that could not be written, the error's sentence after it.
#[export]
pub fn could_not_save(what: String, reason: String) -> String {
    w::could_not_save(&what, &reason)
}

/// When Lumenna could not start, the error's sentence after it.
#[export]
pub fn could_not_start(reason: String) -> String {
    w::could_not_start(&reason)
}

/// What is asked before a file is replaced.
#[export]
pub fn already_there(name: String) -> String {
    w::already_there(&name)
}

/// What is said once a file is written.
#[export]
pub fn saved(what: String) -> String {
    w::saved(&what)
}

/// A file dialog's kind for an extension: "JSON files".
#[export]
pub fn files_of(extension: String) -> String {
    w::files_of(&extension)
}

/// What narrows the pick called `title`.
#[export]
pub fn narrow(title: String) -> String {
    w::narrow(&title)
}

/// What syncs a watch with its phone now.
#[export]
pub fn sync_with(phone: String) -> String {
    w::sync_with(&phone)
}

/// A device list's description where `key` renames.
#[export]
pub fn delete_unpairs_or_renames(key: String) -> String {
    w::delete_unpairs_or_renames(&key)
}

/// Why the shortcuts from anywhere cannot be had: `what` did not answer.
#[export]
pub fn did_not_answer(what: String, reason: String) -> String {
    w::did_not_answer(&what, &reason)
}

/// Why `system` would not start Lumenna at sign-in, where it gives no reason.
#[export]
pub fn did_not_turn_it_on(system: String) -> String {
    w::did_not_turn_it_on(&system)
}

/// The warning that `keys` are someone else's.
#[export]
pub fn keys_taken(keys: String) -> String {
    w::keys_taken(&keys)
}

/// Its button that keeps the keys as they were.
#[export]
pub fn keep_keys(keys: String) -> String {
    w::keep_keys(&keys)
}

/// Its question, after whose they are.
#[export]
pub fn use_anyway_for(whose: String, name: String) -> String {
    w::use_anyway_for(&whose, &name)
}

/// What is said once the shortcut called `name` is changed, to `described` or off.
#[export]
pub fn shortcut_changed(name: String, described: Option<String>) -> String {
    lumenna_app::hotkeys::changed(&name, described.as_deref())
}

/// What the dialog for new keys asks, for the shortcut called `name`.
#[export]
pub fn press_keys_for(name: String) -> String {
    lumenna_app::hotkeys::press_keys_for(&name)
}

/// The button that changes the shortcut called `name`, as a screen reader names it.
#[export]
pub fn change_shortcut_for(name: String) -> String {
    lumenna_app::hotkeys::change_shortcut_for(&name)
}

/// What is asked before keys warned about are kept.
#[export]
pub fn use_keys(described: String) -> String {
    lumenna_app::hotkeys::use_keys(&described)
}

/// What the Mac asks while it waits for new keys, `keeps` the keys Escape leaves in place.
#[export]
pub fn press_mac_keys(keeps: String) -> String {
    w::press_mac_keys(&keeps)
}

/// A block's length, as a stepper that changes it says it: "Lasts 1 hour 30 minutes".
#[export]
pub fn lasts(minutes: u32) -> String {
    w::lasts(minutes)
}

/// The core's own letter for a chord of dots (dot 1 = bit 0 … dot 8 = bit 7), a to z. A chord with
/// dot 7 or dot 8 is no letter: those dots are a device's own keys. Never the person's table.
#[export]
pub fn braille_letter(dots: u8) -> Option<String> {
    lumenna_app::braille::braille_letter(dots).map(|letter| letter.to_string())
}

/// How `letter` is pressed on the Orbit Speak: Space with it, Dot 8 with it, or neither.
#[export]
pub fn speak_letter(letter: String) -> SpeakLetter {
    lumenna_app::braille::speak_letter(&letter)
}

/// How a letter's press is said after what it runs on the Orbit Speak, none for one with no press.
#[export]
pub fn speak_press(letter: String) -> Option<String> {
    lumenna_app::braille::speak_press(&letter)
}

/// Space with a letter that takes Dot 8 on the Orbit Speak, refused: which to press.
#[export]
pub fn speak_press_instead(letter: String) -> String {
    lumenna_app::braille::speak_press_instead(&letter)
}

/// Where the next line starting with `letter` is, from the line after `from` on, wrapping round
/// and ending back at `from`; a line `hidden` says is folded away is passed. For a list a shell
/// draws itself.
#[export]
pub fn first_letter(lines: Vec<String>, hidden: Vec<bool>, from: u32, letter: String) -> Option<u32> {
    lumenna_app::braille::first_letter(&lines, &hidden, from as usize, &letter).map(|at| u32::try_from(at).unwrap_or(u32::MAX))
}

/// A kind of action's letter on a braille device, none for one no letter runs.
#[export]
pub fn action_letter(kind: ActionKind) -> Option<String> {
    lumenna_app::braille::action_letter(kind).map(str::to_owned)
}

/// A settings page's letter on a braille device, by `SettingsPage::id`.
#[export]
pub fn page_letter(id: String) -> Option<String> {
    lumenna_app::braille::page_letter(&id).map(str::to_owned)
}

/// A settings page's button's letter on a braille device, by `SettingsButton::id`.
#[export]
pub fn button_letter(id: String) -> Option<String> {
    lumenna_app::braille::button_letter(&id).map(str::to_owned)
}

/// A reminder's button's letter on a braille device, by `DueButton::id`.
#[export]
pub fn due_letter(id: String) -> Option<String> {
    lumenna_app::braille::due_letter(&id).map(str::to_owned)
}

/// A screen's own command's letter, by the shared table's `id` or `projects`, `labels`,
/// `filters`, `reminders-due`, `add-task-in`, `new-project-inside`, `read-back`.
#[export]
pub fn command_letter(id: String) -> Option<String> {
    lumenna_app::braille::command_letter(&id).map(str::to_owned)
}

/// What `place` itself offers on a braille device besides its rows, each with its letter.
#[export]
pub fn place_commands(place: Place, sentence: bool) -> Vec<LetterCommand> {
    lumenna_app::braille::place_commands(&place, sentence)
}

/// A braille device's main menu: the places, then adding, undo and redo, reminders, syncing and
/// settings, each with its letter.
#[export]
pub fn braille_main_menu(sentence: bool) -> Vec<LetterCommand> {
    lumenna_app::braille::main_menu(sentence)
}

/// What is said when no row's line starts with the letter pressed.
#[export]
pub fn no_row_starts_with(letter: String) -> String {
    lumenna_records::words::no_row_starts_with(&letter)
}

/// What is said of a letter that runs nothing on this screen.
#[export]
pub fn nothing_on_letter(letter: String) -> String {
    lumenna_records::words::nothing_on_letter(&letter)
}
