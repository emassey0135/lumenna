//! A quick-add line being typed: what it will add, and what fits where the cursor is.

use crux_core::Command;
use crux_core::render::render;
use lumenna_records::types::{Completions, Preview};
use serde::{Deserialize, Serialize};

use crate::model::Screen;
use crate::{Effect, Event, Lumenna};

/// What the person does in quick add.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Typing {
    /// Quick add opens, the line starting with what the place adds.
    Open,
    /// The line as it now is, and the cursor in it as a UTF-8 byte offset.
    Text {
        /// The line.
        text: String,
        /// Where the cursor is, in UTF-8 bytes.
        cursor: u32,
    },
    /// Put in the completion offered at this position in the list.
    Choose(u32),
    /// Quick add closes.
    Close,
}

/// A line being typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Line {
    /// The line.
    pub text: String,
    /// The cursor, in UTF-8 bytes.
    pub cursor: u32,
}

/// The line as shown, with what it will add and what fits at the cursor.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineView {
    /// The line.
    pub text: String,
    /// The cursor, in UTF-8 bytes.
    pub cursor: u32,
    /// What it will add, read back, each diagnostic after: empty for an empty line.
    pub readback: String,
    /// Whether it adds nothing as it stands.
    pub has_errors: bool,
    /// What fits at the cursor, and the span (UTF-8 bytes) it replaces.
    pub offer: Completions,
}

/// The readback of `preview`: its sentence, then each thing wrong with the line.
#[must_use]
pub fn readback(preview: &Preview) -> String {
    std::iter::once(preview.announcement.clone())
        .chain(preview.diagnostics.iter().map(|d| d.message.clone()))
        .collect::<Vec<_>>()
        .join(". ")
}

/// `line` with the completion `text` put in over `start..end`, the cursor after it.
#[must_use]
pub fn splice(line: &Line, start: u32, end: u32, text: &str) -> Line {
    let (start, end) = (start as usize, end as usize);
    let (start, end) = (start.min(line.text.len()), end.min(line.text.len()));
    let mut out = String::with_capacity(line.text.len() + text.len());
    out.push_str(&line.text[..start]);
    out.push_str(text);
    out.push_str(&line.text[end..]);
    let cursor = u32::try_from(start + text.len()).unwrap_or(u32::MAX);
    Line { text: out, cursor }
}

impl Lumenna {
    pub(crate) fn typing(&self, event: Typing, model: &mut Screen) -> Command<Effect, Event> {
        match event {
            Typing::Open => {
                let text = model.place().quick_add_prefix();
                let cursor = u32::try_from(text.len()).unwrap_or(u32::MAX);
                model.line = Some(Line { text, cursor });
                model.add_refused = None;
            }
            Typing::Text { text, cursor } => model.line = Some(Line { text, cursor }),
            Typing::Choose(index) => {
                let (Some(line), Some(store)) = (&model.line, &model.store) else { return Command::done() };
                let offer = lumenna_records::text::completions(
                    &store.snapshot,
                    &line.text,
                    line.cursor,
                    lumenna_records::types::Syntax::QuickAdd,
                );
                let Some(candidate) = offer.candidates.get(index as usize) else { return Command::done() };
                model.line = Some(splice(line, offer.start, offer.end, &candidate.text));
            }
            Typing::Close => model.line = None,
        }
        render()
    }
}
