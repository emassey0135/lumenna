//! The filter query language, read from text.

use jiff::civil::Weekday;
use lumenna_core::filter::{DueFilter, Expr, Predicate};
use lumenna_core::model::{Label, Priority, Project};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_core::state::State;
use lumenna_core::time::{DateSpec, Which};
use lumenna_parse::filter::Expected;
use lumenna_parse::quickadd::Known;
use lumenna_parse::parse_filter;

fn store() -> Snapshot {
    let mut snapshot = Snapshot::default();
    for name in ["Work", "My Big Project"] {
        let project = Project::new(name, OrderKey::middle());
        snapshot.projects.insert(project.id, project);
    }
    for name in ["laptop", "waiting"] {
        let label = Label::new(name, OrderKey::middle());
        snapshot.labels.insert(label.id, label);
    }
    snapshot
}

fn parse(input: &str) -> Expr {
    let snapshot = store();
    parse_filter(input, &Known::from_snapshot(&snapshot)).expect(input)
}

fn fails(input: &str) -> lumenna_parse::ParseError {
    let snapshot = store();
    parse_filter(input, &Known::from_snapshot(&snapshot)).unwrap_err()
}

fn project(name: &str, descendants: bool) -> Expr {
    Expr::Predicate(Predicate::Project {
        name: name.to_owned(),
        include_descendants: descendants,
    })
}

#[test]
fn an_empty_query_is_everything() {
    assert_eq!(parse(""), Expr::All);
    assert_eq!(parse("   "), Expr::All);
}

#[test]
fn the_canonical_query_parses_with_the_right_shape() {
    // #work & (p1 | overdue) & !@waiting
    assert_eq!(
        parse("#work & (p1 | overdue) & !@waiting"),
        Expr::And(vec![
            project("work", false),
            Expr::Or(vec![
                Expr::Predicate(Predicate::Priority(Priority::P1)),
                Expr::Predicate(Predicate::State(State::Overdue)),
            ]),
            Expr::Not(Box::new(Expr::Predicate(Predicate::Label("waiting".to_owned())))),
        ])
    );
}

#[test]
fn or_binds_looser_than_and() {
    // a & b | c parses as (a & b) | c, which is what every language this borrows from does.
    let parsed = parse("p1 & overdue | p2");
    assert!(matches!(parsed, Expr::Or(ref parts) if parts.len() == 2), "{parsed:?}");
    let Expr::Or(parts) = parsed else { unreachable!() };
    assert!(matches!(parts[0], Expr::And(_)));
    assert!(matches!(parts[1], Expr::Predicate(Predicate::Priority(Priority::P2))));
}

#[test]
fn parentheses_override_precedence() {
    let parsed = parse("p1 & (overdue | p2)");
    let Expr::And(parts) = parsed else { panic!("expected a conjunction") };
    assert!(matches!(parts[1], Expr::Or(_)));
}

#[test]
fn word_forms_of_the_operators_work_too() {
    assert_eq!(parse("p1 and overdue"), parse("p1 & overdue"));
    assert_eq!(parse("p1 or overdue"), parse("p1 | overdue"));
    assert_eq!(parse("not overdue"), parse("!overdue"));
}

#[test]
fn a_double_hash_asks_for_the_whole_subtree() {
    assert_eq!(parse("#work"), project("work", false));
    assert_eq!(parse("##work"), project("work", true));
}

#[test]
fn multi_word_names_match_greedily_or_by_quoting() {
    assert_eq!(parse("#My Big Project"), project("My Big Project", false));
    assert_eq!(parse(r#"#"My Big Project""#), project("My Big Project", false));
    // The greedy match stops at the longest known name, leaving the operator alone.
    let parsed = parse("#My Big Project & p1");
    let Expr::And(parts) = parsed else { panic!("expected a conjunction") };
    assert_eq!(parts[0], project("My Big Project", false));
}

#[test]
fn every_computed_state_is_a_bare_word() {
    for state in State::ALL {
        let parsed = parse(state.keyword());
        assert_eq!(parsed, Expr::Predicate(Predicate::State(*state)), "{}", state.keyword());
    }
}

#[test]
fn the_two_word_states_parse_as_one_predicate() {
    assert_eq!(parse("no date"), Expr::Predicate(Predicate::State(State::NoDate)));
    assert_eq!(parse("no estimate"), Expr::Predicate(Predicate::State(State::NoEstimate)));
    assert_eq!(
        parse("no label & no project"),
        Expr::And(vec![
            Expr::Predicate(Predicate::State(State::NoLabel)),
            Expr::Predicate(Predicate::State(State::NoProject)),
        ])
    );
}

#[test]
fn date_predicates_parse_in_the_forms_the_plan_names() {
    assert_eq!(parse("today"), Expr::Predicate(Predicate::Due(DueFilter::Today)));
    assert_eq!(
        parse("7 days"),
        Expr::Predicate(Predicate::Due(DueFilter::Within { days: 7 }))
    );
    assert_eq!(
        parse("due before: friday"),
        Expr::Predicate(Predicate::Due(DueFilter::Before(DateSpec::Weekday {
            day: Weekday::Friday,
            which: Which::This,
        })))
    );
    assert_eq!(
        parse("due after: next friday"),
        Expr::Predicate(Predicate::Due(DueFilter::After(DateSpec::Weekday {
            day: Weekday::Friday,
            which: Which::Next,
        })))
    );
    assert_eq!(parse("due: today"), Expr::Predicate(Predicate::Due(DueFilter::Today)));
    assert_eq!(
        parse("due tomorrow"),
        Expr::Predicate(Predicate::Due(DueFilter::On(DateSpec::Tomorrow)))
    );
}

#[test]
fn the_planner_predicates_parse() {
    // "Work tasks not already assigned today" is the canonical block filter.
    assert_eq!(
        parse("assigned: today"),
        Expr::Predicate(Predicate::Assigned(DateSpec::Today))
    );
    let parsed = parse("#work & !assigned: today");
    let Expr::And(parts) = parsed else { panic!("expected a conjunction") };
    assert_eq!(parts.len(), 2);
    assert!(matches!(parts[1], Expr::Not(_)));
}

#[test]
fn search_takes_one_word_or_a_quoted_phrase() {
    assert_eq!(
        parse("search: invoice"),
        Expr::Predicate(Predicate::Search("invoice".to_owned()))
    );
    assert_eq!(
        parse(r#"search: "unpaid invoice""#),
        Expr::Predicate(Predicate::Search("unpaid invoice".to_owned()))
    );
}

#[test]
fn a_saved_query_round_trips_through_its_own_readback() {
    // Not a formal guarantee, but the readback is only trustworthy if it describes the
    // thing that was actually parsed.
    let query = "#work & (p1 | overdue) & !@waiting";
    assert_eq!(
        parse(query).describe(),
        "tasks in work, and either priority 1 or overdue, and not labelled waiting"
    );
}

// ---------------------------------------------------------------------------------------
// Errors, which have to be speakable
// ---------------------------------------------------------------------------------------

#[test]
fn an_unknown_word_says_where_it_is_and_what_was_meant() {
    let error = fails("#work & blokced");
    assert!(error.message.contains("'blokced'"), "{}", error.message);
    assert!(error.message.contains("position 8"), "{}", error.message);
    assert!(error.message.contains("did you mean 'blocked'?"), "{}", error.message);
    assert_eq!(error.start, 8);
}

#[test]
fn an_unclosed_parenthesis_is_reported_at_the_end() {
    let error = fails("#work & (p1 | overdue");
    assert!(error.message.contains("ends too early"), "{}", error.message);
    assert!(error.expected.contains(&Expected::CloseParen));
}

#[test]
fn a_dangling_operator_says_what_belongs_next() {
    let error = fails("#work &");
    assert!(error.expected.contains(&Expected::Keyword), "{:?}", error.expected);
    assert!(error.message.contains("ends too early"), "{}", error.message);
}

#[test]
fn a_missing_date_after_due_is_reported_as_such() {
    let error = fails("due before: blah");
    assert!(error.message.contains("expected a date"), "{}", error.message);
    assert_eq!(error.expected, vec![Expected::Date]);
}

#[test]
fn trailing_junk_does_not_pass_silently() {
    let error = fails("p1 p2");
    assert!(error.message.contains("unexpected 'p2'"), "{}", error.message);
    assert!(error.expected.contains(&Expected::Operator));
}
