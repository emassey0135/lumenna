//! The filter query parser.
//!
//! Produces [`Expr`], which lives in `core` because the AST is the stable interface.
//! What is here is only the reading of text.
//!
//! # Why recursive descent
//!
//! A combinator library such as `chumsky` would offer one thing worth having: it reports
//! **expected-token sets at a position**, and completion depends on exactly that. This is
//! recursive descent instead, and the reason is the date grammar.
//!
//! `parse_when` works over **words**, because the actual requirement is knowing which
//! *span* a date phrase consumed, and because multi-word phrases — `next friday`, `every
//! mon, wed and fri`, `no estimate` — are the whole vocabulary. A character-level `chumsky`
//! grammar for filters would mean two tokenisations in one crate and a bridge between them
//! at every `due before:` — the same class of mistake as two date parsers, which is a bug
//! generator.
//!
//! That benefit is kept: [`ParseError::expected`] carries exactly the
//! token kinds that would have been valid, [`crate::complete`] is built on it, and it is
//! produced deliberately rather than inferred from a combinator's internals.

use lumenna_core::filter::{DueFilter, Expr, Predicate, Sort};
use lumenna_core::state::State;
use lumenna_core::suggest;
use lumenna_core::time::DateSpec;

use crate::date::parse_date;
use crate::quickadd::Known;
use crate::words::{Word, position, priority_of, sigil_name, words};

/// What could have appeared at the position where parsing stopped.
///
/// This is what completion is built on, and it is why the parser reports it rather than just
/// failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Expected {
    /// A `#project` name.
    Project,
    /// An `@label` name.
    Label,
    /// A `p1`–`p4`.
    Priority,
    /// A computed state, or another bare keyword.
    Keyword,
    /// A date phrase.
    Date,
    /// Free text to search for.
    Text,
    /// `&`, `|`, or their word forms.
    Operator,
    /// A closing parenthesis.
    CloseParen,
}

impl Expected {
    /// How to say it in an error message.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Project => "a project, like #work",
            Self::Label => "a label, like @laptop",
            Self::Priority => "a priority, p1 to p4",
            Self::Keyword => "a keyword, like overdue or blocked",
            Self::Date => "a date, like next friday",
            Self::Text => "some text to search for",
            Self::Operator => "an operator: & or |",
            Self::CloseParen => "a closing parenthesis",
        }
    }
}

/// A query that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Byte offset where the trouble is.
    pub start: usize,
    /// Byte offset one past it.
    pub end: usize,
    /// The message, carrying the position and the offending token in the text itself —
    /// because there is no squiggle to point at.
    pub message: String,
    /// What would have been valid here.
    pub expected: Vec<Expected>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

/// Reads a filter query.
///
/// An empty query is [`Expr::All`], which is what an empty filter box should mean.
///
/// # Errors
///
/// [`ParseError`] with the position, the token, and what would have been valid there.
pub fn parse_filter(input: &str, known: &Known) -> Result<Expr, ParseError> {
    let tokens = words(input);
    if tokens.is_empty() {
        return Ok(Expr::All);
    }
    // An order means something only to a list: anywhere else, say so rather than search for it.
    if let Some(word) = tokens.iter().find(|w| w.lower.starts_with("sort:")) {
        return Err(ParseError {
            start: word.start,
            end: word.end,
            message: format!("'sort:' at position {} orders a list, and only at the end of its query", position(input, word.start)),
            expected: vec![Expected::Operator],
        });
    }
    let mut parser = Cursor { input, tokens: &tokens, at: 0, known };
    let expr = parser.expression()?;
    if parser.at < parser.tokens.len() {
        let word = &parser.tokens[parser.at];
        return Err(parser.error(
            word,
            format!("unexpected '{}' at position {}", word.raw(input), position(input, word.start)),
            vec![Expected::Operator, Expected::CloseParen],
        ));
    }
    Ok(expr)
}

/// Reads a list's query: a filter, then optionally its order at the end, `sort: urgency` or
/// `sort: added`; `None` when it names none, so the list takes the `task-order` setting.
///
/// # Errors
///
/// As [`parse_filter`], or an order that is not one, or one not at the end.
pub fn parse_query(input: &str, known: &Known) -> Result<(Expr, Option<Sort>), ParseError> {
    let tokens = words(input);
    let Some(at) = tokens.iter().position(|w| w.lower.starts_with("sort:")) else {
        return Ok((parse_filter(input, known)?, None));
    };
    let word = &tokens[at];
    let glued = &word.lower["sort:".len()..];
    let (key, rest) = if glued.is_empty() { (tokens.get(at + 1), &tokens[(at + 2).min(tokens.len())..]) } else { (Some(word), &tokens[at + 1..]) };
    let key_text = if glued.is_empty() { key.map(|k| k.lower.as_str()) } else { Some(glued) };
    if let Some(after) = rest.first() {
        return Err(ParseError {
            start: after.start,
            end: after.end,
            message: format!("unexpected '{}' at position {}: sort: goes at the end", after.raw(input), position(input, after.start)),
            expected: Vec::new(),
        });
    }
    let sort = match key_text {
        Some("urgency") => Sort::Urgency,
        Some("added") => Sort::Added,
        Some(other) => {
            let key = key.unwrap_or(word);
            return Err(ParseError {
                start: key.start,
                end: key.end,
                message: format!("'{other}' at position {} is not an order: urgency or added", position(input, key.start)),
                expected: vec![Expected::Keyword],
            });
        }
        None => {
            return Err(ParseError {
                start: word.start,
                end: word.end,
                message: format!("'sort:' at position {} needs an order after it: urgency or added", position(input, word.start)),
                expected: vec![Expected::Keyword],
            });
        }
    };
    Ok((parse_filter(&input[..word.start], known)?, Some(sort)))
}

struct Cursor<'a> {
    input: &'a str,
    tokens: &'a [Word],
    at: usize,
    known: &'a Known,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&'a Word> {
        self.tokens.get(self.at)
    }

    fn eat_any(&mut self, texts: &[&str]) -> bool {
        if self.peek().is_some_and(|w| w.any_of(texts)) {
            self.at += 1;
            return true;
        }
        false
    }

    fn error(&self, word: &Word, message: String, expected: Vec<Expected>) -> ParseError {
        ParseError { start: word.start, end: word.end, message, expected }
    }

    fn at_end(&self, expected: Vec<Expected>) -> ParseError {
        let end = self.input.len();
        let names: Vec<&str> = expected.iter().map(|e| e.describe()).collect();
        ParseError {
            start: end,
            end,
            message: format!("the query ends too early; expected {}", names.join(", or ")),
            expected,
        }
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        let mut parts = vec![self.conjunction()?];
        while self.eat_any(&["|", "or"]) {
            parts.push(self.conjunction()?);
        }
        Ok(if parts.len() == 1 { parts.remove(0) } else { Expr::Or(parts) })
    }

    fn conjunction(&mut self) -> Result<Expr, ParseError> {
        let mut parts = vec![self.unary()?];
        while self.eat_any(&["&", "and"]) {
            parts.push(self.unary()?);
        }
        Ok(if parts.len() == 1 { parts.remove(0) } else { Expr::And(parts) })
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.eat_any(&["!", "not"]) {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        self.atom()
    }

    fn atom(&mut self) -> Result<Expr, ParseError> {
        if self.eat_any(&["("]) {
            let inner = self.expression()?;
            if !self.eat_any(&[")"]) {
                return Err(match self.peek() {
                    Some(word) => self.error(
                        word,
                        format!(
                            "expected a closing parenthesis at position {}, found '{}'",
                            position(self.input, word.start),
                            word.raw(self.input)
                        ),
                        vec![Expected::CloseParen, Expected::Operator],
                    ),
                    None => self.at_end(vec![Expected::CloseParen]),
                });
            }
            return Ok(inner);
        }
        self.predicate()
    }

    /// Reads the name after a sigil, as quick add reads it; a sigil naming nothing, `#` or
    /// `@""`, is refused rather than read as a project or label called nothing.
    fn sigil_name(&mut self, known: &[String], expected: Expected) -> Result<String, ParseError> {
        let word = self.tokens[self.at].clone();
        let (name, used) = sigil_name(self.input, self.tokens, self.at, known);
        if name.trim().is_empty() {
            return Err(self.error(
                &word,
                format!("'{}' at position {} names nothing", word.raw(self.input), position(self.input, word.start)),
                vec![expected],
            ));
        }
        self.at += used;
        Ok(name)
    }

    fn predicate(&mut self) -> Result<Expr, ParseError> {
        let Some(word) = self.peek() else {
            return Err(self.at_end(vec![
                Expected::Project,
                Expected::Label,
                Expected::Priority,
                Expected::Keyword,
            ]));
        };
        let word = word.clone();

        if word.lower.starts_with("##") {
            let name = self.sigil_name(&self.known.projects, Expected::Project)?;
            return Ok(Expr::Predicate(Predicate::Project {
                name,
                include_descendants: true,
            }));
        }
        if word.lower.starts_with('#') {
            let name = self.sigil_name(&self.known.projects, Expected::Project)?;
            return Ok(Expr::Predicate(Predicate::Project {
                name,
                include_descendants: false,
            }));
        }
        if word.lower.starts_with('@') {
            let name = self.sigil_name(&self.known.labels, Expected::Label)?;
            return Ok(Expr::Predicate(Predicate::Label(name)));
        }
        if let Some(priority) = priority_of(&word.lower) {
            self.at += 1;
            return Ok(Expr::Predicate(Predicate::Priority(priority)));
        }

        // `due before: friday`, `due after: today`, `due: monday`
        if word.lower.starts_with("due") {
            return self.due_predicate();
        }
        if let Some(rest) = keyed(&word.lower, "assigned") {
            let rest = rest.to_owned();
            self.at += 1;
            let spec = self.date_argument(&rest, "assigned")?;
            return Ok(Expr::Predicate(Predicate::Assigned(spec)));
        }
        if let Some(rest) = keyed(word.raw(self.input), "search") {
            self.at += 1;
            return Ok(Expr::Predicate(Predicate::Search(self.text_argument(rest)?)));
        }

        // Two-word states: `no date`, `no label`, `no project`, `no estimate`.
        if word.is("no")
            && let Some(next) = self.tokens.get(self.at + 1)
            && let Some(state) = State::from_keyword(&format!("no {}", next.lower))
        {
            self.at += 2;
            return Ok(Expr::Predicate(Predicate::State(state)));
        }
        if let Some(state) = State::from_keyword(&word.lower) {
            self.at += 1;
            return Ok(Expr::Predicate(Predicate::State(state)));
        }

        // `today`, and `7 days`.
        if word.is("today") {
            self.at += 1;
            return Ok(Expr::Predicate(Predicate::Due(DueFilter::Today)));
        }
        if let Ok(days) = word.lower.parse::<i64>()
            && self.tokens.get(self.at + 1).is_some_and(|w| w.any_of(&["day", "days"]))
        {
            self.reachable(&DateSpec::Offset { amount: days, unit: lumenna_core::time::RelativeUnit::Day }, 2)?;
            self.at += 2;
            return Ok(Expr::Predicate(Predicate::Due(DueFilter::Within { days })));
        }
        if let Some((spec, used)) = parse_date(self.tokens, self.at) {
            self.reachable(&spec, used)?;
            self.at += used;
            return Ok(Expr::Predicate(Predicate::Due(DueFilter::On(spec))));
        }

        let keywords: Vec<&str> = State::ALL.iter().map(|s| s.keyword()).collect();
        let hint = suggest::nearest(&word.lower, keywords)
            .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
        Err(self.error(
            &word,
            format!(
                "'{}' at position {} is not something to filter on{hint}",
                word.raw(self.input),
                position(self.input, word.start)
            ),
            vec![Expected::Project, Expected::Label, Expected::Priority, Expected::Keyword],
        ))
    }

    fn due_predicate(&mut self) -> Result<Expr, ParseError> {
        let word = self.tokens[self.at].clone();
        self.at += 1;
        // `due:friday` glued into one word, or `due` followed by its argument.
        let glued = word.lower.strip_prefix("due").unwrap_or("").trim_start_matches(':');
        let mut comparator = String::new();

        if glued.is_empty()
            && self.peek().is_some_and(|w| {
                w.any_of(&["before", "before:", "after", "after:", "on", "on:"])
            })
        {
            comparator = self.tokens[self.at].lower.trim_end_matches(':').to_owned();
            self.at += 1;
        }

        let spec = self.date_argument(glued, "due")?;
        Ok(Expr::Predicate(Predicate::Due(match comparator.as_str() {
            "before" => DueFilter::Before(spec),
            "after" => DueFilter::After(spec),
            _ if spec == DateSpec::Today => DueFilter::Today,
            _ => DueFilter::On(spec),
        })))
    }

    /// Refuses a date `used` words long at the cursor that no calendar reaches, naming it:
    /// it would resolve to nothing on every device the saved filter syncs to.
    fn reachable(&self, spec: &DateSpec, used: usize) -> Result<(), ParseError> {
        if spec.is_reachable() {
            return Ok(());
        }
        let (first, last) = (&self.tokens[self.at], &self.tokens[self.at + used - 1]);
        Err(ParseError {
            start: first.start,
            end: last.end,
            message: format!(
                "'{}' at position {} is further off than any date",
                &self.input[first.start..last.end],
                position(self.input, first.start)
            ),
            expected: vec![Expected::Date],
        })
    }

    /// Reads a date, either from a fragment glued onto the keyword or from the words after
    /// it.
    fn date_argument(&mut self, glued: &str, keyword: &str) -> Result<DateSpec, ParseError> {
        if !glued.is_empty() {
            // A date glued on is one word, which no offset is, so every one is reachable.
            if let Some(spec) = date_from_fragment(glued) {
                return Ok(spec);
            }
        } else if let Some((spec, used)) = parse_date(self.tokens, self.at) {
            self.reachable(&spec, used)?;
            self.at += used;
            return Ok(spec);
        }
        Err(match self.peek() {
            Some(word) => {
                let word = word.clone();
                self.error(
                    &word,
                    format!(
                        "expected a date after '{keyword}' at position {}, found '{}'",
                        position(self.input, word.start),
                        word.raw(self.input)
                    ),
                    vec![Expected::Date],
                )
            }
            None => self.at_end(vec![Expected::Date]),
        })
    }

    /// Reads the text after `search`, glued on (`search:milk`) or the next word, and a
    /// quoted phrase on to its closing quote either way: `search:"buy milk"`.
    fn text_argument(&mut self, inline: &str) -> Result<String, ParseError> {
        let raw = if inline.is_empty() {
            let Some(word) = self.peek() else {
                return Err(self.at_end(vec![Expected::Text]));
            };
            self.at += 1;
            word.raw(self.input).to_owned()
        } else {
            inline.to_owned()
        };
        if let Some(open) = raw.strip_prefix('"') {
            let mut text = open.to_owned();
            if raw.len() > 1 && raw.ends_with('"') {
                return Ok(text[..text.len() - 1].to_owned());
            }
            while let Some(next) = self.peek() {
                let piece = next.raw(self.input).to_owned();
                self.at += 1;
                text.push(' ');
                if let Some(closed) = piece.strip_suffix('"') {
                    text.push_str(closed);
                    return Ok(text);
                }
                text.push_str(&piece);
            }
            return Ok(text);
        }
        Ok(raw)
    }
}

/// Splits `assigned: today` written as one word, or bare `assigned`, returning whatever was
/// glued on after the colon. The key must be the whole word or end at its colon: `searching`
/// is not `search` with "ing" glued on.
fn keyed<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = text.get(..key.len()).filter(|start| start.eq_ignore_ascii_case(key)).map(|_| &text[key.len()..])?;
    if rest.is_empty() { Some(rest) } else { rest.strip_prefix(':') }
}

/// Parses a date out of a fragment that was glued onto a keyword, such as the `friday` in
/// `due:friday`. The whole fragment has to be the date, or it is not one.
fn date_from_fragment(fragment: &str) -> Option<DateSpec> {
    let tokens = words(fragment);
    let (spec, used) = parse_date(&tokens, 0)?;
    (used == tokens.len()).then_some(spec)
}
