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
use crate::words::{Word, minutes_of, position, priority_of, sigil_name, words};

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
    /// The line as typed, for saying where in it each diagnostic is.
    pub input: String,
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
    /// A `#` or `@` with no name after it, left in the title.
    pub bare_sigils: Vec<Spanned<String>>,
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
    let mut out = QuickAdd { input: input.to_owned(), ..QuickAdd::default() };
    let mut consumed: Vec<(usize, usize)> = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        let word = &tokens[index];

        // A `#` or `@` alone names nothing: it stays in the title, and is said, rather than
        // being read as a project or label called nothing.
        if word.any_of(&["#", "@"]) {
            out.bare_sigils.push(spanned_text(input, word.start, word.end));
            index += 1;
            continue;
        }
        if word.lower.starts_with('#') {
            let (name, used) = sigil_name(input, &tokens, index, &known.projects);
            let end = tokens[index + used - 1].end;
            // `#""` names nothing, as a bare `#` does.
            if name.trim().is_empty() {
                out.bare_sigils.push(spanned_text(input, word.start, end));
                index += used;
                continue;
            }
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
        if word.lower.starts_with('@') {
            let (name, used) = sigil_name(input, &tokens, index, &known.labels);
            let end = tokens[index + used - 1].end;
            if name.trim().is_empty() {
                out.bare_sigils.push(spanned_text(input, word.start, end));
                index += used;
                continue;
            }
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
        if let Some(minutes) = minutes_of(&word.lower) {
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
        // "this month" and "this year" name no one day; "this" before anything else is English.
        let names_no_day = word.is("this") && tokens.get(index + 1).is_some_and(|next| next.any_of(&["month", "year"]));
        if (names_no_day || word.any_of(&["next", "last", "every", "in"]))
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
        || minutes_of(&next.lower).is_some()
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

/// Everything the recognised tokens did not take.
///
/// Cut from the original input rather than rejoined from words, so punctuation and spacing
/// survive: *"buy milk, eggs (not bread) tomorrow"* keeps its commas and its parentheses and
/// loses only the date. Runs of whitespace left behind by a removed span collapse to one.
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
    /// Whether the readback is terse (the `verbosity` setting): the date said once, as itself.
    pub terse: bool,
    /// The day it was read on, for saying a date without its year when it is this year's.
    pub today: jiff::civil::Date,
    /// Whether a time is said as 3:00 PM rather than 15:00: the person's clock, which
    /// [`QuickAdd::resolve`] cannot know and leaves twelve-hour.
    pub twelve_hour: bool,
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
            let mut phrase = match (&self.due_phrase, self.terse) {
                // Terse: the date itself, which is the check, and not the phrase that named it.
                (_, true) => format!("due {}", lumenna_core::time::day_words_long(due.date, self.today)),
                (Some(text), false) => format!("due {text}, that is {}", long_date(due.date)),
                (None, false) => format!("due {}", long_date(due.date)),
            };
            if let Some(time) = due.time {
                phrase.push_str(&format!(" at {}", clock_in(time, self.twelve_hour)));
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
            parts.push(format!("labelled {}", lumenna_core::words::listed(&self.label_names)));
        }
        if self.priority != Priority::P4 {
            parts.push(format!("priority {}", self.priority.as_u8()));
        }
        if let Some(minutes) = self.estimate_mins {
            parts.push(format!("estimated {}", minutes_words(minutes)));
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
                        message: unknown_message("project", &spanned.value, position(&self.input, spanned.start), &names),
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
                                spanned.value,
                                position(&self.input, spanned.start)
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
            match spanned.value.resolve_in(now, snapshot.settings.week_start) {
                Ok(None) if spanned.value.date.is_some() => {
                    // Cut from the title and then dropped would be the silent loss quick add
                    // exists to prevent: "pay rent feb 30" saved as "pay rent", due never.
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        start: spanned.start,
                        end: spanned.end,
                        message: format!(
                            "there is no date {}, at position {}",
                            due_phrase.as_deref().unwrap_or_default(),
                            position(&self.input, spanned.start)
                        ),
                    });
                    None
                }
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
                    spanned.value,
                    position(&self.input, spanned.start)
                ),
            });
        }
        for spanned in &self.bare_sigils {
            let noun = if spanned.value == "#" { "project" } else { "label" };
            diagnostics.push(Diagnostic {
                severity: Severity::Notice,
                start: spanned.start,
                end: spanned.end,
                message: format!(
                    "'{}' at position {} has no {noun} name after it; it stayed in the title",
                    spanned.value,
                    position(&self.input, spanned.start)
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
                    spanned.value,
                    position(&self.input, spanned.start)
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
            terse: snapshot.settings.verbosity == lumenna_core::model::Verbosity::Terse,
            today: now.date(),
            twelve_hour: true,
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

use lumenna_core::time::{clock_in, long_date, minutes_words};
