//! §5's two recurrence systems, which §18 calls the place where getting the model wrong is
//! most expensive to correct later.
//!
//! Expensive because it is not just a wrong answer today: `BlockException` is keyed by the
//! date the *rule* produced (§3.6), so a change in what the rule produces orphans every
//! exception written against the old answer. These tests pin the behaviour before anything
//! stores a date derived from it.

use std::collections::BTreeMap;

use jiff::civil::{date, time};
use lumenna_core::model::{
    BlockException, BlockKind, BlockRef, BlockSeries, Due, ExceptionAction, Recurrence,
};
use lumenna_core::recur::{Advanced, RecurError, Rule, advance, expand, expand_all};

fn rule(text: &str) -> Rule {
    Rule::parse(text).unwrap()
}

fn due_every(text: &str, on: jiff::civil::Date, from_completion: bool) -> Due {
    Due {
        recurrence: Some(Recurrence { rrule: text.to_owned(), from_completion }),
        ..Due::on(on)
    }
}

// ---------------------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------------------

#[test]
fn ordinary_rules_parse() {
    for text in [
        "FREQ=DAILY",
        "FREQ=DAILY;INTERVAL=3",
        "FREQ=WEEKLY;BYDAY=MO,WE,FR",
        "FREQ=MONTHLY;BYMONTHDAY=15",
        "FREQ=MONTHLY;BYMONTHDAY=-1",
        "FREQ=MONTHLY;BYDAY=3FR",
        "FREQ=YEARLY",
        "FREQ=WEEKLY;COUNT=10",
        "FREQ=DAILY;UNTIL=20261231T000000Z",
    ] {
        assert!(Rule::parse(text).is_ok(), "{text}");
    }
}

#[test]
fn nonsense_is_rejected_rather_than_guessed_at() {
    assert!(matches!(Rule::parse("every tuesday"), Err(RecurError::Invalid { .. })));
    assert!(matches!(Rule::parse(""), Err(RecurError::Invalid { .. })));
    assert!(matches!(Rule::parse("FREQ=FORTNIGHTLY"), Err(RecurError::Invalid { .. })));
}

#[test]
fn sub_daily_frequencies_are_refused() {
    // Not merely unsupported: a block has one start time, so an hourly rule would expand to
    // the same date twenty-four times and the duplicates would vanish silently.
    for (text, name) in [
        ("FREQ=HOURLY", "hourly"),
        ("FREQ=MINUTELY", "minutely"),
        ("FREQ=SECONDLY", "secondly"),
    ] {
        match Rule::parse(text) {
            Err(RecurError::SubDaily { frequency }) => assert_eq!(frequency, name),
            other => panic!("{text} gave {other:?}"),
        }
    }
}

#[test]
fn a_rule_knows_whether_it_ever_stops() {
    assert!(!rule("FREQ=DAILY").is_bounded());
    assert!(rule("FREQ=DAILY;COUNT=5").is_bounded());
    assert!(rule("FREQ=DAILY;UNTIL=20261231T000000Z").is_bounded());
    assert_eq!(rule("FREQ=DAILY;COUNT=5").count(), Some(5));
    assert_eq!(rule("FREQ=DAILY").count(), None);
}

// ---------------------------------------------------------------------------------------
// Expansion arithmetic
// ---------------------------------------------------------------------------------------

#[test]
fn the_next_occurrence_is_strictly_after_the_one_asked_about() {
    let anchor = date(2026, 1, 1);
    let r = rule("FREQ=DAILY");
    assert_eq!(r.next_after(anchor, anchor).unwrap(), Some(date(2026, 1, 2)));
}

#[test]
fn interval_counts_from_the_anchor() {
    let anchor = date(2026, 1, 1);
    let r = rule("FREQ=DAILY;INTERVAL=3");
    assert_eq!(r.next_after(anchor, anchor).unwrap(), Some(date(2026, 1, 4)));
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 1, 1)..=date(2026, 1, 10))).unwrap(),
        vec![date(2026, 1, 1), date(2026, 1, 4), date(2026, 1, 7), date(2026, 1, 10)]
    );
}

#[test]
fn weekly_by_day_picks_out_the_named_days() {
    // 2026-01-05 is a Monday.
    let anchor = date(2026, 1, 5);
    let r = rule("FREQ=WEEKLY;BYDAY=MO,WE,FR");
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 1, 5)..=date(2026, 1, 11))).unwrap(),
        vec![date(2026, 1, 5), date(2026, 1, 7), date(2026, 1, 9)]
    );
}

#[test]
fn monthly_on_the_thirty_first_skips_the_months_that_have_none() {
    let anchor = date(2026, 1, 31);
    let r = rule("FREQ=MONTHLY;BYMONTHDAY=31");
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 1, 1)..=date(2026, 6, 30))).unwrap(),
        vec![date(2026, 1, 31), date(2026, 3, 31), date(2026, 5, 31)],
        "February, April and June are skipped, not clamped to the 28th or 30th"
    );
}

#[test]
fn monthly_on_the_last_day_follows_the_month_length() {
    let anchor = date(2026, 1, 31);
    let r = rule("FREQ=MONTHLY;BYMONTHDAY=-1");
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 1, 1)..=date(2026, 4, 30))).unwrap(),
        vec![date(2026, 1, 31), date(2026, 2, 28), date(2026, 3, 31), date(2026, 4, 30)]
    );
}

#[test]
fn the_third_friday_of_the_month_is_expressible() {
    let anchor = date(2026, 1, 16);
    let r = rule("FREQ=MONTHLY;BYDAY=3FR");
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 1, 1)..=date(2026, 3, 31))).unwrap(),
        vec![date(2026, 1, 16), date(2026, 2, 20), date(2026, 3, 20)]
    );
}

#[test]
fn a_yearly_rule_anchored_on_a_leap_day_only_lands_on_leap_days() {
    let anchor = date(2024, 2, 29);
    let r = rule("FREQ=YEARLY");
    assert_eq!(r.next_after(anchor, anchor).unwrap(), Some(date(2028, 2, 29)));
}

#[test]
fn until_ends_the_series() {
    let anchor = date(2026, 1, 1);
    let r = rule("FREQ=DAILY;UNTIL=20260103T000000Z");
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 1, 1)..=date(2026, 1, 31))).unwrap(),
        vec![date(2026, 1, 1), date(2026, 1, 2), date(2026, 1, 3)]
    );
    assert_eq!(r.next_after(anchor, date(2026, 1, 3)).unwrap(), None);
}

#[test]
fn an_anchor_that_does_not_match_the_rule_is_not_an_occurrence() {
    // RFC 5545: the first occurrence is the first date matching the rule *on or after*
    // DTSTART, not DTSTART itself. So a weekly-Monday block whose start date is a Tuesday
    // never occurs on its own start date.
    //
    // This is correct, and it is a trap for the block editor rather than for this module:
    // "every Monday, starting Tuesday the 6th" is almost certainly not what the user meant,
    // and §16 should normalise the start date to the first real occurrence rather than
    // storing a series whose start date is a day it never happens.
    let tuesday = date(2026, 1, 6);
    let r = rule("FREQ=WEEKLY;BYDAY=MO");
    assert_eq!(
        r.occurrences(tuesday, &(tuesday..=date(2026, 1, 20))).unwrap(),
        vec![date(2026, 1, 12), date(2026, 1, 19)]
    );
    assert_eq!(r.next_after(tuesday, tuesday).unwrap(), Some(date(2026, 1, 12)));

    // Anchored on a Monday, the anchor is the first occurrence.
    let monday = date(2026, 1, 5);
    assert_eq!(
        r.occurrences(monday, &(monday..=date(2026, 1, 20))).unwrap(),
        vec![date(2026, 1, 5), date(2026, 1, 12), date(2026, 1, 19)]
    );
}

#[test]
fn the_first_occurrence_of_a_matching_anchor_is_the_anchor() {
    for (text, anchor) in [
        ("FREQ=DAILY", date(2026, 1, 1)),
        ("FREQ=WEEKLY;BYDAY=MO", date(2026, 1, 5)),
        ("FREQ=MONTHLY;BYMONTHDAY=15", date(2026, 1, 15)),
        ("FREQ=MONTHLY;BYMONTHDAY=-1", date(2026, 1, 31)),
    ] {
        let found = rule(text)
            .occurrences(anchor, &(anchor..=anchor.checked_add(jiff::Span::new().days(1)).unwrap()))
            .unwrap();
        assert_eq!(found.first().copied(), Some(anchor), "{text}");
    }
}

#[test]
fn a_range_before_the_anchor_yields_nothing() {
    // An RRULE's first occurrence is its anchor; there is no history before it.
    let anchor = date(2026, 6, 1);
    let r = rule("FREQ=DAILY");
    assert!(r.occurrences(anchor, &(date(2026, 1, 1)..=date(2026, 5, 31))).unwrap().is_empty());
}

#[test]
fn daily_recurrence_crosses_a_dst_transition_without_losing_a_day() {
    // 2026-03-08 is when the US springs forward, and 02:30 does not exist that morning in
    // America/New_York. Expansion is a civil-calendar question and runs in UTC, so the day
    // is still there — what to *do* about the missing hour is a separate decision, taken
    // where an occurrence is resolved against a real zone.
    let anchor = date(2026, 3, 6);
    let r = rule("FREQ=DAILY");
    assert_eq!(
        r.occurrences(anchor, &(date(2026, 3, 6)..=date(2026, 3, 10))).unwrap(),
        vec![
            date(2026, 3, 6),
            date(2026, 3, 7),
            date(2026, 3, 8),
            date(2026, 3, 9),
            date(2026, 3, 10)
        ]
    );

    // And when the clocks go back, no day is doubled.
    let autumn = date(2026, 10, 30);
    assert_eq!(
        r.occurrences(autumn, &(date(2026, 10, 31)..=date(2026, 11, 2))).unwrap(),
        vec![date(2026, 10, 31), date(2026, 11, 1), date(2026, 11, 2)]
    );
}

// ---------------------------------------------------------------------------------------
// Recurring tasks (§5's first system)
// ---------------------------------------------------------------------------------------

#[test]
fn a_task_that_does_not_repeat_is_simply_finished() {
    let due = Due::on(date(2026, 1, 1));
    assert_eq!(advance(&due, date(2026, 1, 1), 1).unwrap(), Advanced::NotRecurring);
}

#[test]
fn scheduled_anchoring_can_fall_behind_and_that_is_the_point() {
    // "every 3 days", due the 1st, finished on the 5th. It comes back on the 4th — in the
    // past, and overdue. For a bill or a medication the schedule is the point.
    let due = due_every("FREQ=DAILY;INTERVAL=3", date(2026, 1, 1), false);
    let Advanced::Next(next) = advance(&due, date(2026, 1, 5), 1).unwrap() else {
        panic!("should recur");
    };
    assert_eq!(next.date, date(2026, 1, 4));
    assert!(next.date < date(2026, 1, 5), "it is overdue the moment it comes back");
}

#[test]
fn completion_anchoring_starts_the_interval_over() {
    // The same rule with `every!`: finished on the 5th, back on the 8th.
    let due = due_every("FREQ=DAILY;INTERVAL=3", date(2026, 1, 1), true);
    let Advanced::Next(next) = advance(&due, date(2026, 1, 5), 1).unwrap() else {
        panic!("should recur");
    };
    assert_eq!(next.date, date(2026, 1, 8));
}

#[test]
fn completion_anchoring_respects_the_days_a_rule_names() {
    // "every! monday", finished on a Wednesday, comes back the following Monday — not
    // seven days after Wednesday.
    let due = due_every("FREQ=WEEKLY;BYDAY=MO", date(2026, 1, 5), true);
    let Advanced::Next(next) = advance(&due, date(2026, 1, 7), 1).unwrap() else {
        panic!("should recur");
    };
    assert_eq!(next.date, date(2026, 1, 12));

    // ...and completing it *on* a Monday moves it to the next one, not to that same day.
    let Advanced::Next(next) = advance(&due, date(2026, 1, 12), 1).unwrap() else {
        panic!("should recur");
    };
    assert_eq!(next.date, date(2026, 1, 19));
}

#[test]
fn the_time_and_zone_survive_the_advance() {
    let due = Due {
        time: Some(time(15, 30, 0, 0)),
        timezone: Some(lumenna_core::model::TzName::new("America/New_York")),
        recurrence: Some(Recurrence { rrule: "FREQ=DAILY".to_owned(), from_completion: false }),
        ..Due::on(date(2026, 1, 1))
    };
    let Advanced::Next(next) = advance(&due, date(2026, 1, 1), 1).unwrap() else {
        panic!("should recur");
    };
    assert_eq!(next.date, date(2026, 1, 2));
    assert_eq!(next.time, due.time);
    assert_eq!(next.timezone, due.timezone);
    assert_eq!(next.recurrence, due.recurrence);
}

#[test]
fn count_is_tracked_by_completions_because_the_rule_is_re_anchored() {
    // The model keeps only the current due date, so COUNT cannot be read off the rule after
    // the first advance. The caller supplies how many completions there have been.
    let due = due_every("FREQ=DAILY;COUNT=3", date(2026, 1, 1), false);
    assert!(matches!(advance(&due, date(2026, 1, 1), 1).unwrap(), Advanced::Next(_)));
    assert!(matches!(advance(&due, date(2026, 1, 2), 2).unwrap(), Advanced::Next(_)));
    assert_eq!(advance(&due, date(2026, 1, 3), 3).unwrap(), Advanced::Finished);
    assert_eq!(advance(&due, date(2026, 1, 4), 9).unwrap(), Advanced::Finished);
}

#[test]
fn until_finishes_a_task_for_good() {
    let due = due_every("FREQ=DAILY;UNTIL=20260102T000000Z", date(2026, 1, 2), false);
    assert_eq!(advance(&due, date(2026, 1, 2), 1).unwrap(), Advanced::Finished);
}

#[test]
fn a_broken_rule_surfaces_rather_than_silently_ending_the_task() {
    let due = due_every("FREQ=WHENEVER", date(2026, 1, 1), false);
    assert!(matches!(advance(&due, date(2026, 1, 1), 1), Err(RecurError::Invalid { .. })));
}

// ---------------------------------------------------------------------------------------
// Recurring blocks (§5's second system)
// ---------------------------------------------------------------------------------------

fn work_block(on: jiff::civil::Date, rrule: Option<&str>) -> BlockSeries {
    BlockSeries {
        rrule: rrule.map(ToOwned::to_owned),
        end_date: rrule.map_or_else(|| Some(on), |_| None),
        ..BlockSeries::one_off("Deep work", BlockKind::Work, on, time(9, 0, 0, 0), 90).unwrap()
    }
}

fn no_exceptions(_: jiff::civil::Date) -> Option<&'static ExceptionAction> {
    None
}

#[test]
fn a_block_with_no_rule_happens_once() {
    let series = work_block(date(2026, 5, 4), None);
    let found = expand(&series, no_exceptions, &(date(2026, 5, 1)..=date(2026, 5, 31))).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].date, date(2026, 5, 4));
    assert_eq!(found[0].start_time, time(9, 0, 0, 0));
    assert_eq!(found[0].duration_mins, 90);
    assert!(!found[0].modified);

    // ...and not at all outside its day.
    assert!(
        expand(&series, no_exceptions, &(date(2026, 6, 1)..=date(2026, 6, 30))).unwrap().is_empty()
    );
}

#[test]
fn a_recurring_block_expands_across_the_window_only() {
    let series = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    let found = expand(&series, no_exceptions, &(date(2026, 5, 6)..=date(2026, 5, 8))).unwrap();
    assert_eq!(
        found.iter().map(|o| o.date).collect::<Vec<_>>(),
        vec![date(2026, 5, 6), date(2026, 5, 7), date(2026, 5, 8)]
    );
}

#[test]
fn the_series_end_date_bounds_the_rule_independently() {
    let mut series = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    series.end_date = Some(date(2026, 5, 6));
    let found = expand(&series, no_exceptions, &(date(2026, 5, 1)..=date(2026, 5, 31))).unwrap();
    assert_eq!(
        found.iter().map(|o| o.date).collect::<Vec<_>>(),
        vec![date(2026, 5, 4), date(2026, 5, 5), date(2026, 5, 6)]
    );
}

#[test]
fn a_cancelled_occurrence_disappears_and_the_rest_do_not() {
    let series = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    let cancelled = ExceptionAction::Cancelled;
    let found = expand(
        &series,
        |d| (d == date(2026, 5, 5)).then_some(&cancelled),
        &(date(2026, 5, 4)..=date(2026, 5, 6)),
    )
    .unwrap();
    assert_eq!(
        found.iter().map(|o| o.date).collect::<Vec<_>>(),
        vec![date(2026, 5, 4), date(2026, 5, 6)]
    );
}

#[test]
fn a_modified_occurrence_carries_its_overrides_and_says_so() {
    // "I finished at 10:40 instead of 11:00" is an edit to the occurrence, not a lifecycle
    // transition (§3.6).
    let series = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    let shortened = ExceptionAction::Modified {
        start_time: Some(time(10, 0, 0, 0)),
        duration_mins: Some(40),
        title: Some("Deep work (short)".to_owned()),
        kind: None,
        flags: None,
    };
    let found = expand(
        &series,
        |d| (d == date(2026, 5, 5)).then_some(&shortened),
        &(date(2026, 5, 4)..=date(2026, 5, 5)),
    )
    .unwrap();

    assert!(!found[0].modified);
    assert_eq!(found[0].duration_mins, 90);

    assert!(found[1].modified);
    assert_eq!(found[1].start_time, time(10, 0, 0, 0));
    assert_eq!(found[1].duration_mins, 40);
    assert_eq!(found[1].title, "Deep work (short)");
    assert_eq!(found[1].kind, BlockKind::Work, "an unset override keeps the series' value");
    assert_eq!(found[1].flags, series.flags);
    assert_eq!(found[1].date, date(2026, 5, 5), "keyed by the date the rule produced");
}

#[test]
fn a_shortened_break_stays_incompressible() {
    // The floor comes from the effective kind, so an exception that shortens a break does
    // not quietly make it compressible on top (§3.6).
    let series = BlockSeries {
        rrule: Some("FREQ=DAILY".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Lunch", BlockKind::Break, date(2026, 5, 4), time(12, 0, 0, 0), 45)
            .unwrap()
    };
    let shorter = ExceptionAction::Modified {
        start_time: None,
        duration_mins: Some(20),
        title: None,
        kind: None,
        flags: None,
    };
    let found = expand(
        &series,
        |d| (d == date(2026, 5, 5)).then_some(&shorter),
        &(date(2026, 5, 4)..=date(2026, 5, 5)),
    )
    .unwrap();
    assert_eq!(found[0].min_duration_mins, 45);
    assert_eq!(found[1].min_duration_mins, 20, "still exactly as long as it is");
}

#[test]
fn occurrences_know_how_an_assignment_should_name_them() {
    let recurring = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    let once = work_block(date(2026, 5, 4), None);

    let r = expand(&recurring, no_exceptions, &(date(2026, 5, 4)..=date(2026, 5, 4))).unwrap();
    assert_eq!(r[0].block_ref(&recurring), BlockRef::Occurrence(recurring.id, date(2026, 5, 4)));

    let o = expand(&once, no_exceptions, &(date(2026, 5, 4)..=date(2026, 5, 4))).unwrap();
    assert_eq!(o[0].block_ref(&once), BlockRef::OneOff(once.id));
}

#[test]
fn an_occurrence_knows_when_it_ends() {
    let series = work_block(date(2026, 5, 4), None);
    let found = expand(&series, no_exceptions, &(date(2026, 5, 4)..=date(2026, 5, 4))).unwrap();
    assert_eq!(found[0].end_time(), time(10, 30, 0, 0));
}

#[test]
fn a_day_comes_out_in_the_order_it_is_lived() {
    let mut series = BTreeMap::new();
    let lunch = BlockSeries {
        rrule: Some("FREQ=DAILY".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Lunch", BlockKind::Break, date(2026, 5, 1), time(12, 0, 0, 0), 45)
            .unwrap()
    };
    let standup = BlockSeries {
        rrule: Some("FREQ=WEEKLY;BYDAY=MO".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Standup", BlockKind::Event, date(2026, 5, 4), time(9, 30, 0, 0), 15)
            .unwrap()
    };
    let work = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    let deleted = BlockSeries {
        deleted_at: Some(lumenna_core::time::now()),
        rrule: Some("FREQ=DAILY".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Gone", BlockKind::Work, date(2026, 5, 1), time(8, 0, 0, 0), 30)
            .unwrap()
    };
    for s in [&lunch, &standup, &work, &deleted] {
        series.insert(s.id, s.clone());
    }

    let monday = date(2026, 5, 4);
    let found = expand_all(&series, &BTreeMap::new(), &(monday..=monday)).unwrap();
    assert_eq!(
        found.iter().map(|o| (o.start_time, o.title.as_str())).collect::<Vec<_>>(),
        vec![
            (time(9, 0, 0, 0), "Deep work"),
            (time(9, 30, 0, 0), "Standup"),
            (time(12, 0, 0, 0), "Lunch"),
        ],
        "the deleted series is not in a day view"
    );
}

#[test]
fn expand_all_applies_each_series_own_exceptions() {
    let a = work_block(date(2026, 5, 4), Some("FREQ=DAILY"));
    let b = BlockSeries {
        rrule: Some("FREQ=DAILY".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Other", BlockKind::Work, date(2026, 5, 4), time(14, 0, 0, 0), 60)
            .unwrap()
    };
    let series: BTreeMap<_, _> = [(a.id, a.clone()), (b.id, b.clone())].into_iter().collect();

    // Cancelling one series' Tuesday must not touch the other's.
    let mut exceptions = BTreeMap::new();
    exceptions.insert(
        (a.id, date(2026, 5, 5)),
        BlockException {
            series_id: a.id,
            original_date: date(2026, 5, 5),
            action: ExceptionAction::Cancelled,
        },
    );

    let tuesday = date(2026, 5, 5);
    let found = expand_all(&series, &exceptions, &(tuesday..=tuesday)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].series_id, b.id);
}

#[test]
fn one_bad_rule_fails_the_range_rather_than_hiding_a_block() {
    let broken = BlockSeries {
        rrule: Some("FREQ=NONSENSE".to_owned()),
        end_date: None,
        ..BlockSeries::one_off("Broken", BlockKind::Work, date(2026, 5, 4), time(9, 0, 0, 0), 60)
            .unwrap()
    };
    let series: BTreeMap<_, _> = [(broken.id, broken)].into_iter().collect();
    let day = date(2026, 5, 4);
    assert!(expand_all(&series, &BTreeMap::new(), &(day..=day)).is_err());
}

// ---------------------------------------------------------------------------------------
// Properties that must hold for any rule, over any window
// ---------------------------------------------------------------------------------------

use proptest::prelude::*;

/// Rules a person might plausibly produce, plus the awkward ones.
const RULES: &[&str] = &[
    "FREQ=DAILY",
    "FREQ=DAILY;INTERVAL=2",
    "FREQ=DAILY;INTERVAL=7",
    "FREQ=WEEKLY",
    "FREQ=WEEKLY;BYDAY=MO,WE,FR",
    "FREQ=WEEKLY;BYDAY=SA,SU;INTERVAL=2",
    "FREQ=MONTHLY",
    "FREQ=MONTHLY;BYMONTHDAY=1",
    "FREQ=MONTHLY;BYMONTHDAY=31",
    "FREQ=MONTHLY;BYMONTHDAY=-1",
    "FREQ=MONTHLY;BYDAY=1MO",
    "FREQ=MONTHLY;BYDAY=-1FR",
    "FREQ=YEARLY",
    "FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29",
];

fn any_date() -> impl Strategy<Value = jiff::civil::Date> {
    (2024i16..2030, 1i8..13, 1i8..29).prop_map(|(y, m, d)| date(y, m, d))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Occurrences come out ascending, distinct, and inside the window that was asked for.
    ///
    /// Distinctness is the one worth stating: expansion runs at midnight UTC and truncates
    /// to a date, so any rule that produced two datetimes on one day would collapse into a
    /// duplicate. Sub-daily frequencies are refused for exactly this reason, and this says
    /// nothing else sneaks past.
    #[test]
    fn expansion_stays_in_bounds_and_in_order(
        index in 0..RULES.len(),
        anchor in any_date(),
        span in 1i64..800,
    ) {
        let r = rule(RULES[index]);
        let end = anchor.checked_add(jiff::Span::new().days(span)).unwrap();
        let dates = r.occurrences(anchor, &(anchor..=end)).unwrap();

        prop_assert!(dates.windows(2).all(|w| w[0] < w[1]), "{dates:?}");
        prop_assert!(dates.iter().all(|d| *d >= anchor && *d <= end));
    }

    /// A narrower window is a subset of a wider one. Nothing appears only when you happen
    /// to look at a smaller range — which is what a day view is.
    #[test]
    fn a_narrow_window_agrees_with_a_wide_one(
        index in 0..RULES.len(),
        anchor in any_date(),
        offset in 0i64..200,
        span in 0i64..60,
    ) {
        let r = rule(RULES[index]);
        let wide_end = anchor.checked_add(jiff::Span::new().days(400)).unwrap();
        let wide = r.occurrences(anchor, &(anchor..=wide_end)).unwrap();

        let from = anchor.checked_add(jiff::Span::new().days(offset)).unwrap();
        let to = from.checked_add(jiff::Span::new().days(span)).unwrap();
        if to > wide_end {
            return Ok(());
        }
        let narrow = r.occurrences(anchor, &(from..=to)).unwrap();

        let expected: Vec<_> = wide.iter().copied().filter(|d| *d >= from && *d <= to).collect();
        prop_assert_eq!(narrow, expected);
    }

    /// The next occurrence is always strictly later, and is the one expansion would give.
    /// Advancing a recurring task can never stand still — an equal date would loop forever.
    #[test]
    fn advancing_always_moves_forward(
        index in 0..RULES.len(),
        anchor in any_date(),
    ) {
        let r = rule(RULES[index]);
        let next = r.next_after(anchor, anchor).unwrap().expect("unbounded rules never run out");
        prop_assert!(next > anchor);

        let end = anchor.checked_add(jiff::Span::new().days(1500)).unwrap();
        let expanded = r.occurrences(anchor, &(anchor..=end)).unwrap();
        prop_assert_eq!(expanded.iter().copied().find(|d| *d > anchor), Some(next));
    }

    /// Repeatedly completing a recurring task walks its sequence and never repeats a date.
    #[test]
    fn a_recurring_task_walks_its_own_sequence(
        index in 0..RULES.len(),
        anchor in any_date(),
        from_completion in any::<bool>(),
    ) {
        let mut due = due_every(RULES[index], anchor, from_completion);
        let mut seen = vec![due.date];
        for step in 1..12u32 {
            // Finished on the day it was due, so both anchoring modes walk the same
            // sequence and any divergence is a bug rather than a scenario.
            let Advanced::Next(next) = advance(&due, due.date, step).unwrap() else {
                panic!("an unbounded rule should not finish");
            };
            prop_assert!(next.date > due.date, "{:?} -> {:?}", due.date, next.date);
            seen.push(next.date);
            due = next;
        }
        let mut sorted = seen.clone();
        sorted.dedup();
        prop_assert_eq!(sorted.len(), seen.len());
    }
}
