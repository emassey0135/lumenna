//! The core's words a shell asks for itself, outside a turn: a fixed button in its capitals,
//! and why a key does nothing on a row.

use boltffi::export;
use lumenna_records::{ActionKind, Subject};

/// `text` in sentence case: for a platform whose convention it is, and for braille.
#[export]
pub fn sentence_case(text: String) -> String {
    lumenna_records::fields::sentence_case(text)
}

/// Why a key that means `kind` does nothing on a row that is a `subject`.
#[export]
pub fn not_offered(kind: ActionKind, subject: Subject, this_device: bool) -> String {
    lumenna_records::not_offered(kind, subject, this_device)
}
