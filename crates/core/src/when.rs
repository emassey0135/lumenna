//! Which of a task's dates says what: when it may be begun, when it is meant to be done, and when
//! it must be done by.
//!
//! A task has three dates, as Things (When and Deadline), OmniFocus (defer and due) and
//! Taskwarrior (`wait` and `due`) keep apart: its **start** ([`Task::start`]), before which it is
//! kept out of the listings and the planner, or **Someday**, kept but never offered until it is
//! given a date; its **due date**, the day it is meant to be done, as it always was; and its
//! **deadline** ([`Task::deadline`]), the day it can wait no longer. Every rule about which date a
//! listing, the urgency or the planner reads is here, so none is decided twice.

use jiff::civil::Date;

use crate::model::{Start, Task};

/// Whether `task` has been put off to Someday.
#[must_use]
pub fn is_someday(task: &Task) -> bool {
    task.start == Some(Start::Someday)
}

/// Whether `task`'s start is still to come on `day`: kept out until then.
#[must_use]
pub fn starts_later(task: &Task, day: Date) -> bool {
    task.start.and_then(Start::date).is_some_and(|start| start > day)
}

/// Whether `task` may be offered on `day`: by the planner, What Should I Work On? and every
/// listing that does not ask for it. Not before its start, and never while it is Someday.
#[must_use]
pub fn offered_on(task: &Task, day: Date) -> bool {
    !is_someday(task) && !starts_later(task, day)
}

/// The last day `task` can be done on: its deadline or its due date, whichever comes first. What
/// the planner spreads its sittings to and what it names as due by a day.
#[must_use]
pub fn done_by(task: &Task) -> Option<Date> {
    let due = task.due.as_ref().map(|due| due.date);
    match (due, task.deadline) {
        (Some(due), Some(deadline)) => Some(due.min(deadline)),
        (one, other) => one.or(other),
    }
}

/// Whether `task`'s deadline has passed by `day`: overdue, as a due date passed is.
#[must_use]
pub fn past_deadline(task: &Task, day: Date) -> bool {
    task.deadline.is_some_and(|deadline| deadline < day)
}

/// Whether `task` falls on `day` in a view of the days ahead: due, starting or with its deadline
/// then.
#[must_use]
pub fn falls_on(task: &Task, day: Date) -> bool {
    task.due.as_ref().is_some_and(|due| due.date == day)
        || task.start.and_then(Start::date) == Some(day)
        || task.deadline == Some(day)
}

/// `after` as an edit leaves it: a task given a date (a start, a due date or a deadline it did not
/// have) while Someday is Someday no longer, since Someday lasts only until it is given a date.
#[must_use]
pub fn given_a_date(before: &Task, mut after: Task) -> Task {
    let dated = (after.due.is_some() && after.due.as_ref().map(|d| d.date) != before.due.as_ref().map(|d| d.date))
        || (after.deadline.is_some() && after.deadline != before.deadline);
    if dated && is_someday(&after) && is_someday(before) {
        after.start = None;
    }
    after
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::model::{Due, Project};
    use crate::order::OrderKey;

    fn task() -> Task {
        Task::new(Project::inbox().id, "essay", OrderKey::middle())
    }

    #[test]
    fn a_task_is_offered_from_its_start_and_never_while_someday() {
        let mut t = task();
        assert!(offered_on(&t, date(2026, 12, 1)));
        t.start = Some(Start::On(date(2026, 12, 3)));
        assert!(!offered_on(&t, date(2026, 12, 2)));
        assert!(offered_on(&t, date(2026, 12, 3)));
        t.start = Some(Start::Someday);
        assert!(!offered_on(&t, date(2027, 12, 3)));
    }

    #[test]
    fn a_task_is_done_by_the_first_of_its_due_date_and_deadline() {
        let mut t = task();
        assert_eq!(done_by(&t), None);
        t.deadline = Some(date(2026, 12, 4));
        assert_eq!(done_by(&t), Some(date(2026, 12, 4)));
        t.due = Some(Due::on(date(2026, 12, 2)));
        assert_eq!(done_by(&t), Some(date(2026, 12, 2)));
        t.due = Some(Due::on(date(2026, 12, 9)));
        assert_eq!(done_by(&t), Some(date(2026, 12, 4)));
    }

    #[test]
    fn a_someday_task_given_a_due_date_is_someday_no_longer() {
        let mut before = task();
        before.start = Some(Start::Someday);
        let mut after = before.clone();
        after.due = Some(Due::on(date(2026, 12, 2)));
        assert_eq!(given_a_date(&before, after).start, None);
        let mut after = before.clone();
        after.title = "the essay".to_owned();
        assert_eq!(given_a_date(&before, after).start, Some(Start::Someday));
    }

    #[test]
    fn a_task_put_off_to_someday_with_its_date_stays_someday() {
        let mut before = task();
        before.due = Some(Due::on(date(2026, 12, 2)));
        let mut after = before.clone();
        after.start = Some(Start::Someday);
        after.due = Some(Due::on(date(2026, 12, 9)));
        assert_eq!(given_a_date(&before, after).start, Some(Start::Someday));
    }
}
