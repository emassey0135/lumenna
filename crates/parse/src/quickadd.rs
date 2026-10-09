//! Quick add.
//!
//! Todoist-style quick add is the highest-leverage feature in the whole product for a screen
//! reader user: typing a full task specification beats navigating any date picker on any
//! platform.
//!
//! ```text
//! review PR tomorrow 3pm p1 #work @laptop
//!        ^          ^        ^  ^     ^
//!        title      date     pri proj label
//! ```
//!
//! # The preview is the whole point
//!
//! A sighted user gets live inline highlighting as they type — each recognised token
//! coloured as it is understood. That channel does not exist here, so [`Preview`] replaces
//! it: what was understood, what was not, and a sentence ready to be spoken.
//!
//! Three rules shape it:
//!
//! - **Always expose the resolved absolute date**, never just the phrase. "Friday" is
//!   ambiguous, and the resolution is the part worth confirming.
//! - **Never silently fold an unrecognised token into the title.** A swallowed date is
//!   invisible until the task fails to fire.
//! - **An unknown `@label` is a new label; an unknown `#project` is an error**. Both
//!   are reported with a nearest match, but only the label proceeds on confirmation.

use jiff::Zoned;
use lumenna_core::model::{Due, Priority};
use lumenna_core::snapshot::Snapshot;
use lumenna_core::suggest;
use lumenna_core::time::{DueSpec, RecurrenceSpec};

use crate::date::parse_when;
use crate::words::{Word, words};

/// Something recognised, and where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Byte offset of the first character.
    pub start: usize,
    /// Byte offset one past the last.
    pub end: usize,
}

/// The names quick add matches against.
///
/// The answer to the name ambiguity — *is `p1` part of the project name or a priority?* —
/// is to **greedy-match against known names** and support quoting as the escape hatch. That
/// makes parsing depend on what exists, which is why these are passed in rather than the
/// grammar being purely syntactic.
#[derive(Debug, Clone, Default)]
pub struct Known {
    /// Project names, as the user spelled them.
    pub projects: Vec<String>,
    /// Label names.
    pub labels: Vec<String>,
}

impl Known {
    /// The live names in a store, skipping deleted ones.
    #[must_use]
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        Self {
            projects: snapshot
                .projects
                .values()
                .filter(|p| p.deleted_at.is_none())
                .map(|p| p.name.clone())
                .collect(),
            labels: snapshot
                .labels
                .values()
                .filter(|l| l.deleted_at.is_none())
                .map(|l| l.name.clone())
                .collect(),
        }
    }
}

/// What the text said, before it is checked against the store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuickAdd {
    /// Everything that was not a recognised token.
    pub title: String,
    /// The `#project`, if one was given.
    pub project: Option<Spanned<String>>,
    /// Every `@label`, in the order they appeared.
    pub labels: Vec<Spanned<String>>,
    /// `p1`–`p4`.
    pub priority: Option<Spanned<Priority>>,
    /// The date phrase, unresolved.
    pub due: Option<Spanned<DueSpec>>,
    /// `45m`, `2h`, `1h30m`.
    pub estimate_mins: Option<Spanned<u32>>,
    /// Phrases that looked like they were going somewhere and were not understood.
    pub unparsed: Vec<Spanned<String>>,
    /// Second and later occurrences of something only one of which can be set.
    ///
    /// These are **left in the title** rather than consumed. Dropping them would be the
    /// swallowed-token failure to avoid: `book flight monday to friday` has two things
    /// that look like dates, and quietly discarding one leaves the user with a title that
    /// is missing a word and no way to notice.
    pub duplicates: Vec<Spanned<String>>,
}

/// Reads a quick-add line.
#[must_use]
pub fn parse_quick_add(input: &str, known: &Known) -> QuickAdd {
    let tokens = words(input);
    let mut out = QuickAdd::default();
    let mut consumed: Vec<(usize, usize)> = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        let word = &tokens[index];

        if let Some(rest) = word.lower.strip_prefix('#') {
            let (name, used) = sigil_name(input, &tokens, index, rest, &known.projects);
            let end = tokens[index + used - 1].end;
            if out.project.is_some() {
                out.duplicates.push(spanned_text(input, word.start, end));
                index += used;
                continue;
            }
            consumed.push((word.start, end));
            out.project = Some(Spanned { value: name, start: word.start, end });
            index += used;
            continue;
        }
        if let Some(rest) = word.lower.strip_prefix('@') {
            let (name, used) = sigil_name(input, &tokens, index, rest, &known.labels);
            let end = tokens[index + used - 1].end;
            consumed.push((word.start, end));
            out.labels.push(Spanned { value: name, start: word.start, end });
            index += used;
            continue;
        }
        if let Some(priority) = priority_of(&word.lower) {
            if out.priority.is_some() {
                out.duplicates.push(spanned_text(input, word.start, word.end));
                index += 1;
                continue;
            }
            consumed.push((word.start, word.end));
            out.priority =
                Some(Spanned { value: priority, start: word.start, end: word.end });
            index += 1;
            continue;
        }
        if let Some(when) = parse_when(&tokens, index)
            && !is_title_word(&tokens, index, when.words)
        {
            let end = tokens[index + when.words - 1].end;
            if out.due.is_some() {
                out.duplicates.push(spanned_text(input, word.start, end));
                index += when.words;
                continue;
            }
            consumed.push((word.start, end));
            out.due = Some(Spanned { value: when.spec, start: word.start, end });
            index += when.words;
            continue;
        }
        if let Some(minutes) = estimate_of(&word.lower) {
            if out.estimate_mins.is_some() {
                out.duplicates.push(spanned_text(input, word.start, word.end));
                index += 1;
                continue;
            }
            consumed.push((word.start, word.end));
            out.estimate_mins =
                Some(Spanned { value: minutes, start: word.start, end: word.end });
            index += 1;
            continue;
        }
        // A word that opens a date phrase but leads nowhere is worth saying out loud rather
        // than quietly becoming part of the title.
        if word.any_of(&["next", "last", "every", "in"])
            && tokens.get(index + 1).is_none_or(|next| looks_like_a_date(&next.lower))
        {
            let end = tokens.get(index + 1).map_or(word.end, |w| w.end);
            out.unparsed.push(Spanned {
                value: input.get(word.start..end).unwrap_or_default().to_owned(),
                start: word.start,
                end,
            });
        }
        index += 1;
    }

    out.title = title_without(input, consumed);
    out
}

/// Everything the recognised tokens did not take.
///
/// Cut from the original input rather than rejoined from words, so punctuation and spacing
/// survive: *"buy milk, eggs (not bread) tomorrow"* keeps its commas and its parentheses and
/// loses only the date. Runs of whitespace left behind by a removed span collapse to one.
/// Whether a one-word date phrase is more likely part of the title.
///
/// Two families of word are dates only in context:
///
/// - **"daily", "weekly", "monthly", "yearly"** stand for a repetition at the end of the
///   input or before another recognised token — *"water plants daily"*, *"standup daily
///   9am"* — and are an adjective anywhere else: *"write weekly report"*.
/// - **"sun", "sat", "wed"** are dates only with a time after them; on their own they are
///   English (see [`crate::date::is_ambiguous_weekday`]).
///
/// The readback names the date either way, so a wrong guess here is heard. A date silently
/// taken out of a title is the worse failure, and the one this avoids.
fn is_title_word(tokens: &[Word], index: usize, used: usize) -> bool {
    if used != 1 {
        return false;
    }
    let word = &tokens[index];
    if crate::date::is_ambiguous_weekday(&word.lower) {
        return true;
    }
    if !word.any_of(&["daily", "weekly", "monthly", "yearly", "annually"]) {
        return false;
    }
    let Some(next) = tokens.get(index + 1) else {
        return false;
    };
    let recognised = next.lower.starts_with(['#', '@'])
        || priority_of(&next.lower).is_some()
        || estimate_of(&next.lower).is_some()
        || parse_when(tokens, index + 1).is_some();
    !recognised
}

/// Whether a word could continue a date phrase, so that one which then fails to parse is
/// worth a notice. A phrase opener with nothing after it is always worth one — that is the
/// line entered half-written. "in the evening" and "last chapter" are titles, and saying so after every
/// one is noise a screen reader user has to sit through.
fn looks_like_a_date(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_digit())
        || crate::date::weekday_of(word).is_some()
        || crate::date::month_of(word).is_some()
        || matches!(
            word,
            "!" | "other" | "day" | "days" | "week" | "weeks" | "month" | "months" | "year"
                | "years" | "weekday" | "weekdays"
        )
}

fn title_without(input: &str, mut spans: Vec<(usize, usize)>) -> String {
    spans.sort_unstable();
    let mut kept = String::with_capacity(input.len());
    let mut at = 0;
    for (start, end) in spans {
        if start > at {
            kept.push_str(input.get(at..start).unwrap_or_default());
        }
        at = at.max(end);
    }
    kept.push_str(input.get(at..).unwrap_or_default());
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn spanned_text(input: &str, start: usize, end: usize) -> Spanned<String> {
    Spanned { value: input.get(start..end).unwrap_or_default().to_owned(), start, end }
}

/// Reads the name after a `#` or `@`, honouring quotes and greedy multi-word matching.
///
/// Returns the name and how many words it took.
fn sigil_name(
    input: &str,
    tokens: &[Word],
    at: usize,
    first: &str,
    known: &[String],
) -> (String, usize) {
    // `#"My Project"` is the escape hatch for a project genuinely named "work p1".
    if first.starts_with('"') {
        let mut name = String::new();
        for (offset, word) in tokens.iter().enumerate().skip(at) {
            let raw = word.raw(input);
            let piece = if offset == at { raw.trim_start_matches(['#', '@', '"']) } else { raw };
            let closed = piece.ends_with('"');
            if !name.is_empty() {
                name.push(' ');
            }
            name.push_str(piece.trim_end_matches('"'));
            if closed {
                return (name, offset - at + 1);
            }
        }
        return (name, tokens.len() - at);
    }

    // Greedy: prefer the longest known name that the following words spell out, so
    // `#My Project p1` takes the project and leaves the priority alone.
    let raw_first = tokens[at].raw(input).trim_start_matches(['#', '@']).to_owned();
    let mut best = (raw_first.clone(), 1);
    let mut candidate = raw_first;
    for (offset, word) in tokens.iter().enumerate().skip(at + 1) {
        candidate.push(' ');
        candidate.push_str(word.raw(input));
        if known.iter().any(|name| name.eq_ignore_ascii_case(&candidate)) {
            best = (candidate.clone(), offset - at + 1);
        }
    }
    let _ = first;
    best
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

/// `45m`, `90min`, `2h`, `1h30m`.
///
/// One glued token only. A loose `30 minutes` would take three words out of *"wait 30
/// minutes for the dough"* and put them in a field nobody was filling in — and `in 30
/// minutes` is a time rather than an estimate, which is indistinguishable once the number is
/// separated from its unit.
fn estimate_of(text: &str) -> Option<u32> {
    let mut total: u32 = 0;
    let mut digits = String::new();
    let mut matched = false;
    let mut rest = text;

    while !rest.is_empty() {
        let split = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        if split == 0 {
            return None;
        }
        digits.clear();
        digits.push_str(&rest[..split]);
        let value: u32 = digits.parse().ok()?;
        rest = &rest[split..];

        if let Some(after) = rest.strip_prefix("hours").or_else(|| rest.strip_prefix("hour")) {
            total = total.checked_add(value.checked_mul(60)?)?;
            rest = after;
        } else if let Some(after) = rest.strip_prefix('h') {
            total = total.checked_add(value.checked_mul(60)?)?;
            rest = after;
        } else {
            // Anything but a minute unit here means this word is not an estimate at all.
            let after = rest
                .strip_prefix("minutes")
                .or_else(|| rest.strip_prefix("minute"))
                .or_else(|| rest.strip_prefix("mins"))
                .or_else(|| rest.strip_prefix("min"))
                .or_else(|| rest.strip_prefix('m'))?;
            total = total.checked_add(value)?;
            rest = after;
        }
        matched = true;
    }
    (matched && total > 0).then_some(total)
}

/// How much a diagnostic matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Confirming would lose or mistake something. An unknown `#project` is this, because
    /// projects are not created implicitly — they have a parent, ordering, archive state and
    /// a weight, which is structure that wants a decision.
    Error,
    /// Worth saying before confirming, but confirming is fine. A new `@label` is this: labels
    /// are created implicitly, because being a record is an implementation fact the user
    /// should never have to think about during capture.
    Notice,
}

/// Something to say about the parse, with where in the input it applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// How much it matters.
    pub severity: Severity,
    /// Byte offset of the first character it refers to.
    pub start: usize,
    /// Byte offset one past the last.
    pub end: usize,
    /// The message, complete with position and token — because there is no squiggle to
    /// point at and this text is the only channel.
    pub message: String,
}

/// A parse checked against the store: what a task would become, and what to say first.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// The task's title.
    pub title: String,
    /// The resolved due date, if one was given and could be resolved.
    pub due: Option<Due>,
    /// The date phrase as typed, for reading back beside the resolved value.
    pub due_phrase: Option<String>,
    /// The repetition in English, if one was given.
    pub repetition: Option<String>,
    /// The project it would go in. `None` means the Inbox.
    pub project: Option<lumenna_core::id::ProjectId>,
    /// That project's name as stored, for reading back.
    pub project_name: Option<String>,
    /// Labels that already exist.
    pub labels: Vec<lumenna_core::id::LabelId>,
    /// Their names as stored, for reading back.
    pub label_names: Vec<String>,
    /// Labels that would be created on confirmation.
    pub new_labels: Vec<String>,
    /// The priority, defaulting to none.
    pub priority: Priority,
    /// The estimate in minutes, if one was given.
    pub estimate_mins: Option<u32>,
    /// Everything worth saying before confirming.
    pub diagnostics: Vec<Diagnostic>,
}

impl Preview {
    /// Whether anything would be lost or mistaken by confirming.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// The sentence to announce, standing in for the inline highlighting a sighted user gets.
    ///
    /// The resolved date is always spoken alongside the phrase, never instead of it: *"due
    /// next Friday, that is Friday 15 May 2026"*. The absolute value is there precisely
    /// because "Friday" is the ambiguous part, and confirming the phrase back would confirm
    /// nothing.
    #[must_use]
    pub fn announcement(&self) -> String {
        let mut parts = Vec::new();
        parts.push(if self.title.is_empty() {
            "Untitled task".to_owned()
        } else {
            self.title.clone()
        });

        if let Some(due) = &self.due {
            let mut phrase = match &self.due_phrase {
                Some(text) => format!("due {text}, that is {}", long_date(due.date)),
                None => format!("due {}", long_date(due.date)),
            };
            if let Some(time) = due.time {
                phrase.push_str(&format!(" at {}", clock_words(time)));
            }
            if let Some(repetition) = &self.repetition {
                phrase.push_str(&format!(", {repetition}"));
            }
            if due.recurrence.as_ref().is_some_and(|r| r.from_completion) {
                phrase.push_str(", counting from when you finish it");
            }
            parts.push(phrase);
        }
        // The project and labels are said as they resolved, since a name that matched the
        // wrong one is what the readback is there to catch.
        if let Some(project) = &self.project_name {
            parts.push(format!("in {project}"));
        }
        if !self.label_names.is_empty() {
            parts.push(format!("labelled {}", self.label_names.join(", ")));
        }
        if self.priority != Priority::P4 {
            parts.push(format!("priority {}", self.priority.as_u8()));
        }
        if let Some(minutes) = self.estimate_mins {
            parts.push(format!("estimated {minutes} minutes"));
        }
        for name in &self.new_labels {
            parts.push(format!("new label {name}"));
        }
        parts.join(", ")
    }
}

impl QuickAdd {
    /// Checks the parse against the store and resolves it against a clock.
    #[must_use]
    pub fn resolve(&self, snapshot: &Snapshot, now: &Zoned) -> Preview {
        let mut diagnostics = Vec::new();

        let project = self.project.as_ref().and_then(|spanned| {
            match snapshot.project_by_name(&spanned.value) {
                Some(project) => Some(project.id),
                None => {
                    let names: Vec<&str> = snapshot
                        .projects
                        .values()
                        .filter(|p| p.deleted_at.is_none())
                        .map(|p| p.name.as_str())
                        .collect();
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        start: spanned.start,
                        end: spanned.end,
                        message: unknown_message("project", &spanned.value, spanned.start, &names),
                    });
                    None
                }
            }
        });

        let mut labels = Vec::new();
        let mut new_labels = Vec::new();
        for spanned in &self.labels {
            match snapshot.label_by_name(&spanned.value) {
                Some(label) => labels.push(label.id),
                None => {
                    let names: Vec<&str> = snapshot
                        .labels
                        .values()
                        .filter(|l| l.deleted_at.is_none())
                        .map(|l| l.name.as_str())
                        .collect();
                    // Confirm-on-new, never prompt-on-known: silence would let typos
                    // accumulate, and a prompt on every label would make capture miserable.
                    // The readback already says "new label …"; a notice is added only when
                    // it has more to say, or every client would say it twice.
                    if let Some(near) = suggest::nearest(&spanned.value, names) {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Notice,
                            start: spanned.start,
                            end: spanned.end,
                            message: format!(
                                "new label '{}' at position {}; did you mean '{near}'?",
                                spanned.value, spanned.start
                            ),
                        });
                    }
                    new_labels.push(spanned.value.clone());
                }
            }
        }

        let mut due_phrase = None;
        let mut repetition = None;
        let due = self.due.as_ref().and_then(|spanned| {
            due_phrase = spanned.value.date.as_ref().map(ToString::to_string);
            repetition = spanned.value.recurrence.as_ref().map(RecurrenceSpec::describe);
            match spanned.value.resolve(now) {
                Ok(due) => due,
                Err(error) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        start: spanned.start,
                        end: spanned.end,
                        message: format!("could not read the repetition: {error}"),
                    });
                    None
                }
            }
        });

        for spanned in &self.duplicates {
            diagnostics.push(Diagnostic {
                severity: Severity::Notice,
                start: spanned.start,
                end: spanned.end,
                message: format!(
                    "'{}' at position {} repeats something already given; it stayed in the title",
                    spanned.value, spanned.start
                ),
            });
        }
        for spanned in &self.unparsed {
            diagnostics.push(Diagnostic {
                severity: Severity::Notice,
                start: spanned.start,
                end: spanned.end,
                message: format!(
                    "could not read a date from '{}' at position {}; it stayed in the title",
                    spanned.value, spanned.start
                ),
            });
        }
        diagnostics.sort_by_key(|d| d.start);

        let project_name = project.and_then(|id| snapshot.projects.get(&id)).map(|p| p.name.clone());
        let label_names = labels.iter().filter_map(|id| snapshot.labels.get(id)).map(|l| l.name.clone()).collect();
        Preview {
            title: self.title.clone(),
            due,
            due_phrase,
            repetition,
            project,
            project_name,
            labels,
            label_names,
            new_labels,
            priority: self.priority.as_ref().map_or(Priority::P4, |p| p.value),
            estimate_mins: self.estimate_mins.as_ref().map(|e| e.value),
            diagnostics,
        }
    }
}

fn unknown_message(noun: &str, name: &str, position: usize, candidates: &[&str]) -> String {
    match suggest::nearest(name, candidates.iter().copied()) {
        Some(near) => {
            format!("unknown {noun} '{name}' at position {position} — did you mean '{near}'?")
        }
        None => format!("unknown {noun} '{name}' at position {position}"),
    }
}

use lumenna_core::time::{clock_words, long_date};
