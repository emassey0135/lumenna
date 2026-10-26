//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals, a
//! completion's line, and Go to Day's question for a date picker of its own.

use boltffi::export;
use lumenna_records::places::{Place, SidebarGroup, SidebarKind};
use lumenna_app::glance::GlanceEntry;
use lumenna_app::hotkeys::{HotKeyWords, HotKind};
use lumenna_app::keys::{MenuGroup, MenuItem};
use lumenna_records::fields::{DetailsWords, NotificationWords, QuickAddWords, SettingWords, SettingsPage};
use lumenna_records::words::PhoneSync;
use lumenna_records::{Candidate, PickerWords, Question};

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

/// The question Go to Day asks, for a shell asking for a day itself: its own date picker.
#[export]
pub fn go_to_day_question() -> Question {
    lumenna_records::go_to_day_question()
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
