//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals, a
//! completion's line, and Go to Day's question for a date picker of its own.

use boltffi::export;
use lumenna_records::places::{Place, SidebarEntry, SidebarGroup, SidebarKind};
use lumenna_app::glance::GlanceEntry;
use lumenna_records::glance::GlanceTimer;
use lumenna_records::controls::TimerControl;
use lumenna_app::Capitals;
use lumenna_app::ShellEvent;
use lumenna_app::arrangement::{Arrangement, WatchKind};
use lumenna_app::speech::Line;
use lumenna_app::dialog::{ControlMove, ControlSends};
use lumenna_app::hotkeys::HotKind;
use lumenna_app::words::{self as w, ShellWords, ThisDevice};
use lumenna_app::keys::{MenuGroup, MenuItem};
use lumenna_app::braille::{LetteredMenu, MenuShortcut};
use lumenna_app::key_help::{KeyDevice, LetterPress};
use lumenna_records::fields::SettingsPage;
use lumenna_records::words::PhoneSync;
use lumenna_records::{ActionKind, Candidate, Completions, Offered};

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

/// The letter each of a menu's items shows, empty for one with none.
#[export]
pub fn menu_shortcuts(items: Vec<MenuShortcut>) -> Vec<String> {
    lumenna_app::braille::menu_shortcuts(&items)
}

/// The menu bar on a braille device, each item with its letter: first the screen's own menu
/// (`SCREEN_MENU`, its `screen_actions`, named for the screen), then the shared menu bar. Under the
/// place shown, or over Home (`main`, `ViewModel::home` shown).
#[export]
pub fn braille_menu_bar(place: Option<Place>, main: bool, sentence: bool) -> Vec<LetteredMenu> {
    lumenna_app::braille::menu_bar(place.as_ref(), main, sentence)
}

/// The menu bar of a device whose one menu key is the bar and a row's actions both (the Orbit's
/// Space with M): the row in hand's actions first (`ROW_MENU`, named for the row, `title` its
/// line's, each item's `action` handed back with `ActOn`), then [`braille_menu_bar`]'s. A row with
/// no actions (`offered` empty) has none.
#[export]
pub fn braille_menu_bar_on_row(place: Option<Place>, main: bool, sentence: bool, title: String, offered: Vec<Offered>) -> Vec<LetteredMenu> {
    lumenna_app::braille::menu_bar_on_row(place.as_ref(), main, sentence, Some(lumenna_app::braille::RowMenu { title: &title, offered: &offered }))
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

/// How much of a running timer's planned length is left at `now` (RFC 3339), said: "12
/// minutes left", "time is up", "3 minutes over"; none without a planned length.
#[export]
pub fn glance_countdown(timer: GlanceTimer, now: String) -> Option<String> {
    let now = now.parse().unwrap_or_else(|_| jiff::Timestamp::now());
    lumenna_app::glance::countdown(&timer, now)
}

/// The timer's control as a tray's or menu bar's item reads it at `now` (RFC 3339): "Stop Timer,
/// Essay, 12 minutes left", in sentence case where `sentence`.
#[export]
pub fn timer_control_line(control: TimerControl, now: String, sentence: bool) -> String {
    let now = now.parse().unwrap_or_else(|_| jiff::Timestamp::now());
    lumenna_app::glance::control_line(&control, now, sentence)
}

/// What a timer's control shows beneath its name at `now` (RFC 3339), where what a press does is
/// its hint: the task it runs on, then what is left of the sitting's planned length, else how long
/// so far ("Essay, 12 minutes left"); while none runs, what a press would start, or why nothing
/// would. Control Center's toggle, read afresh each time it is drawn.
#[export]
pub fn timer_control_value(control: TimerControl, now: String) -> String {
    let now = now.parse().unwrap_or_else(|_| jiff::Timestamp::now());
    lumenna_app::glance::control_value(&control, now)
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

/// How `letter`'s command is pressed on `device`: alone, with Space, with Enter, or not at all.
#[export]
pub fn letter_press(device: KeyDevice, letter: String) -> LetterPress {
    lumenna_app::key_help::letter_press(device, &letter)
}

/// The keys that press `letter`'s command on `device`, as the device names keys ("Space+U" on a
/// reader, "Enter-C" on the BrailleSense, "dot 8 c" on the Speak, "u" on a BTSpeak), for a menu to
/// show beside its item; none where no key reaches the app.
#[export]
pub fn press_name(device: KeyDevice, letter: String) -> Option<String> {
    lumenna_app::key_help::press_name(device, &letter)
}

/// What `device` says when Space with `letter` reaches the app though Enter presses its command
/// there; none for a letter pressed otherwise.
#[export]
pub fn press_instead(device: KeyDevice, letter: String) -> Option<String> {
    lumenna_app::key_help::press_instead(device, &letter)
}

/// `text` with the keys that press its `letter` on `device` after it ("Today, space t"), as a
/// device that says a letter's press beside a row, an item or a button says it; `text` alone where
/// no key presses it.
#[export]
pub fn with_press(device: KeyDevice, text: String, letter: String) -> String {
    lumenna_app::key_help::with_press(device, &text, &letter)
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
/// `filters`, `reminders-due`, `add-task-in`, `new-project-inside`, `new-section`, `read-back`.
#[export]
pub fn command_letter(id: String) -> Option<String> {
    lumenna_app::braille::command_letter(&id).map(str::to_owned)
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

/// Whether `text`, entered whole, ends in a name begun and not finished, which `found` (the
/// completion at its end) would finish: a device whose line cannot complete as it is typed offers
/// `found` once the line is entered.
#[export]
pub fn ends_unfinished(text: String, found: Completions) -> bool {
    lumenna_app::typing::ends_unfinished(&text, &found)
}

/// `text` with `choice` put in over `start..end` (UTF-8 byte offsets, a completion's span).
#[export]
pub fn put_in(text: String, start: u32, end: u32, choice: String) -> String {
    lumenna_app::typing::splice(&lumenna_app::typing::Line { text, cursor: 0 }, start, end, &choice).text
}

/// What doing `how` to a dialog's control that sends `sends`, holding `value`, sends, in order
/// (`dialog::control_event`): a shell sends each, building none itself.
#[export]
pub fn control_event(sends: ControlSends, value: String, how: ControlMove) -> Vec<ShellEvent> {
    lumenna_app::dialog::control_event(&sends, &value, how)
}

/// What a key of the shared table (`id`) runs on a row offering `offered`, in short: the action
/// a screen reader names as the row's default (Enter's is `key_runs_on("open", …)`).
#[export]
pub fn key_runs_on(id: String, offered: Vec<Offered>) -> Option<Offered> {
    lumenna_app::keys::runs_on(&id, &offered)
}

/// Parts of a question or a heading as one line, each a sentence ended once: a refusal before
/// what is asked, a title before what it says.
#[export]
pub fn prompt_of(parts: Vec<String>) -> String {
    lumenna_app::screen::prompt(&parts.iter().map(String::as_str).collect::<Vec<_>>())
}

/// Several things the app said in a row, as one thing to say: each its own sentence, ended once,
/// the last keeping its full stop.
#[export]
pub fn said_together(parts: Vec<String>) -> String {
    lumenna_app::screen::said_together(&parts.iter().map(String::as_str).collect::<Vec<_>>())
}

/// A list's name with what it says of itself, as one heading, joined as `ListShown::heading` is:
/// for a list a shell names itself over a place the app names otherwise.
#[export]
pub fn list_heading(name: String, readback: String) -> String {
    lumenna_app::screen::heading(&name, &readback)
}

/// A line said with its level after it, where it has one: a project in a pick, read one line a
/// row.
#[export]
pub fn line_with_level(line: Line) -> String {
    lumenna_app::lines::with_level(&line)
}

/// A row's line said whole with its folding and its level after it, where it has them
/// (`expanded`: none for a row that does not fold), for a list that reads one line a row.
#[export]
pub fn line_with_state(line: Line, expanded: Option<bool>) -> String {
    lumenna_app::lines::with_state(&line, expanded)
}

/// [`line_with_state`] less the line's first part, for a list that shows a row's title apart.
#[export]
pub fn detail_with_state(line: Line, expanded: Option<bool>) -> String {
    lumenna_app::lines::detail_with_state(&line, expanded)
}

/// The settings page a command of the menu bar opens (`devices`, `export-import`,
/// `restore-backup`), by its id; none for any other.
#[export]
pub fn settings_page_of(command: String) -> Option<String> {
    lumenna_app::keys::settings_page_of(&command).map(str::to_owned)
}

/// Where a phone and a watch put the places, from the sidebar as the view gives it: a phone's
/// tabs, its Browse tab's rows and each group's screen, and a watch's root list (from Home, as a
/// watch of `watch`'s kind has it; a phone passes either).
#[export]
pub fn place_arrangement(sidebar: Vec<SidebarEntry>, watch: WatchKind, capitals: Capitals) -> Arrangement {
    lumenna_app::arrangement::arrangement(&sidebar, watch, capitals == Capitals::Sentence)
}
