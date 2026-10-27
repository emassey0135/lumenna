//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals, a
//! completion's line, and Go to Day's question for a date picker of its own.

use boltffi::export;
use lumenna_records::places::{Place, SidebarGroup, SidebarKind};
use lumenna_app::glance::GlanceEntry;
use lumenna_app::hotkeys::{HotKeyWords, HotKind};
use lumenna_app::keys::{MenuGroup, MenuItem};
use lumenna_records::fields::{DetailsWords, NotificationWords, QuickAddWords, SettingWords, SettingsPage};
use lumenna_records::fields::{FileWords, ReadBackWords};
use lumenna_records::words::{AppMetadata, PhoneSync};
use lumenna_records::{Candidate, Choice, PickerWords};

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

/// What a shell's own day or time picker says, beside the line it fills.
#[export]
pub fn picker_words() -> PickerWords {
    lumenna_records::picker_words()
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

/// What a setting's own control says: a time's hint and example, a number's.
#[export]
pub fn setting_words() -> SettingWords {
    lumenna_records::fields::setting_words()
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

/// What the task details say around their form: every label and button.
#[export]
pub fn details_words(sentence: bool) -> DetailsWords {
    lumenna_records::fields::details_words(sentence)
}

/// What quick add and a list's filter say.
#[export]
pub fn quick_add_words(sentence: bool) -> QuickAddWords {
    lumenna_records::fields::quick_add_words(sentence)
}

/// A completion's button or menu item: "Complete with Work, project".
#[export]
pub fn complete_with(candidate: Candidate) -> String {
    lumenna_records::fields::complete_with(&lumenna_app::speech::candidate(&candidate))
}

/// What a device says of its notifications, `this_device` being how it calls itself.
#[export]
pub fn notification_words(this_device: String, sentence: bool) -> NotificationWords {
    lumenna_records::fields::notification_words(&this_device, sentence)
}

/// What Lumenna is, in one line: About, a store's listing, a desktop entry.
#[export]
pub fn app_description() -> String {
    lumenna_records::words::app_description()
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

/// What a list a shell offers of its own says with nothing in it to choose.
#[export]
pub fn nothing_to_choose() -> String {
    lumenna_records::words::nothing_to_choose()
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

/// How long, in words: "1 hour 30 minutes".
#[export]
pub fn duration_words(minutes: u32) -> String {
    lumenna_records::words::duration(minutes)
}

/// What Settings says of the shortcuts from anywhere.
#[export]
pub fn hotkey_words() -> HotKeyWords {
    lumenna_app::hotkeys::hotkey_words()
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

/// What Lumenna is: its name, description, version, home, issues, publisher, licence and
/// identifier, from the workspace's one source.
#[export]
pub fn app_metadata() -> AppMetadata {
    lumenna_records::words::metadata()
}

/// What a shell asks when a command could run more than one of the row's actions of a kind.
#[export]
pub fn which() -> String {
    lumenna_records::words::which()
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

/// The list of reminders due, as its heading or screen names it.
#[export]
pub fn reminders_due(sentence: bool) -> String {
    lumenna_records::words::reminders_due(sentence)
}

/// The day's own actions, as a menu button names them.
#[export]
pub fn day_actions(sentence: bool) -> String {
    lumenna_records::words::day_actions(sentence)
}

/// A place's own actions, as a menu button names them.
#[export]
pub fn place_actions(sentence: bool) -> String {
    lumenna_records::words::place_actions(sentence)
}

/// What the devices list says Delete does there.
#[export]
pub fn delete_unpairs() -> String {
    lumenna_records::words::delete_unpairs()
}

/// The field naming this device for pairing, `this_device` how it calls itself.
#[export]
pub fn device_name_label(this_device: String) -> String {
    lumenna_records::words::device_name_label(&this_device)
}

/// What the field naming this device is for.
#[export]
pub fn device_name_hint() -> String {
    lumenna_records::words::device_name_hint()
}

/// What is said when a pairing code could not be copied.
#[export]
pub fn copy_failed() -> String {
    lumenna_records::words::copy_failed()
}

/// What a pick says when answered with nothing chosen.
#[export]
pub fn choose_one_first() -> String {
    lumenna_records::words::choose_one_first()
}

/// A dialog's Cancel.
#[export]
pub fn cancel(sentence: bool) -> String {
    lumenna_records::words::cancel(sentence)
}

/// A dialog's Close.
#[export]
pub fn close(sentence: bool) -> String {
    lumenna_records::words::close(sentence)
}

/// What a line that completes says of it, after its own hint.
#[export]
pub fn completion_hint() -> String {
    lumenna_records::words::completion_hint()
}

/// What is said when the shortcuts from anywhere cannot be had here, and the platform's why.
#[export]
pub fn hotkeys_unavailable(reason: String) -> String {
    lumenna_records::words::hotkeys_unavailable(&reason)
}

/// The tab or screen of every project, label and saved filter.
#[export]
pub fn browse(sentence: bool) -> String {
    lumenna_records::words::browse(sentence)
}

/// A field's placeholder showing the shape of an answer: "For example, tomorrow".
#[export]
pub fn for_example(example: String) -> String {
    lumenna_records::words::for_example(&example)
}

/// What an empty field says of itself where its value is shown apart from it.
#[export]
pub fn empty_field() -> String {
    lumenna_records::words::empty_field()
}

/// An empty value, said in a line: "none".
#[export]
pub fn none_value() -> String {
    lumenna_records::words::none_value()
}

/// What a place is, as the heading over its own actions says it; empty for the app's own places.
#[export]
pub fn place_heading(place: Place, sentence: bool) -> String {
    lumenna_records::words::place_heading(&place, sentence).unwrap_or_default()
}

/// A button that asks for more of a line to put at its end.
#[export]
pub fn add_to_line(sentence: bool) -> String {
    lumenna_records::words::add_to_line(sentence)
}

/// The list of what could come next, as its dialog or menu is called.
#[export]
pub fn completions(sentence: bool) -> String {
    lumenna_records::words::completions(sentence)
}

/// How many completions a shell lists at once.
#[export]
pub fn completions_shown() -> u32 {
    u32::try_from(lumenna_records::words::COMPLETIONS_SHOWN).unwrap_or(u32::MAX)
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

/// What an iPad's settings say with no page open.
#[export]
pub fn no_settings_page() -> String {
    lumenna_records::words::no_settings_page()
}

/// A field that is on or off, as a choice of two.
#[export]
pub fn yes_no() -> Vec<Choice> {
    lumenna_records::fields::yes_no()
}

/// What a shell's option of reading a line back before adding it says.
#[export]
pub fn read_back_words(sentence: bool) -> ReadBackWords {
    lumenna_records::fields::read_back_words(sentence)
}

/// The question a line read back asks: "… Add it?"
#[export]
pub fn add_it(readback: String) -> String {
    lumenna_records::fields::add_it(&readback)
}

/// What a shell's own file and folder dialogs ask, and the export's button.
#[export]
pub fn file_words(sentence: bool) -> FileWords {
    lumenna_records::fields::file_words(sentence)
}

/// Whether reminders are given on this device, said once changed.
#[export]
pub fn reminders_on_here(on: bool, this_device: String) -> String {
    lumenna_records::words::reminders_on_here(on, &this_device)
}
