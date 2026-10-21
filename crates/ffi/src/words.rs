//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals, a
//! completion's line, and Go to Day's question for a date picker of its own.

use boltffi::export;
use lumenna_records::places::{Place, SidebarGroup, SidebarKind};
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

