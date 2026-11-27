//! A repeating task's streak: how many of its latest occurrences were done on time, counted
//! from its completions, each of which records the occurrence it completed. No target per
//! period: the cadence says that already.
//!
//! Walking back from the newest, an occurrence is kept when a completion names it and came on
//! or before its day. It is missed when its day has gone with none, when one came later than
//! its day, or, for a rule counted from its schedule, when the rule had an occurrence between
//! two that were done which nothing names: the date moved past it. A rule counted from
//! completion (`every!`) has no such gaps, each occurrence counting from the last done.

use std::collections::BTreeMap;

use jiff::{Zoned, civil};

use crate::model::Task;
use crate::recur::Rule;
use crate::snapshot::Snapshot;

/// How far back a streak is counted.
const LONGEST: usize = 1_000;

/// A repeating task's streak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Streak {
    /// How many of its latest occurrences were done on time, running.
    pub running: u32,
    /// The latest occurrence missed, if one ended the run.
    pub missed: Option<civil::Date>,
}

/// `task`'s streak at `now`, if it repeats and has been done or missed.
#[must_use]
pub fn streak(snapshot: &Snapshot, task: &Task, now: &Zoned) -> Option<Streak> {
    let due = task.due.as_ref()?;
    let recurrence = due.recurrence.as_ref()?;
    let rule = Rule::parse(&recurrence.rrule).ok();
    let today = now.date();
    let zone = now.time_zone();
    // Each occurrence done, and the first day it was done on.
    let mut done: BTreeMap<civil::Date, civil::Date> = BTreeMap::new();
    for completion in snapshot.completions.values().filter(|c| c.task_id == task.id) {
        let Some(occurrence) = completion.occurrence_date else { continue };
        let on = completion.completed_at.to_zoned(zone.clone()).date();
        let first = done.entry(occurrence).or_insert(on);
        *first = (*first).min(on);
    }
    // The occurrence open now, gone by without being done: missed, however the rest went.
    if due.date < today && !done.contains_key(&due.date) {
        return Some(Streak { running: 0, missed: Some(due.date) });
    }
    let mut running = 0;
    let mut missed = None;
    let mut following = due.date;
    let until = following;
    for (occurrence, on) in done.iter().rev().filter(|(occurrence, _)| **occurrence < until).take(LONGEST) {
        // Between this one and the next done, the schedule's occurrences nothing names.
        if !recurrence.from_completion
            && let Some(skipped) = rule.as_ref().and_then(|rule| last_between(rule, *occurrence, following))
        {
            missed = Some(skipped);
            break;
        }
        if on > occurrence {
            missed = Some(*occurrence);
            break;
        }
        running += 1;
        following = *occurrence;
    }
    (running > 0 || missed.is_some()).then_some(Streak { running, missed })
}

/// The last occurrence of `rule` after `from` and before `until`, anchored at `from`.
fn last_between(rule: &Rule, from: civil::Date, until: civil::Date) -> Option<civil::Date> {
    let mut last = None;
    let mut at = from;
    for _ in 0..LONGEST {
        match rule.next_after(from, at) {
            Ok(Some(next)) if next < until => {
                last = Some(next);
                at = next;
            }
            _ => break,
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Due, Project, Recurrence, TaskCompletion};
    use crate::order::OrderKey;

    fn now() -> Zoned {
        "2026-11-27T12:00:00[UTC]".parse().unwrap()
    }

    fn date(text: &str) -> civil::Date {
        text.parse().unwrap()
    }

    fn daily(due: &str, from_completion: bool) -> (Snapshot, Task) {
        let mut task = Task::new(Project::inbox().id, "Stretch", OrderKey::middle());
        task.due = Some(Due {
            recurrence: Some(Recurrence { rrule: "FREQ=DAILY".to_owned(), from_completion }),
            ..Due::on(date(due))
        });
        let mut snapshot = Snapshot::default();
        snapshot.tasks.insert(task.id, task.clone());
        (snapshot, task)
    }

    fn done(snapshot: &mut Snapshot, task: &Task, occurrence: &str, on: &str) {
        let mut completion = TaskCompletion::new(task.id, Some(date(occurrence)));
        completion.completed_at = format!("{on}T09:00:00Z").parse().unwrap();
        snapshot.completions.insert(completion.id, completion);
    }

    #[test]
    fn occurrences_done_on_their_day_run_on() {
        let (mut snapshot, task) = daily("2026-11-27", false);
        for day in ["2026-11-24", "2026-11-25", "2026-11-26"] {
            done(&mut snapshot, &task, day, day);
        }
        assert_eq!(streak(&snapshot, &task, &now()), Some(Streak { running: 3, missed: None }));
    }

    #[test]
    fn a_day_gone_by_undone_is_missed() {
        let (mut snapshot, task) = daily("2026-11-24", false);
        done(&mut snapshot, &task, "2026-11-23", "2026-11-23");
        assert_eq!(streak(&snapshot, &task, &now()), Some(Streak { running: 0, missed: Some(date("2026-11-24")) }));
    }

    #[test]
    fn a_date_moved_past_an_occurrence_misses_it_and_a_late_one_ends_the_run() {
        let (mut snapshot, task) = daily("2026-11-27", false);
        done(&mut snapshot, &task, "2026-11-22", "2026-11-22");
        done(&mut snapshot, &task, "2026-11-24", "2026-11-24");
        done(&mut snapshot, &task, "2026-11-25", "2026-11-26");
        done(&mut snapshot, &task, "2026-11-26", "2026-11-26");
        assert_eq!(streak(&snapshot, &task, &now()), Some(Streak { running: 1, missed: Some(date("2026-11-25")) }));
        let (mut snapshot, task) = daily("2026-11-27", false);
        done(&mut snapshot, &task, "2026-11-23", "2026-11-23");
        done(&mut snapshot, &task, "2026-11-26", "2026-11-26");
        assert_eq!(streak(&snapshot, &task, &now()), Some(Streak { running: 1, missed: Some(date("2026-11-25")) }));
    }

    #[test]
    fn a_rule_counted_from_completion_has_no_gaps() {
        let (mut snapshot, task) = daily("2026-11-28", true);
        done(&mut snapshot, &task, "2026-11-20", "2026-11-20");
        done(&mut snapshot, &task, "2026-11-27", "2026-11-27");
        assert_eq!(streak(&snapshot, &task, &now()), Some(Streak { running: 2, missed: None }));
    }

    #[test]
    fn a_task_that_does_not_repeat_or_was_never_done_has_none() {
        let (snapshot, mut task) = daily("2026-11-27", false);
        assert_eq!(streak(&snapshot, &task, &now()), None);
        task.due = Some(Due::on(date("2026-11-20")));
        assert_eq!(streak(&snapshot, &task, &now()), None);
    }
}
