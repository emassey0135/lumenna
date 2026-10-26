//! Quick add — the highest-leverage feature in the product for a screen reader user.

use jiff::civil::{date, time};
use jiff::Zoned;
use lumenna_core::model::{Label, Priority, Project};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_core::time::{DateSpec, MonthDay, RecurrenceSpec, RelativeUnit, Which};
use lumenna_parse::quickadd::{Known, Preview, Severity, parse_quick_add};

/// Wednesday 2026-05-06, mid-afternoon.
fn now() -> Zoned {
    date(2026, 5, 6).at(14, 30, 0, 0).in_tz("America/New_York").unwrap()
}

fn store() -> Snapshot {
    let mut snapshot = Snapshot::default();
    let inbox = Project::inbox();
    snapshot.projects.insert(inbox.id, inbox);
    for name in ["Work", "My Big Project"] {
        let project = Project::new(name, OrderKey::middle());
        snapshot.projects.insert(project.id, project);
    }
    for name in ["laptop", "errand"] {
        let label = Label::new(name, OrderKey::middle());
        snapshot.labels.insert(label.id, label);
    }
    snapshot
}

fn preview(input: &str) -> Preview {
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    parse_quick_add(input, &known).resolve(&snapshot, &now())
}

#[test]
fn the_headline_example_parses() {
    // review PR tomorrow 3pm p1 #work @laptop
    let p = preview("review PR tomorrow 3pm p1 #work @laptop");
    assert_eq!(p.title, "review PR");
    assert_eq!(p.due.as_ref().unwrap().date, date(2026, 5, 7));
    assert_eq!(p.due.as_ref().unwrap().time, Some(time(15, 0, 0, 0)));
    assert_eq!(p.priority, Priority::P1);
    assert!(p.project.is_some());
    assert_eq!(p.labels.len(), 1);
    assert!(p.new_labels.is_empty());
    assert!(!p.has_errors());
}

#[test]
fn a_bare_line_is_just_a_title() {
    let p = preview("buy milk");
    assert_eq!(p.title, "buy milk");
    assert!(p.due.is_none());
    assert_eq!(p.priority, Priority::P4);
    assert!(p.diagnostics.is_empty());
}

#[test]
fn the_date_phrase_is_cut_out_of_the_title_wherever_it_sits() {
    // The actual requirement: knowing which span the date consumed.
    for (input, title) in [
        ("tomorrow call the dentist", "call the dentist"),
        ("call the dentist tomorrow", "call the dentist"),
        ("call next friday about the roof", "call about the roof"),
        ("submit in 3 days please", "submit please"),
    ] {
        let p = preview(input);
        assert_eq!(p.title, title, "{input}");
        assert!(p.due.is_some(), "{input}");
    }
}

#[test]
fn dates_of_every_supported_shape_resolve() {
    let cases = [
        ("today", date(2026, 5, 6)),
        ("tomorrow", date(2026, 5, 7)),
        ("yesterday", date(2026, 5, 5)),
        ("friday", date(2026, 5, 8)),
        ("next friday", date(2026, 5, 15)),
        ("last friday", date(2026, 5, 1)),
        // The first day of next week, which starts on Monday unless the setting says not.
        ("next week", date(2026, 5, 11)),
        ("end of the week", date(2026, 5, 10)),
        ("in 3 days", date(2026, 5, 9)),
        ("in 2 weeks", date(2026, 5, 20)),
        ("2026-09-01", date(2026, 9, 1)),
        ("sep 1", date(2026, 9, 1)),
        ("1 september", date(2026, 9, 1)),
        ("september 1st", date(2026, 9, 1)),
    ];
    for (phrase, expected) in cases {
        let p = preview(&format!("task {phrase}"));
        assert_eq!(p.due.as_ref().map(|d| d.date), Some(expected), "{phrase}");
        assert_eq!(p.title, "task", "{phrase}");
    }
}

#[test]
fn times_of_every_supported_shape_resolve() {
    let cases = [
        ("3pm", time(15, 0, 0, 0)),
        ("3:30pm", time(15, 30, 0, 0)),
        ("at 9am", time(9, 0, 0, 0)),
        ("15:00", time(15, 0, 0, 0)),
        ("9:05", time(9, 5, 0, 0)),
        ("noon", time(12, 0, 0, 0)),
        ("midnight", time(0, 0, 0, 0)),
        ("12pm", time(12, 0, 0, 0)),
        ("12am", time(0, 0, 0, 0)),
    ];
    for (phrase, expected) in cases {
        let p = preview(&format!("task tomorrow {phrase}"));
        assert_eq!(p.due.as_ref().and_then(|d| d.time), Some(expected), "{phrase}");
        assert_eq!(p.title, "task", "{phrase}");
    }
}

#[test]
fn a_bare_number_is_not_a_time() {
    // "Call mum 3" is not three o'clock, and treating it as one would eat a digit out of
    // every title that has one.
    let p = preview("call mum 3");
    assert_eq!(p.title, "call mum 3");
    assert!(p.due.is_none());
}

#[test]
fn a_bare_month_name_is_not_a_date() {
    let p = preview("ask May about the report");
    assert_eq!(p.title, "ask May about the report");
    assert!(p.due.is_none());
}

#[test]
fn recurrence_phrases_become_rules() {
    let cases = [
        ("every day", RecurrenceSpec::Daily { interval: 1 }),
        ("every 3 days", RecurrenceSpec::Daily { interval: 3 }),
        ("every other day", RecurrenceSpec::Daily { interval: 2 }),
        ("daily", RecurrenceSpec::Daily { interval: 1 }),
        ("every week", RecurrenceSpec::Weekly { interval: 1, days: vec![] }),
        ("every weekday", RecurrenceSpec::Weekdays),
        ("every month", RecurrenceSpec::Monthly { interval: 1, day: None }),
        ("every year", RecurrenceSpec::Yearly { interval: 1 }),
        (
            "every 15th",
            RecurrenceSpec::Monthly { interval: 1, day: Some(MonthDay::Nth(15)) },
        ),
        (
            "every last day of the month",
            RecurrenceSpec::Monthly { interval: 1, day: Some(MonthDay::Last) },
        ),
    ];
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    for (phrase, expected) in cases {
        let parsed = parse_quick_add(&format!("task {phrase}"), &known);
        assert_eq!(parsed.title, "task", "{phrase}");
        assert_eq!(
            parsed.due.as_ref().unwrap().value.recurrence.as_ref(),
            Some(&expected),
            "{phrase}"
        );
    }
}

#[test]
fn weekday_lists_become_one_rule() {
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    let parsed = parse_quick_add("standup every mon, wed and fri at 9am", &known);
    assert_eq!(parsed.title, "standup");
    let spec = &parsed.due.as_ref().unwrap().value;
    assert_eq!(
        spec.recurrence.as_ref().unwrap().to_rrule(),
        "FREQ=WEEKLY;BYDAY=MO,WE,FR"
    );
    assert_eq!(spec.time, Some(time(9, 0, 0, 0)));
}

#[test]
fn a_trailing_and_is_not_eaten_by_a_weekday_list() {
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    let parsed = parse_quick_add("every mon and call the dentist", &known);
    assert_eq!(parsed.title, "and call the dentist");
    assert_eq!(
        parsed.due.as_ref().unwrap().value.recurrence.as_ref().unwrap().to_rrule(),
        "FREQ=WEEKLY;BYDAY=MO"
    );
}

#[test]
fn the_bang_means_advance_from_completion() {
    // Todoist's `every!`: the difference shows up exactly when you are late.
    let p = preview("water plants every! 3 days");
    let recurrence = p.due.as_ref().unwrap().recurrence.as_ref().unwrap();
    assert_eq!(recurrence.rrule, "FREQ=DAILY;INTERVAL=3");
    assert!(recurrence.from_completion);

    let p = preview("water plants every 3 days");
    assert!(!p.due.unwrap().recurrence.unwrap().from_completion);
}

#[test]
fn a_repetition_with_no_day_starts_at_its_first_real_occurrence() {
    // Typed on a Wednesday: a weekly-Monday series must not be anchored on a Wednesday,
    // which is a day it never occurs.
    let p = preview("standup every monday");
    assert_eq!(p.due.as_ref().unwrap().date, date(2026, 5, 11));
}

#[test]
fn estimates_parse_in_the_forms_people_write_them() {
    for (phrase, minutes) in [("45m", 45), ("90min", 90), ("2h", 120), ("1h30m", 90)] {
        let p = preview(&format!("task {phrase}"));
        assert_eq!(p.estimate_mins, Some(minutes), "{phrase}");
    }
}

#[test]
fn a_spelled_out_duration_stays_in_the_title() {
    // An estimate has to be one glued token. "wait 30 minutes for the dough" would
    // otherwise lose three words to a field nobody was filling in, and "in 30 minutes" is a
    // time, not an estimate — the two are indistinguishable once the number is loose.
    let p = preview("wait 30 minutes for the dough");
    assert_eq!(p.estimate_mins, None);
    assert_eq!(p.title, "wait 30 minutes for the dough");
}

#[test]
fn a_multi_word_project_is_matched_greedily_and_the_priority_survives() {
    // The name ambiguity: is `p1` part of the name or a priority?
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    let parsed = parse_quick_add("draft #My Big Project p1", &known);
    assert_eq!(parsed.project.as_ref().unwrap().value, "My Big Project");
    assert_eq!(parsed.priority.as_ref().unwrap().value, Priority::P1);
    assert_eq!(parsed.title, "draft");
}

#[test]
fn quoting_is_the_escape_hatch() {
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    let parsed = parse_quick_add(r#"draft #"My Big Project" p1"#, &known);
    assert_eq!(parsed.project.as_ref().unwrap().value, "My Big Project");
    assert_eq!(parsed.priority.as_ref().unwrap().value, Priority::P1);
}

#[test]
fn an_unknown_project_is_an_error_and_an_unknown_label_is_a_new_label() {
    // The asymmetry: a project has a parent, ordering, archive state and a weight, which
    // is structure that wants a decision. A label has none of that.
    let p = preview("task #Nonexistent @brandnew");
    assert!(p.has_errors());
    assert_eq!(p.project, None);
    assert_eq!(p.new_labels, vec!["brandnew"]);

    let severities: Vec<Severity> = p.diagnostics.iter().map(|d| d.severity).collect();
    assert!(severities.contains(&Severity::Error));
    // The readback says "new label brandnew"; a notice saying it again would be said twice.
    assert!(!severities.contains(&Severity::Notice), "{:?}", p.diagnostics);
}

#[test]
fn a_typo_is_reported_with_its_position_and_the_nearest_match() {
    // The position and the token must be in the message text, because there is no
    // squiggle to point at.
    let p = preview("task @lapto");
    let message = &p.diagnostics[0].message;
    assert!(message.contains("new label 'lapto'"), "{message}");
    assert!(message.contains("did you mean 'laptop'?"), "{message}");
    assert!(message.contains("position 6"), "counted from one: {message}");
    assert_eq!(p.diagnostics[0].severity, Severity::Notice);

    let p = preview("task #Wrok");
    let message = &p.diagnostics[0].message;
    assert!(message.contains("unknown project 'Wrok'"), "{message}");
    assert!(message.contains("did you mean 'Work'?"), "{message}");
}

#[test]
fn a_known_label_is_never_prompted_about() {
    // Confirm-on-new, never prompt-on-known.
    let p = preview("task @laptop");
    assert!(p.diagnostics.is_empty());
    assert_eq!(p.labels.len(), 1);
    assert!(p.new_labels.is_empty());
}

#[test]
fn a_half_written_date_is_reported_rather_than_swallowed() {
    // Never silently fold an unrecognised token into the title. A swallowed date is
    // invisible until the task fails to fire.
    let p = preview("call next");
    assert!(p.due.is_none());
    assert_eq!(p.title, "call next");
    assert_eq!(p.diagnostics.len(), 1);
    assert!(p.diagnostics[0].message.contains("could not read a date"), "{:?}", p.diagnostics);
    assert!(p.diagnostics[0].message.contains("stayed in the title"));
}

#[test]
fn the_announcement_always_states_the_resolved_date() {
    // "Friday" is ambiguous, and the resolution is the part worth confirming.
    let p = preview("review PR next friday 3pm p1");
    let said = p.announcement();
    assert!(said.starts_with("review PR"), "{said}");
    assert!(said.contains("next Friday"), "{said}");
    assert!(said.contains("Friday 15 May 2026"), "{said}");
    assert!(said.contains("3:00 PM"), "{said}");
    assert!(said.contains("priority 1"), "{said}");
}

#[test]
fn the_announcement_mentions_a_label_that_would_be_created() {
    let p = preview("task @brandnew");
    assert!(p.announcement().contains("new label brandnew"), "{}", p.announcement());
}

#[test]
fn an_empty_line_announces_as_untitled_rather_than_as_nothing() {
    let p = preview("");
    assert_eq!(p.title, "");
    assert_eq!(p.announcement(), "Untitled task");
}

#[test]
fn spans_point_at_what_was_recognised() {
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    let input = "review PR tomorrow 3pm p1 #Work @laptop";
    let parsed = parse_quick_add(input, &known);

    let due = parsed.due.as_ref().unwrap();
    assert_eq!(&input[due.start..due.end], "tomorrow 3pm");
    let project = parsed.project.as_ref().unwrap();
    assert_eq!(&input[project.start..project.end], "#Work");
    let label = &parsed.labels[0];
    assert_eq!(&input[label.start..label.end], "@laptop");
    let priority = parsed.priority.as_ref().unwrap();
    assert_eq!(&input[priority.start..priority.end], "p1");
}

#[test]
fn the_date_spec_is_kept_unresolved_so_it_can_be_read_back() {
    let snapshot = store();
    let known = Known::from_snapshot(&snapshot);
    let parsed = parse_quick_add("task next friday", &known);
    assert_eq!(
        parsed.due.as_ref().unwrap().value.date,
        Some(DateSpec::Weekday {
            day: jiff::civil::Weekday::Friday,
            which: Which::Next,
        })
    );

    let parsed = parse_quick_add("task in 2 weeks", &known);
    assert_eq!(
        parsed.due.as_ref().unwrap().value.date,
        Some(DateSpec::Offset { amount: 2, unit: RelativeUnit::Week })
    );
}

#[test]
fn a_repeated_token_stays_in_the_title_and_is_mentioned() {
    // "book flight monday to friday" has two things that look like dates. Quietly
    // discarding one leaves a title missing a word and no way to notice.
    let p = preview("book flight monday to friday");
    assert_eq!(p.due.as_ref().unwrap().date, date(2026, 5, 11), "the first one wins");
    assert_eq!(p.title, "book flight to friday");
    assert_eq!(p.diagnostics.len(), 1);
    assert!(p.diagnostics[0].message.contains("repeats something already given"));
    assert!(p.diagnostics[0].message.contains("stayed in the title"));

    let p = preview("task p1 p2");
    assert_eq!(p.priority, Priority::P1);
    assert_eq!(p.title, "task p2");
}

#[test]
fn a_repetition_adjective_inside_a_title_stays_there() {
    for input in ["write weekly report", "read daily news", "pay monthly rent p2"] {
        let p = preview(input);
        assert!(p.due.is_none(), "{input}");
        assert!(p.title.split(' ').count() >= 3, "{input}: {}", p.title);
    }
    // At the end, or before another token, it is still a repetition.
    for input in ["water plants daily", "standup daily 9am", "standup weekly #Work"] {
        let p = preview(input);
        assert!(p.due.as_ref().is_some_and(|d| d.recurrence.is_some()), "{input}");
    }
}

#[test]
fn weekday_abbreviations_that_are_words_need_context_to_be_dates() {
    let p = preview("buy sun cream");
    assert!(p.due.is_none());
    assert_eq!(p.title, "buy sun cream");
    assert!(preview("get wed").due.is_none());

    // Saturday the ninth, from Wednesday the sixth.
    for input in ["call mum on sat", "call mum next sat", "call mum sat 3pm"] {
        let p = preview(input);
        assert_eq!(p.title, "call mum", "{input}");
        assert!(p.due.is_some(), "{input}");
    }
    assert_eq!(preview("call mum on sat").due.unwrap().date, date(2026, 5, 9));
}

#[test]
fn a_word_ending_in_s_is_not_a_weekday_by_accident() {
    let p = preview("thus spoke zarathustra");
    assert!(p.due.is_none());
    assert_eq!(preview("standup every mondays").due.unwrap().date, date(2026, 5, 11));
}

#[test]
fn an_ordinary_preposition_is_not_reported_as_a_broken_date() {
    for input in ["put files in the folder", "read the last chapter", "plan next steps"] {
        let p = preview(input);
        assert!(p.diagnostics.is_empty(), "{input}: {:?}", p.diagnostics);
    }
    // ...but one that looks like it was starting a date still is.
    assert!(!preview("call in 3 dys").diagnostics.is_empty());
}

#[test]
fn the_readback_names_the_project_and_labels_as_they_resolved() {
    let p = preview("call the bank #work @errand");
    let said = p.announcement();
    assert!(said.contains("in Work"), "{said}");
    assert!(said.contains("labelled errand"), "{said}");
}

#[test]
fn a_hash_or_at_sign_with_no_name_stays_in_the_title_and_is_said() {
    let p = preview("Call the bank # tomorrow");
    assert_eq!(p.title, "Call the bank #");
    assert_eq!(p.project, None, "not a project called nothing");
    assert_eq!(p.diagnostics.len(), 1, "{:?}", p.diagnostics);
    assert_eq!(p.diagnostics[0].severity, Severity::Notice);
    assert_eq!(p.diagnostics[0].message, "'#' at position 15 has no project name after it; it stayed in the title");

    let p = preview("Buy milk @");
    assert_eq!(p.title, "Buy milk @");
    assert!(p.labels.is_empty() && p.new_labels.is_empty(), "no label called nothing: {:?}", p.new_labels);
    assert_eq!(p.diagnostics[0].message, "'@' at position 10 has no label name after it; it stayed in the title");
}

#[test]
fn next_week_and_the_end_of_the_week_follow_the_week_start_setting() {
    let mut snapshot = Snapshot::default();
    let due = |snapshot: &Snapshot, line: &str| {
        let known = Known::from_snapshot(snapshot);
        parse_quick_add(line, &known).resolve(snapshot, &now()).due.map(|d| d.date)
    };
    let monday = due(&snapshot, "plan the trip next week").unwrap();
    assert_eq!(monday.weekday(), jiff::civil::Weekday::Monday);
    snapshot.settings.week_start = jiff::civil::Weekday::Sunday;
    let sunday = due(&snapshot, "plan the trip next week").unwrap();
    assert_eq!(sunday.weekday(), jiff::civil::Weekday::Sunday);
    let end = due(&snapshot, "file the report by end of the week").unwrap();
    assert_eq!(end.weekday(), jiff::civil::Weekday::Saturday, "a week from Sunday ends on Saturday");
    let preview = parse_quick_add("file the report by end of the week", &Known::from_snapshot(&snapshot)).resolve(&snapshot, &now());
    assert_eq!(preview.title, "file the report by");
}

#[test]
fn a_terse_readback_says_the_date_once_as_itself() {
    let mut snapshot = store();
    let line = "Call Sam tomorrow at 3pm p1";
    let full = parse_quick_add(line, &Known::from_snapshot(&snapshot)).resolve(&snapshot, &now()).announcement();
    assert_eq!(full, "Call Sam, due tomorrow, that is Thursday 7 May 2026 at 3:00 PM, priority 1");
    snapshot.settings.verbosity = lumenna_core::model::Verbosity::Terse;
    let terse = parse_quick_add(line, &Known::from_snapshot(&snapshot)).resolve(&snapshot, &now()).announcement();
    assert_eq!(terse, "Call Sam, due Thursday 7 May at 3:00 PM, priority 1");
}

#[test]
fn a_date_that_does_not_exist_is_an_error_naming_it_not_a_title_quietly_shortened() {
    // 2026 is no leap year, and the search for the next 29 February reaches 2028.
    assert_eq!(preview("pay rent feb 29").due.unwrap().date, date(2028, 2, 29));
    for (line, named) in [("pay rent feb 30", "February 30"), ("x in 99999 months", "in 99999 months"), ("x in 9999999 days", "in 9999999 days"), ("x 99999999999 weeks ago", "99999999999 weeks ago"), ("x in 99999999 years", "in 99999999 years")] {
        let p = preview(line);
        assert!(p.due.is_none(), "{line}");
        assert!(p.has_errors(), "{line}: {:?}", p.diagnostics);
        assert!(p.diagnostics.iter().any(|d| d.message.contains(named)), "{line}: {:?}", p.diagnostics);
    }
}

#[test]
fn this_day_is_today_and_this_month_is_no_date_and_said() {
    let p = preview("call this day");
    assert_eq!(p.due.unwrap().date, date(2026, 5, 6));
    let p = preview("call this month");
    assert!(p.due.is_none());
    assert_eq!(p.title, "call this month");
    assert!(!p.diagnostics.is_empty(), "the phrase is said, not left silent");
}

#[test]
fn a_day_after_the_time_is_the_same_phrase() {
    let p = preview("meet at 3pm tomorrow");
    assert_eq!(p.title, "meet");
    let due = p.due.unwrap();
    assert_eq!((due.date, due.time), (date(2026, 5, 7), Some(time(15, 0, 0, 0))));
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
}

#[test]
fn an_ordinal_before_a_unit_counts_the_units() {
    let p = preview("run every 3rd day");
    assert_eq!(p.title, "run");
    assert_eq!(p.due.unwrap().recurrence.unwrap().rrule, "FREQ=DAILY;INTERVAL=3");
    assert_eq!(preview("bins every 2nd week").due.unwrap().recurrence.unwrap().rrule, "FREQ=WEEKLY;INTERVAL=2");
    assert_eq!(preview("rent every 15th").due.unwrap().recurrence.unwrap().rrule, "FREQ=MONTHLY;BYMONTHDAY=15", "with no unit it is still the day of the month");
}

#[test]
fn every_other_weekday_is_said_rather_than_read_as_every_weekday() {
    let p = preview("stretch every other weekday");
    assert!(p.due.is_none(), "{:?}", p.due);
    assert_eq!(p.title, "stretch every other weekday");
    assert!(!p.diagnostics.is_empty());
}

#[test]
fn the_readback_says_the_time_in_the_persons_clock() {
    let mut p = preview("Call Sam tomorrow at 3pm");
    p.twelve_hour = false;
    assert!(p.announcement().contains("at 15:00"), "{}", p.announcement());
}

#[test]
fn one_minute_is_said_as_one() {
    assert!(preview("stretch 1m").announcement().contains("estimated 1 minute"), "{}", preview("stretch 1m").announcement());
    assert_eq!(preview("read 45minutes").estimate_mins, Some(45));
}

#[test]
fn an_empty_quoted_name_stays_in_the_title_and_takes_nothing_after_it() {
    let p = preview(r#"call #"" now p1"#);
    assert_eq!(p.priority, Priority::P1, "the priority after it is still read");
    assert_eq!(p.title, r#"call #"" now"#);
    assert!(p.project.is_none());
}

#[test]
fn a_name_matches_whatever_its_case_beyond_ascii() {
    let mut snapshot = store();
    let project = Project::new("École", OrderKey::middle());
    snapshot.projects.insert(project.id, project);
    let p = parse_quick_add("enrol #école", &Known::from_snapshot(&snapshot)).resolve(&snapshot, &now());
    assert!(p.project.is_some(), "{:?}", p.diagnostics);
    assert!(!p.has_errors());
}
