//! What is being typed: completion and the quick-add preview.
//!
//! Both are keystroke-rate questions, which is why they are methods on a linked object and
//! never a process per keystroke.

use jiff::Zoned;

use crate::error::Result;
use crate::types::{Completions, Preview, Syntax};
use crate::{Lumenna, repaired};

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
        self.told(|store| Ok(lumenna_records::text::completions(&repaired(store), text, cursor, syntax)))
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
        self.told(|store| Ok(lumenna_records::text::preview(&repaired(store), text, &now)))
    }
}
