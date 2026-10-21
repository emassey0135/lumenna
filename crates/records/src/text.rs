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

/// What a quick-add line would produce, without producing it: a task, or, for a line that
/// starts with `block`, a block (`lumenna_parse::block`).
#[must_use]
pub fn preview(snapshot: &Snapshot, text: &str, now: &Zoned) -> Preview {
    if let Some(block) = lumenna_parse::block::parse_block_line(text) {
        return block_preview(snapshot, block, now);
    }
    let parsed = parse_quick_add(text, &Known::from_snapshot(snapshot));
    Preview::of(&parsed.resolve(snapshot, now), snapshot)
}

/// A block line read back: "New block Deep work, 9:00 AM to 11:00 AM, every weekday", or why
/// it cannot be added.
fn block_preview(snapshot: &Snapshot, block: Result<lumenna_parse::block::BlockLine, String>, now: &Zoned) -> Preview {
    let refused = |message: String| Preview {
        announcement: message.clone(),
        has_errors: true,
        diagnostics: vec![crate::types::Diagnostic { severity: "error".to_owned(), start: 0, end: 0, message }],
        ..Preview::default()
    };
    let block = match block {
        Ok(block) => block,
        Err(message) => return refused(message),
    };
    let day = match crate::resolve::date(block.date.as_deref(), now, snapshot.settings.week_start) {
        Ok(day) => day,
        Err(error) => return refused(error.message().to_owned()),
    };
    let end = block.start.checked_add(jiff::SignedDuration::from_mins(i64::from(block.minutes))).unwrap_or(block.start);
    let clock = lumenna_core::time::clock_words;
    let mut parts = vec![
        format!("New block {}", block.title),
        format!("{} to {}", clock(block.start), clock(end)),
    ];
    if block.date.is_some() {
        parts.push(lumenna_core::time::day_words(day, now.date()));
    }
    if let Some(repeat) = &block.repeat {
        parts.push(repeat.clone());
    }
    Preview {
        announcement: parts.join(", "),
        title: block.title,
        due: Some(day.to_string()),
        due_time: Some(crate::words::time_text(block.start)),
        due_phrase: block.date,
        repetition: block.repeat,
        priority: 4,
        ..Preview::default()
    }
}
