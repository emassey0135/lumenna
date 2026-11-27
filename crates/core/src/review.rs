//! A second look: the projects due for review, and the tasks the planner asks about.
//!
//! Both are askings, never changes: a project due for review is listed until it is marked
//! reviewed, and a task asked about is listed with its question until it is answered, through
//! the ordinary operations (keeping it, trashing it, making it repeat less often, taking its
//! date away). Nothing here interrupts: the Review place lists them, and a task's details say
//! its question.
//!
//! Two questions about a task, and no bigger formula:
//!
//! - **Stale**: among the most urgent open tasks ([`TOP`]) and with nothing done on it for
//!   [`STALE_DAYS`]: no time logged, no completion, not kept. Permanent furniture at the top
//!   says a task is badly defined or not really wanted.
//! - **Put off**: moved later by hand (`Snapshot::postponed`) or planned in a block and left
//!   ([`crate::history::History::put_off`]) [`PUT_OFF_TIMES`] times since it was last kept.
//!   Repeating, it is offered less often; not, its date can go.

use std::collections::{BTreeMap, BTreeSet};

use jiff::{SignedDuration, Timestamp, Zoned, civil};

use crate::id::{ProjectId, TaskId};
use crate::model::{AssignmentStatus, Project, Recurrence, Task};
use crate::snapshot::Snapshot;
use crate::state::Facts;

/// How many of the most urgent open tasks count as at the top.
pub const TOP: usize = 5;
/// How long a task at the top may go with nothing done on it before it is asked about.
pub const STALE_DAYS: i64 = 21;
/// How often a task is put off before it is asked about, again once kept.
pub const PUT_OFF_TIMES: u32 = 5;
/// How many days apart a project is reviewed unless it says otherwise.
pub const DEFAULT_REVIEW_DAYS: u32 = 7;

/// What the planner asks about a task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconsider {
    /// Among the most urgent, and untouched for `days` days.
    Stale {
        /// Whole days since anything was done on it, or it was made or kept.
        days: i64,
    },
    /// Put off `times` times, all told.
    PutOff {
        /// How often it was put off, all told.
        times: u32,
        /// The same repetition half as often, when it repeats: what "less often" proposes.
        less_often: Option<Recurrence>,
    },
}

/// What a whole snapshot's tasks hold that the questions read, counted in one pass.
#[derive(Debug, Default)]
pub struct Counts {
    /// Sittings on days gone that were never worked, per task.
    left: BTreeMap<TaskId, u32>,
    /// The last moment something was done on each task: time logged, a completion.
    last_done: BTreeMap<TaskId, Timestamp>,
    /// The most urgent open tasks.
    top: BTreeSet<TaskId>,
}

impl Counts {
    /// Every task's counts as of `now`.
    #[must_use]
    pub fn new(snapshot: &Snapshot, facts: &Facts<'_>, now: &Zoned) -> Self {
        let today = now.date();
        let zone = now.time_zone();
        let mut counts = Self::default();
        for sitting in snapshot.assignments.values() {
            let day = snapshot.assignment_date(sitting);
            if sitting.accumulated_mins > 0 || sitting.is_running() {
                // The day it was worked, at its end: nothing finer is kept of a sitting.
                if let Some(at) = day.and_then(|d| d.to_datetime(civil::time(23, 59, 0, 0)).to_zoned(zone.clone()).ok()) {
                    let at = at.timestamp().min(now.timestamp());
                    let last = counts.last_done.entry(sitting.task_id).or_insert(at);
                    *last = (*last).max(at);
                }
            } else if day.is_some_and(|d| d < today)
                && matches!(sitting.status, AssignmentStatus::Planned | AssignmentStatus::Skipped | AssignmentStatus::Deferred)
            {
                *counts.left.entry(sitting.task_id).or_default() += 1;
            }
        }
        for completion in snapshot.completions.values() {
            let last = counts.last_done.entry(completion.task_id).or_insert(completion.completed_at);
            *last = (*last).max(completion.completed_at);
        }
        let coefficients = snapshot.settings.urgency;
        let mut open: Vec<(f32, &Task)> = snapshot
            .tasks
            .values()
            .filter(|t| !t.is_deleted() && !facts.is_completed(t) && !facts.is_blocked(t))
            .map(|t| (crate::urgency::urgency(snapshot, facts, t, now, &coefficients).score, t))
            .collect();
        // Most urgent first, the older first where two score alike, as the lists break ties.
        open.sort_by(|(a, x), (b, y)| b.total_cmp(a).then(x.id.cmp(&y.id)));
        counts.top = open.iter().take(TOP).map(|(_, t)| t.id).collect();
        counts
    }

    /// How often `task` was put off, all told: its date moved later by hand, and sittings left.
    #[must_use]
    pub fn put_off(&self, snapshot: &Snapshot, task: TaskId) -> u32 {
        snapshot.postponed.get(&task).copied().unwrap_or(0) + self.left.get(&task).copied().unwrap_or(0)
    }
}

/// What the planner asks about `task`, if anything: put off too often first, as the more
/// telling, then stale.
#[must_use]
pub fn reconsider(snapshot: &Snapshot, facts: &Facts<'_>, counts: &Counts, task: &Task, now: &Zoned) -> Option<Reconsider> {
    if task.is_deleted() || facts.is_completed(task) {
        return None;
    }
    let times = counts.put_off(snapshot, task.id);
    if times >= task.kept_put_off.unwrap_or(0).saturating_add(PUT_OFF_TIMES) {
        let less_often = task.due.as_ref().and_then(|d| d.recurrence.as_ref()).and_then(less_often);
        return Some(Reconsider::PutOff { times, less_often });
    }
    if counts.top.contains(&task.id) {
        let since = [Some(task.created_at), task.kept_at, counts.last_done.get(&task.id).copied()]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(task.created_at);
        let days = now.timestamp().duration_since(since).as_secs() / 86_400;
        if days >= STALE_DAYS {
            return Some(Reconsider::Stale { days });
        }
    }
    None
}

/// Every task the planner asks about, in the order of the task lists' ranking: most urgent
/// first.
#[must_use]
pub fn to_reconsider(snapshot: &Snapshot, now: &Zoned) -> Vec<(TaskId, Reconsider)> {
    let facts = snapshot.facts();
    let counts = Counts::new(snapshot, &facts, now);
    let coefficients = snapshot.settings.urgency;
    let mut found: Vec<(f32, TaskId, Reconsider)> = snapshot
        .tasks
        .values()
        .filter_map(|task| {
            let asked = reconsider(snapshot, &facts, &counts, task, now)?;
            Some((crate::urgency::urgency(snapshot, &facts, task, now, &coefficients).score, task.id, asked))
        })
        .collect();
    found.sort_by(|(a, x, _), (b, y, _)| b.total_cmp(a).then(x.cmp(y)));
    found.into_iter().map(|(_, id, asked)| (id, asked)).collect()
}

/// The same repetition half as often: its interval doubled, so "every day" becomes "every
/// other day" and "every Monday and Thursday" every other week's. None for a rule that cannot
/// be read part by part.
#[must_use]
pub fn less_often(recurrence: &Recurrence) -> Option<Recurrence> {
    let mut interval = 1_u32;
    let mut parts: Vec<String> = Vec::new();
    for part in recurrence.rrule.split(';').filter(|p| !p.is_empty()) {
        let (key, value) = part.split_once('=')?;
        if key.eq_ignore_ascii_case("INTERVAL") {
            interval = value.parse().ok().filter(|n| *n >= 1)?;
        } else {
            parts.push(part.to_owned());
        }
    }
    let freq = parts.iter().position(|p| p.to_ascii_uppercase().starts_with("FREQ="))?;
    parts.insert(freq + 1, format!("INTERVAL={}", interval.checked_mul(2)?));
    Some(Recurrence { rrule: parts.join(";"), from_completion: recurrence.from_completion })
}

/// How many days apart `project` is reviewed; `None` for never.
#[must_use]
pub fn review_every(project: &Project) -> Option<u32> {
    match project.review_every_days {
        None => Some(DEFAULT_REVIEW_DAYS),
        Some(0) => None,
        Some(days) => Some(days),
    }
}

/// The intervals a project's review is offered at, in days, the usual week first; 0 is never.
pub const INTERVALS: [u32; 6] = [7, 14, 30, 90, 365, 0];

/// A review interval in words: "every week", "every 2 weeks", "every month", "never".
#[must_use]
pub fn interval_words(days: Option<u32>) -> String {
    match days {
        None | Some(0) => "never".to_owned(),
        Some(1) => "every day".to_owned(),
        Some(7) => "every week".to_owned(),
        Some(30 | 31) => "every month".to_owned(),
        Some(365 | 366) => "every year".to_owned(),
        Some(days) if days % 30 == 0 => format!("every {} months", days / 30),
        Some(days) if days % 7 == 0 => format!("every {} weeks", days / 7),
        Some(days) => format!("every {days} days"),
    }
}

/// When `project` is next due for review: its interval after it was last reviewed, or made.
/// None for one never reviewed, the Inbox (which is emptied, not reviewed), or one whose making
/// its identifier does not say.
#[must_use]
pub fn next_review(project: &Project) -> Option<Timestamp> {
    if project.is_inbox {
        return None;
    }
    let every = review_every(project)?;
    let from = project.reviewed_at.or_else(|| made(project.id))?;
    from.checked_add(SignedDuration::from_hours(i64::from(every) * 24)).ok()
}

/// When a project was made, from its identifier: a UUIDv7 carries its moment.
fn made(id: ProjectId) -> Option<Timestamp> {
    let (seconds, nanos) = id.as_uuid().get_timestamp()?.to_unix();
    Timestamp::new(i64::try_from(seconds).ok()?, i32::try_from(nanos).ok()?).ok()
}

/// Whether `project` is due for review at `now`: live, not archived, its review come.
#[must_use]
pub fn due_for_review(project: &Project, now: &Zoned) -> bool {
    project.deleted_at.is_none() && !project.archived && next_review(project).is_some_and(|at| at <= now.timestamp())
}

/// The projects due for review, the longest due first.
#[must_use]
pub fn to_review<'a>(snapshot: &'a Snapshot, now: &Zoned) -> Vec<&'a Project> {
    let mut due: Vec<&Project> = snapshot.projects.values().filter(|p| due_for_review(p, now)).collect();
    due.sort_by_key(|p| (next_review(p), p.id));
    due
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Due, TaskCompletion};
    use crate::order::OrderKey;

    fn now() -> Zoned {
        "2026-11-27T12:00:00[UTC]".parse().unwrap()
    }

    fn days_ago(days: i64) -> Timestamp {
        now().timestamp().checked_sub(SignedDuration::from_hours(days * 24)).unwrap()
    }

    fn world(tasks: &[(&str, i64)]) -> (Snapshot, Vec<TaskId>) {
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let mut ids = Vec::new();
        for (title, age) in tasks {
            let mut task = Task::new(inbox.id, *title, OrderKey::middle());
            task.created_at = days_ago(*age);
            ids.push(task.id);
            snapshot.tasks.insert(task.id, task);
        }
        snapshot.projects.insert(inbox.id, inbox);
        (snapshot, ids)
    }

    fn asked(snapshot: &Snapshot, id: TaskId) -> Option<Reconsider> {
        to_reconsider(snapshot, &now()).into_iter().find(|(t, _)| *t == id).map(|(_, r)| r)
    }

    #[test]
    fn a_task_at_the_top_untouched_for_three_weeks_is_asked_about_and_a_newer_one_is_not() {
        let (snapshot, ids) = world(&[("old", 22), ("new", 3)]);
        assert_eq!(asked(&snapshot, ids[0]), Some(Reconsider::Stale { days: 22 }));
        assert_eq!(asked(&snapshot, ids[1]), None);
    }

    #[test]
    fn a_task_below_the_top_is_never_stale() {
        let titles: Vec<(String, i64)> = (0..TOP + 1).map(|n| (format!("t{n}"), 30)).collect();
        let named: Vec<(&str, i64)> = titles.iter().map(|(t, a)| (t.as_str(), *a)).collect();
        let (mut snapshot, ids) = world(&named);
        for id in &ids[..TOP] {
            snapshot.tasks.get_mut(id).unwrap().priority = crate::model::Priority::P1;
        }
        assert_eq!(asked(&snapshot, ids[TOP]), None, "below the five most urgent");
        assert!(asked(&snapshot, ids[0]).is_some());
    }

    #[test]
    fn keeping_or_finishing_a_stale_task_stops_the_question_for_three_weeks() {
        let (mut snapshot, ids) = world(&[("kept", 40), ("done lately", 40)]);
        snapshot.tasks.get_mut(&ids[0]).unwrap().kept_at = Some(days_ago(20));
        let mut repeat = snapshot.tasks[&ids[1]].clone();
        repeat.due = Some(Due {
            recurrence: Some(Recurrence { rrule: "FREQ=DAILY".to_owned(), from_completion: false }),
            ..Due::on(now().date())
        });
        snapshot.tasks.insert(repeat.id, repeat);
        let mut completion = TaskCompletion::new(ids[1], Some(now().date().yesterday().unwrap()));
        completion.completed_at = days_ago(1);
        snapshot.completions.insert(completion.id, completion);
        assert_eq!(asked(&snapshot, ids[0]), None);
        assert_eq!(asked(&snapshot, ids[1]), None);
    }

    #[test]
    fn a_task_put_off_five_times_is_asked_again_only_five_more_after_it_is_kept() {
        let (mut snapshot, ids) = world(&[("dentist", 2)]);
        snapshot.postponed.insert(ids[0], 5);
        assert_eq!(asked(&snapshot, ids[0]), Some(Reconsider::PutOff { times: 5, less_often: None }));
        snapshot.tasks.get_mut(&ids[0]).unwrap().kept_put_off = Some(5);
        assert_eq!(asked(&snapshot, ids[0]), None);
        snapshot.postponed.insert(ids[0], 10);
        assert!(matches!(asked(&snapshot, ids[0]), Some(Reconsider::PutOff { times: 10, .. })));
    }

    #[test]
    fn less_often_doubles_the_interval_and_keeps_the_rest_of_the_rule() {
        let rule = |text: &str| Recurrence { rrule: text.to_owned(), from_completion: true };
        assert_eq!(less_often(&rule("FREQ=DAILY")).unwrap().rrule, "FREQ=DAILY;INTERVAL=2");
        assert_eq!(less_often(&rule("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO")).unwrap().rrule, "FREQ=WEEKLY;INTERVAL=4;BYDAY=MO");
        assert!(less_often(&rule("FREQ=DAILY")).unwrap().from_completion);
        assert_eq!(less_often(&rule("nonsense")), None);
    }

    #[test]
    fn a_project_is_due_for_review_at_its_interval_after_it_was_reviewed_and_never_the_inbox() {
        let mut project = Project::new("Home", OrderKey::middle());
        project.reviewed_at = Some(days_ago(8));
        assert!(due_for_review(&project, &now()));
        project.review_every_days = Some(14);
        assert!(!due_for_review(&project, &now()));
        project.review_every_days = Some(0);
        assert!(!due_for_review(&project, &now()), "never reviewed");
        assert!(!due_for_review(&Project::inbox(), &now()));
    }

    #[test]
    fn a_project_never_reviewed_is_due_a_week_after_it_was_made() {
        let project = Project::new("New", OrderKey::middle());
        let soon: Zoned = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC);
        assert!(!due_for_review(&project, &soon));
        let later = soon.checked_add(SignedDuration::from_hours(24 * 8)).unwrap();
        assert!(due_for_review(&project, &later));
    }
}
