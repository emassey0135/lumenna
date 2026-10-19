//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals,
//! and why a key does nothing on a row.

use boltffi::export;
use lumenna_app::Capitals;
use lumenna_records::places::{Place, SidebarGroup, SidebarKind};
use lumenna_records::{ActionKind, Candidate, PairingWords, Question, Subject};

/// `text` in sentence case: for a platform whose convention it is, and for braille.
#[export]
pub fn sentence_case(text: String) -> String {
    lumenna_records::fields::sentence_case(text)
}

/// A completion as offered, name first: "Work, project", not the core's "project Work".
#[export]
pub fn candidate_line(candidate: Candidate) -> String {
    lumenna_desktop::speech::candidate(&candidate)
}

/// What identifies a sidebar row of this kind, as the app names its row in hand.
#[export]
pub fn sidebar_key(kind: SidebarKind) -> String {
    lumenna_records::places::SidebarEntry::key_of(&kind)
}

/// The question Go to Day asks, for a shell asking for a day itself: another day's blocks in
/// a pick ([`lumenna_app::Event::PickFrom`]).
#[export]
pub fn go_to_day_question() -> Question {
    lumenna_records::go_to_day_question()
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

/// Why a key that means `kind` does nothing on a row that is a `subject`.
#[export]
pub fn not_offered(kind: ActionKind, subject: Subject, this_device: bool) -> String {
    lumenna_records::not_offered(kind, subject, this_device)
}

/// Every sentence and button of pairing, `this_device` being what this device is called, its
/// buttons and headings in `capitals`.
#[export]
pub fn pairing_words(this_device: String, local: bool, capitals: Capitals) -> PairingWords {
    let mut words = lumenna_records::pairing_words(this_device, local);
    if capitals == Capitals::Sentence {
        for word in [
            &mut words.title,
            &mut words.wait,
            &mut words.join,
            &mut words.match_title,
            &mut words.match_yes,
            &mut words.match_no,
            &mut words.copy_code,
        ] {
            *word = lumenna_records::fields::sentence_case(std::mem::take(word));
        }
    }
    words
}
