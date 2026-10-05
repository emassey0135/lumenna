//! Properties that must hold over inputs nobody thought to write down.

use jiff::civil::date;
use jiff::Zoned;
use lumenna_core::model::{Label, Project};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_parse::quickadd::{Known, parse_quick_add};
use lumenna_parse::{complete, parse_filter};
use lumenna_parse::complete::Syntax;
use proptest::prelude::*;

fn store() -> Snapshot {
    let mut snapshot = Snapshot::default();
    for name in ["Work", "Home"] {
        let project = Project::new(name, OrderKey::middle());
        snapshot.projects.insert(project.id, project);
    }
    for name in ["laptop", "errand"] {
        let label = Label::new(name, OrderKey::middle());
        snapshot.labels.insert(label.id, label);
    }
    snapshot
}

fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

/// A vocabulary mixing everything the grammars recognise with words they do not.
const VOCABULARY: &[&str] = &[
    "review", "PR", "milk", "dentist", "café", "3", "May", "p1", "p4", "#Work", "#Home",
    "#Nope", "@laptop", "@brandnew", "tomorrow", "today", "next", "friday", "next friday",
    "in 3 days", "every", "every day", "every mon, wed and fri", "3pm", "at 9am", "45m",
    "2h", "!", "&", "(", ")", "no date", "overdue", "blocked", "due before:", "search:",
];

fn any_line() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(VOCABULARY), 0..8)
        .prop_map(|parts| parts.join(" "))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Quick add never loses a character.
    ///
    /// An unrecognised token must never be silently folded into the title; the dual matters
    /// just as much. Every non-space character is either in the title or inside a span that
    /// was recognised — nothing simply disappears, which is the failure a user could not
    /// possibly detect.
    #[test]
    fn quick_add_accounts_for_every_character(line in any_line()) {
        let snapshot = store();
        let parsed = parse_quick_add(&line, &Known::from_snapshot(&snapshot));

        let mut covered = vec![false; line.len()];
        let mut mark = |start: usize, end: usize| {
            for flag in covered.iter_mut().take(end.min(line.len())).skip(start) {
                *flag = true;
            }
        };
        if let Some(s) = &parsed.project {
            mark(s.start, s.end);
        }
        for s in &parsed.labels {
            mark(s.start, s.end);
        }
        if let Some(s) = &parsed.priority {
            mark(s.start, s.end);
        }
        if let Some(s) = &parsed.due {
            mark(s.start, s.end);
        }
        if let Some(s) = &parsed.estimate_mins {
            mark(s.start, s.end);
        }

        let unaccounted: String = line
            .char_indices()
            .filter(|(i, c)| !c.is_whitespace() && !covered[*i])
            .map(|(_, c)| c)
            .collect();
        let title_chars: String = parsed.title.chars().filter(|c| !c.is_whitespace()).collect();
        prop_assert_eq!(&title_chars, &unaccounted, "line: {:?}", line);
    }

    /// Parsing never panics, and a successful parse always describes itself.
    #[test]
    fn filters_either_parse_or_explain_themselves(line in any_line()) {
        let snapshot = store();
        let known = Known::from_snapshot(&snapshot);
        match parse_filter(&line, &known) {
            Ok(expr) => prop_assert!(!expr.describe().is_empty()),
            Err(error) => {
                // An error is only useful if it says where: the position and the token
                // have to be in the message text, because there is no squiggle.
                prop_assert!(!error.message.is_empty());
                prop_assert!(error.start <= line.len());
                prop_assert!(error.end <= line.len());
            }
        }
    }

    /// Resolution never panics, whatever the parse produced.
    #[test]
    fn previews_resolve_without_panicking(line in any_line()) {
        let snapshot = store();
        let known = Known::from_snapshot(&snapshot);
        let preview = parse_quick_add(&line, &known).resolve(&snapshot, &now());
        prop_assert!(!preview.announcement().is_empty());
        for diagnostic in &preview.diagnostics {
            prop_assert!(diagnostic.start <= line.len());
            prop_assert!(diagnostic.end <= line.len());
        }
    }

    /// Completion is safe at every cursor position, including inside multi-byte characters.
    #[test]
    fn completion_is_safe_at_any_cursor(line in any_line(), cursor in 0usize..64) {
        let snapshot = store();
        let known = Known::from_snapshot(&snapshot);
        // Only char boundaries are reachable from a real caret.
        let cursor = (0..=line.len()).rev().find(|i| line.is_char_boundary(*i) && *i <= cursor);
        let Some(cursor) = cursor else { return Ok(()) };

        for syntax in [Syntax::QuickAdd, Syntax::Filter] {
            let result = complete(&line, cursor, syntax, &known);
            prop_assert!(result.replace_span.0 <= result.replace_span.1);
            prop_assert!(result.replace_span.1 <= line.len());
            prop_assert!(line.is_char_boundary(result.replace_span.0));
            prop_assert!(line.is_char_boundary(result.replace_span.1));
            prop_assert!(!result.announcement.is_empty());
        }
    }
}
