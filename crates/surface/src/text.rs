//! What is being typed: completion and the quick-add preview.
//!
//! Both are keystroke-rate questions, which is why they are methods on a linked object and
//! never a process per keystroke.

use jiff::Zoned;
use lumenna_parse::complete::{self, complete};
use lumenna_parse::quickadd::{Known, parse_quick_add};

use crate::error::Result;
use crate::types::{Completions, Preview, Syntax};
use crate::{Lumenna, repaired};

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// What could be inserted at `cursor`, a UTF-8 byte offset into `text`.
    ///
    /// The announcement is the count, to be said before the list; the candidates are
    /// components, for a client to present in its own medium.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn complete_text(&self, text: &str, cursor: u32, syntax: Syntax) -> Result<Completions> {
        let mut cursor = (cursor as usize).min(text.len());
        // A cursor inside a character would split it; step back to where it starts.
        while !text.is_char_boundary(cursor) {
            cursor -= 1;
        }
        let syntax = match syntax {
            Syntax::QuickAdd => complete::Syntax::QuickAdd,
            Syntax::Filter => complete::Syntax::Filter,
        };
        self.with(|store| {
            let snapshot = repaired(store);
            Ok(Completions::of(&complete(text, cursor, syntax, &Known::from_snapshot(&snapshot))))
        })
    }

    /// What a quick-add line would produce, without producing it.
    ///
    /// The announcement is the readback a sighted user gets as inline highlighting.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn preview_task(&self, text: &str) -> Result<Preview> {
        let now = Zoned::now();
        self.with(|store| {
            let snapshot = repaired(store);
            let parsed = parse_quick_add(text, &Known::from_snapshot(&snapshot));
            Ok(Preview::of(&parsed.resolve(&snapshot, &now), &snapshot))
        })
    }
}
