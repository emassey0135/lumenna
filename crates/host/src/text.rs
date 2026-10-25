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

/// What is being typed, completed at its end by [`Lumenna::complete_typed`]: for a client
/// completing one argument of its own, such as the PowerShell module's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completing {
    /// A quick-add line.
    Line,
    /// A filter, in the filter language.
    Filter,
    /// A project's name.
    Project,
    /// A label's name.
    Label,
}

impl Lumenna {
    /// What could come next at the end of `text`: a line's or a filter's from the parser's own
    /// completion, which every app's quick-add field offers; a project's or a label's name from
    /// the store, starting with what was typed, ignoring case, worded as the parser words its own.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn complete_typed(&self, kind: Completing, text: &str) -> Result<Completions> {
        let cursor = u32::try_from(text.len()).unwrap_or(u32::MAX);
        let (noun, rows) = match kind {
            Completing::Line => return self.complete_text(text, cursor, Syntax::QuickAdd),
            Completing::Filter => return self.complete_text(text, cursor, Syntax::Filter),
            Completing::Project => ("project", self.list_projects()?.rows),
            Completing::Label => ("label", self.list_labels()?.rows),
        };
        let typed = text.to_lowercase();
        let candidates: Vec<crate::Candidate> = rows
            .into_iter()
            .filter(|row| row.title.to_lowercase().starts_with(&typed))
            .map(|row| crate::Candidate { label: format!("{noun} {}", row.title), text: row.title, kind: noun.to_owned() })
            .collect();
        let announcement = match candidates.as_slice() {
            [only] => format!("1 completion, {}", only.label),
            all => crate::words::count_line(all.len(), "completion"),
        };
        Ok(Completions { announcement, notices: Vec::new(), start: 0, end: cursor, candidates })
    }
}
