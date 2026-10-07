//! A stored repetition read back as words, and those words read again.
//!
//! An edit field shows a task's or a block's repetition as a phrase. Saving the form with
//! that field untouched has to mean what it meant before, so every rule the grammar can write
//! must come back from its phrase unchanged.

use jiff::civil::Weekday;
use lumenna_core::time::{MonthDay, RecurrenceSpec};
use lumenna_parse::date::parse_recurrence;
use lumenna_parse::words;

fn every_spec_the_grammar_writes() -> Vec<RecurrenceSpec> {
    use RecurrenceSpec::{Daily, Monthly, Weekdays, Weekly, Yearly};
    let mut specs = vec![Weekdays];
    for interval in [1, 2, 3, 12] {
        specs.push(Daily { interval });
        specs.push(Weekly { interval, days: Vec::new() });
        specs.push(Monthly { interval, day: None });
        specs.push(Yearly { interval });
    }
    for interval in [1, 2] {
        specs.push(Weekly { interval, days: vec![Weekday::Monday] });
        specs.push(Weekly {
            interval,
            days: vec![Weekday::Monday, Weekday::Wednesday, Weekday::Friday],
        });
        specs.push(Weekly { interval, days: vec![Weekday::Saturday, Weekday::Sunday] });
        for day in [1, 2, 3, 11, 12, 13, 21, 22, 23, 31] {
            specs.push(Monthly { interval, day: Some(MonthDay::Nth(day)) });
        }
        specs.push(Monthly { interval, day: Some(MonthDay::Last) });
    }
    specs
}

#[test]
fn every_rule_the_grammar_writes_reads_back_from_its_rrule() {
    for spec in every_spec_the_grammar_writes() {
        assert_eq!(RecurrenceSpec::from_rrule(&spec.to_rrule()), Some(spec.clone()), "{spec:?}");
    }
}

#[test]
fn every_rule_the_grammar_writes_has_a_phrase_that_parses_back_to_it() {
    for spec in every_spec_the_grammar_writes() {
        for from_completion in [false, true] {
            let phrase = spec.phrase(from_completion).expect("the grammar wrote it");
            let text = words(&phrase);
            let (parsed, counted, used) = parse_recurrence(&text, 0).expect(&phrase);
            assert_eq!((parsed, counted), (spec.clone(), from_completion), "{phrase}");
            assert_eq!(used, text.len(), "'{phrase}' left words over");
        }
    }
}

#[test]
fn a_rule_from_outside_is_not_approximated() {
    for rule in [
        "FREQ=WEEKLY;BYDAY=1MO",
        "FREQ=MONTHLY;BYDAY=MO;BYSETPOS=1",
        "FREQ=DAILY;COUNT=5",
        "FREQ=HOURLY",
        "FREQ=YEARLY;BYMONTH=3",
    ] {
        assert_eq!(RecurrenceSpec::from_rrule(rule), None, "{rule}");
    }
}

#[test]
fn what_the_grammar_cannot_say_has_no_phrase() {
    let spec = RecurrenceSpec::Weekly { interval: 3, days: vec![Weekday::Monday] };
    assert_eq!(RecurrenceSpec::from_rrule(&spec.to_rrule()), Some(spec.clone()));
    assert_eq!(spec.phrase(false), None);
}

#[test]
fn a_day_of_the_month_is_read_after_every_way_of_saying_monthly() {
    let fifteenth = RecurrenceSpec::Monthly { interval: 1, day: Some(MonthDay::Nth(15)) };
    for phrase in [
        "every month on the 15th",
        "every month on 15",
        "monthly on the 15th",
        "every 15th of the month",
        "every month on the 15th of the month",
    ] {
        let parsed = words(phrase);
        let (spec, _, used) = parse_recurrence(&parsed, 0).unwrap_or_else(|| panic!("{phrase}"));
        assert_eq!(spec, fifteenth, "{phrase}");
        assert_eq!(used, parsed.len(), "every word of '{phrase}' is taken");
    }
    let last = parse_recurrence(&words("every 2 months on the last day"), 0).unwrap().0;
    assert_eq!(last, RecurrenceSpec::Monthly { interval: 2, day: Some(MonthDay::Last) });
}

#[test]
fn a_day_of_the_month_reads_back_with_its_ordinal() {
    let spec = RecurrenceSpec::Monthly { interval: 1, day: Some(MonthDay::Nth(1)) };
    assert_eq!(spec.describe(), "every month on the 1st");
}
