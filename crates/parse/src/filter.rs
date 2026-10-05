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

use lumenna_core::filter::{DueFilter, Expr, Predicate};
use lumenna_core::model::Priority;
use lumenna_core::state::State;
use lumenna_core::suggest;
use lumenna_core::time::DateSpec;

use crate::date::parse_date;
use crate::quickadd::Known;
use crate::words::{Word, words};

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
    let mut parser = Cursor { input, tokens: &tokens, at: 0, known };
    let expr = parser.expression()?;
    if parser.at < parser.tokens.len() {
        let word = &parser.tokens[parser.at];
        return Err(parser.error(
            word,
            format!("unexpected '{}' at position {}", word.raw(input), word.start),
            vec![Expected::Operator, Expected::CloseParen],
        ));
    }
    Ok(expr)
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
                            word.start,
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

    /// Reads the name after a sigil, honouring quotes and greedy multi-word matching.
    fn sigil_name(&mut self, stripped: &str, known: &[String]) -> String {
        let first = self.tokens[self.at].clone();
        self.at += 1;

        if stripped.starts_with('"') {
            let mut name =
                first.raw(self.input).trim_start_matches(['#', '@', '"']).to_owned();
            if !first.raw(self.input).trim_start_matches(['#', '@']).ends_with('"')
                || name.is_empty()
            {
                while let Some(word) = self.peek() {
                    let raw = word.raw(self.input);
                    self.at += 1;
                    if !name.is_empty() {
                        name.push(' ');
                    }
                    name.push_str(raw.trim_end_matches('"'));
                    if raw.ends_with('"') {
                        break;
                    }
                }
            }
            return name.trim_end_matches('"').to_owned();
        }

        let base = first.raw(self.input).trim_start_matches(['#', '@']).to_owned();
        let mut best = (base.clone(), self.at);
        let mut candidate = base;
        for (offset, word) in self.tokens.iter().enumerate().skip(self.at) {
            candidate.push(' ');
            candidate.push_str(word.raw(self.input));
            if known.iter().any(|name| name.eq_ignore_ascii_case(&candidate)) {
                best = (candidate.clone(), offset + 1);
            }
        }
        self.at = best.1;
        best.0
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

        if let Some(rest) = word.lower.strip_prefix("##") {
            let name = self.sigil_name(rest, &self.known.projects.clone());
            return Ok(Expr::Predicate(Predicate::Project {
                name,
                include_descendants: true,
            }));
        }
        if let Some(rest) = word.lower.strip_prefix('#') {
            let name = self.sigil_name(rest, &self.known.projects.clone());
            return Ok(Expr::Predicate(Predicate::Project {
                name,
                include_descendants: false,
            }));
        }
        if let Some(rest) = word.lower.strip_prefix('@') {
            let name = self.sigil_name(rest, &self.known.labels.clone());
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
        if let Some(rest) = keyed(&word.lower, "search") {
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
            self.at += 2;
            return Ok(Expr::Predicate(Predicate::Due(DueFilter::Within { days })));
        }
        if let Some((spec, used)) = parse_date(self.tokens, self.at) {
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
                word.start
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

    /// Reads a date, either from a fragment glued onto the keyword or from the words after
    /// it.
    fn date_argument(&mut self, glued: &str, keyword: &str) -> Result<DateSpec, ParseError> {
        if !glued.is_empty() {
            if let Some(spec) = date_from_fragment(glued) {
                return Ok(spec);
            }
        } else if let Some((spec, used)) = parse_date(self.tokens, self.at) {
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
                        word.start,
                        word.raw(self.input)
                    ),
                    vec![Expected::Date],
                )
            }
            None => self.at_end(vec![Expected::Date]),
        })
    }

    fn text_argument(&mut self, inline: &str) -> Result<String, ParseError> {
        if !inline.is_empty() {
            return Ok(inline.trim_matches('"').to_owned());
        }
        let Some(word) = self.peek() else {
            return Err(self.at_end(vec![Expected::Text]));
        };
        let raw = word.raw(self.input).to_owned();
        self.at += 1;
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
/// glued on after the colon.
fn keyed<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = text.strip_prefix(key)?;
    Some(rest.strip_prefix(':').unwrap_or(rest))
}

/// Parses a date out of a fragment that was glued onto a keyword, such as the `friday` in
/// `due:friday`. The whole fragment has to be the date, or it is not one.
fn date_from_fragment(fragment: &str) -> Option<DateSpec> {
    let tokens = words(fragment);
    let (spec, used) = parse_date(&tokens, 0)?;
    (used == tokens.len()).then_some(spec)
}

fn priority_of(text: &str) -> Option<Priority> {
    match text {
        "p1" => Some(Priority::P1),
        "p2" => Some(Priority::P2),
        "p3" => Some(Priority::P3),
        "p4" => Some(Priority::P4),
        _ => None,
    }
}
