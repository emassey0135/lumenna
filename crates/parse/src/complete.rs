//! Completion (§6.3, §6.4).
//!
//! One function, affordance-agnostic. Triggering and presentation differ wildly per platform
//! — a real combobox on desktop, a custom accessibility action opening a modal list on
//! mobile, Ctrl-L on BTSpeak, Tab in a shell — but every one of them calls this and gets the
//! same answer.
//!
//! **The trigger never needs to know what is expected.** It calls [`complete`] with the text
//! and the cursor, and the parser decides whether a project, a label, a date, or a keyword
//! belongs there. `#`, `@`, and anything added later are covered by the same binding, which
//! is what makes one key enough on a device with no room for more.
//!
//! # `kind` is not decoration
//!
//! *"project Work"* tells you what you are inserting where a bare *"Work"* does not. On a
//! screen reader you cannot tell two kinds of candidate apart by styling — only by what is
//! said — so [`Candidate::label`] carries the noun and every affordance announces it.

use lumenna_core::model::Priority;
use lumenna_core::state::State;

use crate::filter::{Expected, parse_filter};
use crate::quickadd::Known;
use crate::words::{Word, word_at, words};

/// Which language the field is in.
///
/// The trigger does not know this; the field does, because it was built as one or the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// A quick-add line (§6.1).
    QuickAdd,
    /// A filter query (§6.2).
    Filter,
}

/// What sort of thing a candidate is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateKind {
    /// A project name.
    Project,
    /// A label name.
    Label,
    /// A priority.
    Priority,
    /// A computed state or other bare keyword.
    Keyword,
    /// A date phrase.
    Date,
    /// A boolean operator.
    Operator,
}

impl CandidateKind {
    /// The noun to announce before the candidate.
    #[must_use]
    pub const fn noun(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Label => "label",
            Self::Priority => "priority",
            Self::Keyword => "keyword",
            Self::Date => "date",
            Self::Operator => "operator",
        }
    }
}

/// One thing that could be inserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The text to put in, sigil included where one applies.
    pub text: String,
    /// What sort of thing it is.
    pub kind: CandidateKind,
    /// What to announce: *"project Work"*.
    pub label: String,
}

impl Candidate {
    fn new(kind: CandidateKind, text: impl Into<String>, name: &str) -> Self {
        Self { text: text.into(), kind, label: format!("{} {name}", kind.noun()) }
    }
}

/// Candidates for a position, and the span they replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completions {
    /// Byte offsets of the text to replace.
    pub replace_span: (usize, usize),
    /// What could go there.
    pub candidates: Vec<Candidate>,
    /// The count, phrased for speech. §6.3: announce the count before the list.
    pub announcement: String,
}

/// What could be inserted at `cursor`.
#[must_use]
pub fn complete(text: &str, cursor: usize, syntax: Syntax, known: &Known) -> Completions {
    let cursor = cursor.min(text.len());
    let tokens = words(text);
    let current = word_at(&tokens, cursor);

    let (span, prefix) = match current {
        Some((_, word)) => {
            let typed = text.get(word.start..cursor).unwrap_or_default();
            ((word.start, word.end), typed.to_lowercase())
        }
        None => ((cursor, cursor), String::new()),
    };

    let candidates = match prefix.strip_prefix("##") {
        Some(rest) => projects(known, rest, "##"),
        None => match prefix.strip_prefix('#') {
            Some(rest) => projects(known, rest, "#"),
            None => match prefix.strip_prefix('@') {
                Some(rest) => labels(known, rest),
                None => bare(text, cursor, &prefix, syntax, known, current.map(|(i, _)| i)),
            },
        },
    };

    Completions { replace_span: span, announcement: announce(&candidates), candidates }
}

fn projects(known: &Known, prefix: &str, sigil: &str) -> Vec<Candidate> {
    known
        .projects
        .iter()
        .filter(|name| name.to_lowercase().starts_with(prefix))
        .map(|name| Candidate::new(CandidateKind::Project, quoted(sigil, name), name))
        .collect()
}

fn labels(known: &Known, prefix: &str) -> Vec<Candidate> {
    known
        .labels
        .iter()
        .filter(|name| name.to_lowercase().starts_with(prefix))
        .map(|name| Candidate::new(CandidateKind::Label, quoted("@", name), name))
        .collect()
}

/// A name as the grammar takes it after a sigil: `#Work`, or `#"Home Office"` when it has a
/// space in it — the escape hatch the grammar offers (§6.2). Completion inserts names this
/// way, since an unquoted one would re-open the ambiguity completion just resolved, and every
/// client building a query or a quick-add prefix from a name does the same.
#[must_use]
pub fn quoted(sigil: &str, name: &str) -> String {
    if name.contains(char::is_whitespace) {
        format!("{sigil}\"{name}\"")
    } else {
        format!("{sigil}{name}")
    }
}

/// Candidates where no sigil has been typed yet.
///
/// For a filter this is where the parser earns its keep: parse everything before the cursor,
/// and whatever it says it [`Expected`] is what belongs here. For quick add there is no
/// grammar to consult — any word may be a title word — so the offer is the vocabulary that
/// would otherwise be undiscoverable.
fn bare(
    text: &str,
    cursor: usize,
    prefix: &str,
    syntax: Syntax,
    known: &Known,
    word_index: Option<usize>,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    let expected = match syntax {
        Syntax::QuickAdd => vec![Expected::Priority, Expected::Date, Expected::Project],
        Syntax::Filter => expected_at(text, cursor, known, word_index),
    };

    for kind in &expected {
        match kind {
            Expected::Priority => {
                for p in [Priority::P1, Priority::P2, Priority::P3, Priority::P4] {
                    let text = format!("p{}", p.as_u8());
                    out.push(Candidate::new(CandidateKind::Priority, text.clone(), &text));
                }
            }
            Expected::Keyword => {
                for state in State::ALL {
                    out.push(Candidate::new(
                        CandidateKind::Keyword,
                        state.keyword(),
                        state.keyword(),
                    ));
                }
                for word in ["today", "due before:", "due after:", "assigned:", "search:"] {
                    out.push(Candidate::new(CandidateKind::Keyword, word, word));
                }
            }
            Expected::Date => {
                for word in DATE_WORDS {
                    out.push(Candidate::new(CandidateKind::Date, *word, word));
                }
            }
            Expected::Operator => {
                for word in ["&", "|", "!"] {
                    out.push(Candidate::new(CandidateKind::Operator, word, word));
                }
            }
            Expected::Project => {
                out.push(Candidate::new(CandidateKind::Project, "#", "#"));
                out.push(Candidate::new(CandidateKind::Label, "@", "@"));
            }
            Expected::Label | Expected::Text | Expected::CloseParen => {}
        }
    }

    out.retain(|c| c.text.to_lowercase().starts_with(prefix));
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.text.cmp(&b.text)));
    out.dedup_by(|a, b| a.text == b.text);
    out
}

/// The date words worth offering. Not the whole grammar — the phrases someone would not
/// guess were available.
const DATE_WORDS: &[&str] = &[
    "today",
    "tomorrow",
    "next monday",
    "next week",
    "in 3 days",
    "every day",
    "every weekday",
    "every monday",
];

/// Runs the parser over everything before the cursor and reports what it wanted next.
///
/// This is §6.3's stated reason for wanting expected-token sets, used for exactly that.
fn expected_at(
    text: &str,
    cursor: usize,
    known: &Known,
    word_index: Option<usize>,
) -> Vec<Expected> {
    // Cut at the start of the word being typed, so a half-finished token does not become
    // the error the parser reports.
    let tokens = words(text);
    let cut = match word_index.and_then(|i| tokens.get(i)) {
        Some(word) => word.start,
        None => cursor,
    };
    let before = text.get(..cut).unwrap_or_default();

    match parse_filter(before, known) {
        // A complete expression: what follows is an operator, or the query simply ends.
        Ok(_) if !before.trim().is_empty() => vec![Expected::Operator],
        Ok(_) => vec![Expected::Project, Expected::Priority, Expected::Keyword, Expected::Date],
        Err(error) => error.expected,
    }
}

fn announce(candidates: &[Candidate]) -> String {
    match candidates.len() {
        0 => "no completions".to_owned(),
        1 => format!("1 completion, {}", candidates[0].label),
        n => format!("{n} completions"),
    }
}

/// The word the cursor is in, if any. Re-exported for callers that want to reason about the
/// same token this module does.
#[must_use]
pub fn token_at(text: &str, cursor: usize) -> Option<Word> {
    let tokens = words(text);
    word_at(&tokens, cursor.min(text.len())).map(|(_, word)| word.clone())
}
