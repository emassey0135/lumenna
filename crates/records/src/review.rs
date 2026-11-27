//! The review, as a person is told it: the projects due for review and the tasks the planner
//! asks about (`lumenna_core::review`), listed in one place, each with its answer among its
//! actions; the questions in full, and a repeating task's streak in words.
//!
//! An asking reaches the person one way, never interrupting: the Review place lists it, and the
//! task's details say its question beside the same action. Nothing is announced or notified,
//! and every answer is an ordinary operation, undone as any other.

use jiff::{Zoned, civil};
use lumenna_core::model::{Project, Task};
use lumenna_core::review::{self, Reconsider};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;
use lumenna_core::streak::Streak;

use crate::actions::{Action, ActionKind, Choice, Question, Subject};
use crate::error::Result;
use crate::types::Rows;
use crate::words::{count_line, to_u32};

/// What answering the planner's question with Keep It is sent as.
pub const KEEP: &str = "keep";
/// Drop It: the task to the trash.
pub const DROP: &str = "drop";
/// Make It Less Often: a repeating task half as often.
pub const LESS_OFTEN: &str = "less-often";
/// Take Its Date Away: a task put off by its date, without one.
pub const NO_DATE: &str = "no-date";

/// A span of whole days as a sentence says it: "3 weeks", "2 months".
#[must_use]
pub fn span_words(days: i64) -> String {
    let days = usize::try_from(days.max(0)).unwrap_or(usize::MAX);
    match days {
        0..=13 => count_line(days, "day"),
        14..=59 => count_line(days / 7, "week"),
        60..=729 => count_line(days / 30, "month"),
        _ => count_line(days / 365, "year"),
    }
}

/// How long ago something was, in whole days: "today", "yesterday", "9 days ago", "3 weeks ago".
#[must_use]
pub fn ago_words(days: i64) -> String {
    match days {
        ..=0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        days => format!("{} ago", span_words(days)),
    }
}

/// What the planner asks about `task`, as one question in full: "Write report has been near the
/// top for 3 weeks with nothing done on it. Is it real?"
#[must_use]
pub fn question(task: &Task, asked: &Reconsider) -> String {
    match asked {
        Reconsider::Stale { days } => {
            format!("{} has been near the top for {} with nothing done on it. Is it real?", task.title, span_words(*days))
        }
        Reconsider::PutOff { times, less_often } => {
            let what = if less_often.is_some() {
                "Make it less often, or drop it?"
            } else if task.due.is_some() {
                "Take its date away, or drop it?"
            } else {
                "Keep it, or drop it?"
            };
            format!("{} has been put off {}. {what}", task.title, times_words(*times))
        }
    }
}

/// The question in short, for a row's value: "near the top 3 weeks", "put off 5 times".
#[must_use]
pub fn in_short(asked: &Reconsider) -> String {
    match asked {
        Reconsider::Stale { days } => format!("near the top {} with nothing done", span_words(*days)),
        Reconsider::PutOff { times, .. } => format!("put off {}", times_words(*times)),
    }
}

fn times_words(times: u32) -> String {
    match times {
        1 => "once".to_owned(),
        2 => "twice".to_owned(),
        n => format!("{n} times"),
    }
}

/// The repetition `task` would have made less often, in quick add's words where they can say
/// it: "every other day".
fn less_often_words(asked: &Reconsider) -> Option<String> {
    match asked {
        Reconsider::PutOff { less_often: Some(rule), .. } => crate::types::repetition_phrase(&rule.rrule, rule.from_completion),
        _ => None,
    }
}

/// Reconsider: the planner's question about `task`, its answers each a button, in order.
#[must_use]
pub fn reconsider_action(task: &Task, asked: &Reconsider) -> Action {
    let choice = |id: &str, title: String| Choice { id: id.to_owned(), title, ..Choice::default() };
    let mut answers = vec![choice(KEEP, "Keep It".to_owned())];
    let mut said = vec![match asked {
        Reconsider::Stale { .. } => format!("Keep It asks again in {}.", span_words(review::STALE_DAYS)),
        Reconsider::PutOff { .. } => format!("Keep It asks again if it is put off {} more.", times_words(review::PUT_OFF_TIMES)),
    }];
    if let Reconsider::PutOff { less_often: Some(_), .. } = asked {
        let now = task.due.as_ref().and_then(|d| d.recurrence.as_ref()).and_then(|r| crate::types::repetition_phrase(&r.rrule, r.from_completion));
        match (less_often_words(asked), now) {
            (Some(then), Some(now)) => {
                answers.push(choice(LESS_OFTEN, format!("Make It {}", crate::actions::title_case(&then.replacen("every!", "every", 1)))));
                said.push(format!("It would repeat {then} rather than {now}."));
            }
            _ => {
                answers.push(choice(LESS_OFTEN, "Make It Less Often".to_owned()));
                said.push("Make It Less Often repeats it half as often.".to_owned());
            }
        }
    } else if matches!(asked, Reconsider::PutOff { .. }) && task.due.is_some() {
        answers.push(choice(NO_DATE, "Take Its Date Away".to_owned()));
        said.push("Take Its Date Away keeps it with no due date.".to_owned());
    }
    answers.push(choice(DROP, "Drop It".to_owned()));
    said.push("Drop It moves it to the trash, and undo brings it back.".to_owned());
    let mut action = Action::new(ActionKind::Reconsider, Subject::Task, "Reconsider", task.id.to_string());
    action.primary = true;
    action.question = Question::Choose { title: question(task, asked), message: said.join(" "), answers };
    action
}

/// What `task` is asked, and its answer as an action, if the planner asks anything of it now.
#[must_use]
pub fn asked_of(snapshot: &Snapshot, task: &Task, now: &Zoned) -> Option<(String, Action)> {
    let facts = snapshot.facts();
    let counts = review::Counts::new(snapshot, &facts, now);
    let asked = review::reconsider(snapshot, &facts, &counts, task, now)?;
    Some((question(task, &asked), reconsider_action(task, &asked)))
}

/// A repeating task's streak in words: "done 12 times running", "done last time", "missed
/// Tuesday".
#[must_use]
pub fn streak_words(streak: &Streak, today: civil::Date) -> Option<String> {
    match (streak.running, streak.missed) {
        (0, Some(missed)) => Some(format!("missed {}", past_day(missed, today))),
        (0, None) => None,
        (1, _) => Some("done last time".to_owned()),
        (n, _) => Some(format!("done {n} times running")),
    }
}

/// `task`'s streak in words at `now`, if it repeats and has one.
#[must_use]
pub fn streak_of(snapshot: &Snapshot, task: &Task, now: &Zoned) -> Option<String> {
    streak_words(&lumenna_core::streak::streak(snapshot, task, now)?, now.date())
}

/// A day gone by, as a person says it: "yesterday", "Tuesday", "Tuesday 3 November".
fn past_day(date: civil::Date, today: civil::Date) -> String {
    let back = date.until(today).map_or(i32::MAX, |span| span.get_days());
    match back {
        0 => "today".to_owned(),
        2..=6 => lumenna_core::time::weekday_name(date.weekday()).to_owned(),
        _ => lumenna_core::time::day_words(date, today),
    }
}

/// How often `project` is reviewed and when it was last, as its row says it: "last reviewed 9
/// days ago, every week".
#[must_use]
pub fn project_review_words(project: &Project, now: &Zoned) -> String {
    let every = review::interval_words(review::review_every(project));
    match project.reviewed_at {
        Some(at) => format!("last reviewed {}, {every}", ago_words(now.timestamp().duration_since(at).as_secs() / 86_400)),
        None => format!("not reviewed yet, {every}"),
    }
}

/// Mark Reviewed, on a project.
#[must_use]
pub fn mark_reviewed_action(project: &Project) -> Action {
    Action::new(ActionKind::MarkReviewed, Subject::Project, "Mark Reviewed", project.id.to_string())
}

/// Open Project, on a project listed beside other things.
#[must_use]
pub fn open_project_action(project: &Project) -> Action {
    Action::new(ActionKind::OpenProject, Subject::Project, "Open Project", project.id.to_string())
}

/// Review Every, a choice of the intervals ([`review::INTERVALS`]), the one it has said.
#[must_use]
pub fn review_every_action(project: &Project) -> Action {
    let current = review::review_every(project).unwrap_or(0);
    let answers = review::INTERVALS
        .iter()
        .map(|days| {
            let words = review::interval_words(Some(*days));
            let title = crate::words::capitalised(&words);
            Choice {
                id: days.to_string(),
                detail: (*days == current).then(|| "now".to_owned()),
                title: crate::actions::title_case(&title),
                ..Choice::default()
            }
        })
        .collect();
    let mut action = Action::new(ActionKind::ReviewEvery, Subject::Project, "Review Every", project.id.to_string());
    action.question = Question::Choose {
        title: format!("How often should {} be reviewed?", project.name),
        message: format!("It is reviewed {} now. When it is due, it is listed in Review until it is marked reviewed.", review::interval_words(review::review_every(project))),
        answers,
    };
    action
}

/// What the review holds, as its count line and the sidebar's detail say it: "2 projects to
/// review and 1 task to reconsider", "Nothing to review".
#[must_use]
pub fn said(projects: usize, tasks: usize) -> String {
    let capital = |count, noun| crate::words::capitalised(&count_line(count, noun));
    match (projects, tasks) {
        (0, 0) => "Nothing to review".to_owned(),
        (p, 0) => format!("{} to review", capital(p, "project")),
        (0, t) => format!("{} to reconsider", capital(t, "task")),
        (p, t) => format!("{} to review and {} to reconsider", capital(p, "project"), count_line(t, "task")),
    }
}

/// The count line of the review at `now`, without its rows.
#[must_use]
pub fn count(snapshot: &Snapshot, now: &Zoned) -> String {
    said(review::to_review(snapshot, now).len(), review::to_reconsider(snapshot, now).len())
}

/// The Review: the projects due for review, longest due first, then the tasks the planner asks
/// about, most urgent first. Each project's row offers Open Project, Mark Reviewed and its own
/// actions; each task's, Reconsider and its own.
///
/// # Errors
///
/// Never; a `Result` as every listing is.
pub fn review(snapshot: &Snapshot, now: &Zoned) -> Result<Rows> {
    let projects = review::to_review(snapshot, now);
    let tasks = review::to_reconsider(snapshot, now);
    let total = to_u32(projects.len() + tasks.len());
    let rows: Vec<Row> = projects
        .iter()
        .enumerate()
        .map(|(index, project)| Row {
            id: RowId::Project(project.id),
            role: Role::Project,
            depth: 0,
            index: to_u32(index) + 1,
            count: total,
            expanded: None,
            checked: None,
            title: project.name.clone(),
            state: Vec::new(),
            due: None,
            due_time: None,
            value: Some(project_review_words(project, now)),
            hint: None,
            priority: None,
        })
        .collect();
    // Every open task's row, as any list has it, read only when a task is asked about.
    let open = if tasks.is_empty() { Rows::new(&[], "task") } else { crate::lists::tasks(snapshot, "", now)? };
    let mut listed = Rows::new(&rows, "item");
    crate::actions::fill_rows(&mut listed, snapshot);
    for (row, project) in listed.rows.iter_mut().zip(&projects) {
        let mut reviewed = mark_reviewed_action(project);
        reviewed.primary = true;
        let mut actions = vec![open_project_action(project), reviewed];
        actions.extend(std::mem::take(&mut row.actions).into_iter().filter(|a| a.kind != ActionKind::MarkReviewed));
        row.actions = actions;
    }
    for (position, (id, asked)) in tasks.iter().enumerate() {
        let Some(task) = snapshot.tasks.get(id) else { continue };
        let found = open.rows.iter().find(|row| row.id == id.to_string()).cloned();
        let Some(mut row) = found else { continue };
        row.depth = 0;
        row.index = to_u32(projects.len() + position) + 1;
        row.count = total;
        let short = in_short(asked);
        row.value = Some(match row.value.take() {
            Some(value) => format!("{short}, {value}"),
            None => short,
        });
        row.actions.insert(0, reconsider_action(task, asked));
        listed.rows.push(row);
    }
    for (position, row) in listed.rows.iter_mut().enumerate() {
        row.row = to_u32(position + 1);
    }
    listed.count = to_u32(listed.rows.len());
    listed.announcement = said(projects.len(), tasks.len());
    listed.empty = "Nothing to review. A project comes back here when its review is due, and a task when the planner has a question about it.".to_owned();
    Ok(listed)
}

/// A review interval read from words, in days, 0 for never: "every 2 weeks", "month", "14
/// days", "14", "never". What `Review Every` offers is the same, by its choice's `id`.
///
/// # Errors
///
/// If the words say no interval.
pub fn parse_interval(text: &str) -> Result<u32> {
    let lower = text.trim().to_lowercase();
    let words = lower.strip_prefix("every").unwrap_or(&lower).trim();
    if words == "never" || words == "0" {
        return Ok(0);
    }
    let (count, unit) = match words.split_once(' ') {
        Some((count, unit)) => (count.trim(), unit.trim()),
        None if words.chars().all(|c| c.is_ascii_digit()) => (words, "days"),
        None => ("1", words),
    };
    let count: u32 = match count {
        "other" => 2,
        "a" | "an" => 1,
        n => n.parse().map_err(|_| crate::error::LumennaError::new(INTERVAL_HINT))?,
    };
    let per = match unit.trim_end_matches('s') {
        "day" => 1,
        "week" => 7,
        "fortnight" => 14,
        "month" => 30,
        "year" => 365,
        _ => return Err(crate::error::LumennaError::new(INTERVAL_HINT)),
    };
    count.checked_mul(per).filter(|days| *days > 0).ok_or_else(|| crate::error::LumennaError::new(INTERVAL_HINT))
}

/// What a review interval takes.
pub const INTERVAL_HINT: &str = "How often to review it, such as every week, every 2 weeks, every month, or never.";

#[cfg(test)]
mod tests {
    use super::*;
    use lumenna_core::model::{Due, Recurrence};
    use lumenna_core::order::OrderKey;

    fn task(title: &str) -> Task {
        Task::new(Project::inbox().id, title, OrderKey::middle())
    }

    #[test]
    fn a_stale_task_is_asked_whether_it_is_real_in_full() {
        let asked = Reconsider::Stale { days: 22 };
        assert_eq!(question(&task("Write report"), &asked), "Write report has been near the top for 3 weeks with nothing done on it. Is it real?");
        let Question::Choose { answers, message, .. } = reconsider_action(&task("Write report"), &asked).question else { panic!() };
        let titles: Vec<&str> = answers.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles, ["Keep It", "Drop It"]);
        assert_eq!(message, "Keep It asks again in 3 weeks. Drop It moves it to the trash, and undo brings it back.");
    }

    #[test]
    fn a_repeating_task_put_off_is_offered_less_often_in_words_and_a_dated_one_its_date_taken_away() {
        let mut stretch = task("Stretch");
        let daily = Recurrence { rrule: "FREQ=DAILY".to_owned(), from_completion: false };
        stretch.due = Some(Due { recurrence: Some(daily.clone()), ..Due::on("2026-11-27".parse().unwrap()) });
        let asked = Reconsider::PutOff { times: 5, less_often: lumenna_core::review::less_often(&daily) };
        assert_eq!(question(&stretch, &asked), "Stretch has been put off 5 times. Make it less often, or drop it?");
        let Question::Choose { answers, message, .. } = reconsider_action(&stretch, &asked).question else { panic!() };
        assert_eq!(answers.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), ["Keep It", "Make It Every Other Day", "Drop It"]);
        assert!(message.contains("It would repeat every other day rather than every day."), "{message}");
        let mut dentist = task("Call the dentist");
        dentist.due = Some(Due::on("2026-11-27".parse().unwrap()));
        let asked = Reconsider::PutOff { times: 6, less_often: None };
        assert_eq!(question(&dentist, &asked), "Call the dentist has been put off 6 times. Take its date away, or drop it?");
        let Question::Choose { answers, .. } = reconsider_action(&dentist, &asked).question else { panic!() };
        assert_eq!(answers.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(), [KEEP, NO_DATE, DROP]);
    }

    #[test]
    fn a_streak_is_said_as_a_run_or_the_day_missed() {
        let today: civil::Date = "2026-11-27".parse().unwrap();
        let said = |running, missed: Option<&str>| streak_words(&Streak { running, missed: missed.map(|d| d.parse().unwrap()) }, today);
        assert_eq!(said(12, None).as_deref(), Some("done 12 times running"));
        assert_eq!(said(1, Some("2026-11-20")).as_deref(), Some("done last time"));
        assert_eq!(said(0, Some("2026-11-24")).as_deref(), Some("missed Tuesday"));
        assert_eq!(said(0, Some("2026-11-26")).as_deref(), Some("missed yesterday"));
        assert_eq!(said(0, None), None);
    }

    #[test]
    fn a_review_interval_is_read_from_words_and_said_back() {
        for (text, days) in [("every week", 7), ("2 weeks", 14), ("every other week", 14), ("month", 30), ("14", 14), ("never", 0), ("every 3 months", 90)] {
            assert_eq!(parse_interval(text).unwrap(), days, "{text}");
        }
        assert!(parse_interval("sometimes").is_err());
        assert_eq!(lumenna_core::review::interval_words(Some(14)), "every 2 weeks");
        assert_eq!(lumenna_core::review::interval_words(Some(90)), "every 3 months");
    }

    #[test]
    fn the_review_lists_projects_due_then_tasks_asked_about_each_with_its_answer_first() {
        let now: Zoned = "2026-11-27T12:00:00[UTC]".parse().unwrap();
        let mut snapshot = Snapshot::default();
        let inbox = Project::inbox();
        let mut home = Project::new("Home", OrderKey::middle());
        home.reviewed_at = Some("2026-11-10T12:00:00Z".parse().unwrap());
        let mut old = Task::new(inbox.id, "Write report", OrderKey::middle());
        old.created_at = "2026-10-01T12:00:00Z".parse().unwrap();
        snapshot.tasks.insert(old.id, old);
        snapshot.projects.insert(inbox.id, inbox);
        snapshot.projects.insert(home.id, home);
        let listed = review(&snapshot, &now).unwrap();
        assert_eq!(listed.announcement, "1 project to review and 1 task to reconsider");
        let titles: Vec<&str> = listed.rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Home", "Write report"]);
        assert_eq!(listed.rows[0].value.as_deref(), Some("last reviewed 2 weeks ago, every week"));
        assert_eq!(listed.rows[0].actions.iter().take(2).map(|a| a.kind).collect::<Vec<_>>(), [ActionKind::OpenProject, ActionKind::MarkReviewed]);
        assert!(listed.rows[1].value.as_deref().unwrap().starts_with("near the top 8 weeks with nothing done"));
        assert_eq!(listed.rows[1].actions[0].kind, ActionKind::Reconsider);
        assert_eq!(listed.rows.iter().map(|r| r.row).collect::<Vec<_>>(), [1, 2]);
    }
}
