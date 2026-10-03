//! Completion (§6.3, §6.4): one function, whatever the affordance.

use lumenna_core::model::{Label, Project};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_parse::complete::{CandidateKind, Syntax, complete};
use lumenna_parse::quickadd::Known;

fn known() -> Known {
    let mut snapshot = Snapshot::default();
    for name in ["Work", "Website", "My Big Project"] {
        let project = Project::new(name, OrderKey::middle());
        snapshot.projects.insert(project.id, project);
    }
    for name in ["laptop", "errand"] {
        let label = Label::new(name, OrderKey::middle());
        snapshot.labels.insert(label.id, label);
    }
    Known::from_snapshot(&snapshot)
}

fn texts(input: &str, cursor: usize, syntax: Syntax) -> Vec<String> {
    let mut out: Vec<String> =
        complete(input, cursor, syntax, &known()).candidates.into_iter().map(|c| c.text).collect();
    out.sort();
    out
}

#[test]
fn a_project_sigil_offers_projects() {
    let found = texts("#w", 2, Syntax::Filter);
    assert_eq!(found, vec!["#Website", "#Work"]);
}

#[test]
fn a_label_sigil_offers_labels_and_nothing_else() {
    // §6.2: `@` completion must offer the user's own labels, not a fixed vocabulary mixed in
    // among them — in a screen reader you cannot tell them apart by styling.
    let found = texts("@l", 2, Syntax::Filter);
    assert_eq!(found, vec!["@laptop"]);
}

#[test]
fn a_double_hash_completes_to_a_double_hash() {
    let found = texts("##wo", 4, Syntax::Filter);
    assert_eq!(found, vec!["##Work"]);
}

#[test]
fn a_name_with_spaces_comes_back_quoted() {
    // Inserting it unquoted would re-open the ambiguity completion just resolved (§6.2).
    let found = texts("#my", 3, Syntax::Filter);
    assert_eq!(found, vec!["#\"My Big Project\""]);
}

#[test]
fn the_replaced_span_is_the_token_being_typed() {
    let result = complete("#work & @la", 11, Syntax::Filter, &known());
    assert_eq!(result.replace_span, (8, 11));
    assert_eq!(&"#work & @la"[8..11], "@la");
}

#[test]
fn the_cursor_can_sit_inside_a_word() {
    // Caret between "@la" and "ptop" — the whole token is replaced, not just what precedes.
    let result = complete("@laptop", 3, Syntax::Filter, &known());
    assert_eq!(result.replace_span, (0, 7));
    assert_eq!(result.candidates.len(), 1);
}

#[test]
fn kind_is_carried_so_the_announcement_can_say_what_it_is() {
    // §6.3: "project Work" tells you what you are inserting where a bare "Work" does not.
    let result = complete("#w", 2, Syntax::Filter, &known());
    let first = &result.candidates[0];
    assert_eq!(first.kind, CandidateKind::Project);
    assert!(first.label.starts_with("project "), "{}", first.label);

    let result = complete("@l", 2, Syntax::Filter, &known());
    assert_eq!(result.candidates[0].kind, CandidateKind::Label);
    assert_eq!(result.candidates[0].label, "label laptop");
}

#[test]
fn the_count_is_announced_before_the_list() {
    let known = known();
    assert_eq!(complete("#w", 2, Syntax::Filter, &known).announcement, "2 completions");
    assert_eq!(
        complete("@l", 2, Syntax::Filter, &known).announcement,
        "1 completion, label laptop"
    );
    assert_eq!(
        complete("#zzz", 4, Syntax::Filter, &known).announcement,
        "no completions"
    );
}

#[test]
fn after_an_operator_a_filter_offers_predicates() {
    // This is what §6.3 wanted expected-token sets for: the parser is asked what belongs
    // here, rather than the affordance guessing.
    let found = texts("#work & ", 8, Syntax::Filter);
    assert!(found.contains(&"overdue".to_owned()), "{found:?}");
    assert!(found.contains(&"blocked".to_owned()), "{found:?}");
    assert!(found.contains(&"p1".to_owned()), "{found:?}");
    assert!(!found.contains(&"&".to_owned()), "an operator cannot follow an operator");
}

#[test]
fn after_a_complete_predicate_a_filter_offers_operators() {
    let found = texts("#work ", 6, Syntax::Filter);
    assert_eq!(found, vec!["!", "&", "|"]);
}

#[test]
fn a_partial_keyword_narrows_the_offer() {
    let found = texts("#work & blo", 11, Syntax::Filter);
    assert_eq!(found, vec!["blocked"]);
}

#[test]
fn quick_add_offers_the_vocabulary_that_is_otherwise_undiscoverable() {
    let found = texts("review PR ", 10, Syntax::QuickAdd);
    assert!(found.contains(&"tomorrow".to_owned()), "{found:?}");
    assert!(found.contains(&"every weekday".to_owned()), "{found:?}");
    assert!(found.contains(&"p1".to_owned()), "{found:?}");
    assert!(found.contains(&"#".to_owned()), "{found:?}");
}

#[test]
fn quick_add_completes_sigils_the_same_way_a_filter_does() {
    // One binding, context-determined (§6.4). The trigger never learns which is which.
    assert_eq!(texts("review PR #w", 12, Syntax::QuickAdd), vec!["#Website", "#Work"]);
    assert_eq!(texts("review PR @e", 12, Syntax::QuickAdd), vec!["@errand"]);
}

#[test]
fn an_empty_field_offers_somewhere_to_start() {
    let found = texts("", 0, Syntax::Filter);
    assert!(!found.is_empty());
    assert!(found.contains(&"#".to_owned()), "{found:?}");
}

#[test]
fn a_cursor_past_the_end_does_not_panic() {
    let known = known();
    let result = complete("#work", 999, Syntax::Filter, &known);
    assert_eq!(result.replace_span.1, 5);
}

#[test]
fn multi_byte_input_does_not_split_a_character() {
    let known = known();
    let input = "café #w";
    let result = complete(input, input.len(), Syntax::QuickAdd, &known);
    assert_eq!(result.candidates.len(), 2);
    assert_eq!(&input[result.replace_span.0..result.replace_span.1], "#w");
}
