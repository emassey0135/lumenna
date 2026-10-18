//! What is being typed: completion and the quick-add preview, keystroke-rate questions of the
//! snapshot.

use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use lumenna_parse::complete::{self, complete};
use lumenna_parse::quickadd::{Known, parse_quick_add};

use crate::types::{Completions, Preview, Syntax};

/// What could be inserted at `cursor`, a UTF-8 byte offset into `text`.
#[must_use]
pub fn completions(snapshot: &Snapshot, text: &str, cursor: u32, syntax: Syntax) -> Completions {
    let mut cursor = (cursor as usize).min(text.len());
    // A cursor inside a character would split it; step back to where it starts.
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let syntax = match syntax {
        Syntax::QuickAdd => complete::Syntax::QuickAdd,
        Syntax::Filter => complete::Syntax::Filter,
    };
    Completions::of(&complete(text, cursor, syntax, &Known::from_snapshot(snapshot)))
}

/// What a quick-add line would produce, without producing it.
#[must_use]
pub fn preview(snapshot: &Snapshot, text: &str, now: &Zoned) -> Preview {
    let parsed = parse_quick_add(text, &Known::from_snapshot(snapshot));
    Preview::of(&parsed.resolve(snapshot, now), snapshot)
}
