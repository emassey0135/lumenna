//! A task's history and urgency, in words: what a person asks for about one task ("why is
//! this at the top?", "how has this gone?"), never said unasked.

use jiff::Zoned;
use lumenna_core::history::History;
use lumenna_core::model::Task;
use lumenna_core::snapshot::Snapshot;
use lumenna_core::urgency::{Term, Urgency, urgency};
use serde::{Deserialize, Serialize};

use crate::error::{LumennaError, Result};
use crate::words::{count_line, duration};
use crate::{Lumenna, repaired, resolve};

/// One term of a task's urgency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct UrgencyTerm {
    /// `due`, `priority`, `age`, `started`, `blocking`, `blocked`, `assigned_today`.
    pub term: String,
    /// What it says: "due tomorrow", "priority 1".
    pub words: String,
    /// What it adds, before the project's weight.
    pub value: f32,
}

/// A task's history and what makes up its urgency.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct TaskHistory {
    /// Its title.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// What has happened to it, as parts: "3 sittings, 1 hour 30 minutes logged against a
    /// 1-hour estimate", "put in a block twice and left".
    pub history: Vec<String>,
    /// Its urgency, to one decimal place.
    pub urgency: f32,
    /// What it is made of, largest first.
    pub terms: Vec<UrgencyTerm>,
    /// What the project's weight does, when it does anything: "Work weighs 2".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub weight: String,
}

crate::announced!(TaskHistory);

/// "today", "3 days", "2 weeks", "4 months", "a year".
fn age_words(days: i64) -> String {
    match days {
        ..1 => "today".to_owned(),
        1..14 => format!("{} ago", count_line(usize::try_from(days).unwrap_or(0), "day")),
        14..60 => format!("{} ago", count_line(usize::try_from(days / 7).unwrap_or(0), "week")),
        60..730 => format!("{} ago", count_line(usize::try_from(days / 30).unwrap_or(0), "month")),
        _ => format!("{} ago", count_line(usize::try_from(days / 365).unwrap_or(0), "year")),
    }
}

fn times(n: u32) -> String {
    match n {
        1 => "once".to_owned(),
        2 => "twice".to_owned(),
        n => format!("{n} times"),
    }
}

/// What has happened to a task, as parts every app says alike.
#[must_use]
pub fn history_words(found: &History, today: jiff::civil::Date) -> Vec<String> {
    let mut parts = Vec::new();
    if found.sittings > 0 {
        let logged = if found.worked_mins > 0 { format!("{} logged", duration(found.worked_mins)) } else { "nothing logged".to_owned() };
        let against = found.estimate_mins.map_or_else(String::new, |e| format!(" against an estimate of {}", duration(e)));
        parts.push(format!("{}, {logged}{against}", count_line(usize::try_from(found.sittings).unwrap_or(0), "sitting")));
        if let Some(estimate) = found.estimate_mins
            && found.worked_mins > 0
            && found.worked_mins < estimate
        {
            parts.push(format!("{} left of the estimate", duration(estimate - found.worked_mins)));
        }
    }
    if found.put_off > 0 {
        parts.push(format!("put in a block {} and left", times(found.put_off)));
    }
    if let Some(day) = found.last_worked {
        parts.push(format!("last worked on {}", lumenna_core::time::day_words(day, today)));
    }
    if found.completions > 0 {
        parts.push(format!("done {}", times(found.completions)));
    }
    let made = age_words(found.age_days);
    parts.push(if found.sittings == 0 && found.age_days >= 7 {
        format!("made {made}, never put in a block")
    } else {
        format!("made {made}")
    });
    parts
}

fn term_words(term: Term, task: &Task, snapshot: &Snapshot, now: &Zoned) -> (&'static str, String) {
    match term {
        Term::Due => (
            "due",
            task.due.as_ref().map_or_else(String::new, |due| lumenna_core::time::due_words(due.date, now.date())),
        ),
        Term::Priority => ("priority", format!("priority {}", task.priority.as_u8())),
        Term::Age => ("age", format!("made {}", age_words(now.timestamp().duration_since(task.created_at).as_secs() / 86_400))),
        Term::Started => ("started", "partly worked".to_owned()),
        Term::Blocking => {
            let waiting = snapshot.tasks.values().filter(|t| t.depends.contains(&task.id) && !t.is_deleted()).count();
            ("blocking", format!("{} on it", match waiting { 1 => "1 task waits".to_owned(), n => format!("{n} tasks wait") }))
        }
        Term::Blocked => ("blocked", "waits on another task".to_owned()),
        Term::AssignedToday => ("assigned_today", "already in a block today".to_owned()),
    }
}

fn round(value: f32) -> f32 {
    (value * 10.0).round() / 10.0
}

/// A task's urgency as words, largest term first.
pub(crate) fn urgency_view(found: &Urgency, task: &Task, snapshot: &Snapshot, now: &Zoned) -> (Vec<UrgencyTerm>, String) {
    let mut terms: Vec<UrgencyTerm> = found
        .terms
        .iter()
        .map(|(term, value)| {
            let (name, words) = term_words(*term, task, snapshot, now);
            UrgencyTerm { term: name.to_owned(), words, value: round(*value) }
        })
        // What rounds to nothing says nothing: a task made today has an age of 0.0.
        .filter(|term| term.value != 0.0)
        .collect();
    terms.sort_by(|a, b| b.value.abs().total_cmp(&a.value.abs()));
    let weight = if (found.weight - 1.0).abs() > f32::EPSILON {
        let project = snapshot.projects.get(&task.project_id).map_or("Its project", |p| p.name.as_str());
        format!("{project} weighs {}", found.weight)
    } else {
        String::new()
    };
    (terms, weight)
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// A task's history and what makes up its urgency: asked for about one task, never said
    /// unasked.
    ///
    /// # Errors
    ///
    /// If no task matches `id`.
    pub fn task_history(&self, id: &str) -> Result<TaskHistory> {
        let now = Zoned::now();
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let task = snapshot.tasks.get(&id).ok_or_else(|| LumennaError::new("that task is not here"))?;
            let facts = snapshot.facts();
            let found = urgency(&snapshot, &facts, task, &now, &snapshot.settings.urgency);
            let (terms, weight) = urgency_view(&found, task, &snapshot, &now);
            Ok(TaskHistory {
                announcement: task.title.clone(),
                notices: Vec::new(),
                history: history_words(&lumenna_core::history::history(&snapshot, task, &now), now.date()),
                urgency: round(found.score),
                terms,
                weight,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tasks_history_reads_as_a_person_would_say_it() {
        let found = History {
            sittings: 3,
            worked_mins: 43,
            estimate_mins: Some(60),
            put_off: 2,
            age_days: 120,
            completions: 0,
            last_worked: None,
        };
        let today = jiff::civil::date(2026, 5, 6);
        assert_eq!(
            history_words(&found, today),
            [
                "3 sittings, 43 minutes logged against an estimate of 1 hour",
                "17 minutes left of the estimate",
                "put in a block twice and left",
                "made 4 months ago",
            ]
        );
        let untouched = History { age_days: 30, ..History::default() };
        assert_eq!(history_words(&untouched, today), ["made 4 weeks ago, never put in a block"]);
    }
}
