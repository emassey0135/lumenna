//! The review: what is due for a second look (`lumenna_records::review`), marking a project
//! reviewed, how often it is, and answering the planner's question about a task. Each answer
//! is an ordinary change, undone as any other.

use jiff::Zoned;
use lumenna_core::edit;
use lumenna_records::review::{DROP, KEEP, LESS_OFTEN, NO_DATE};

use crate::error::{LumennaError, Result};
use crate::tasks::record;
use crate::types::{Change, Rows};
use crate::{Lumenna, repaired, resolve};

impl Lumenna {
    /// The review: the projects due for review, then the tasks the planner asks about.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn review(&self) -> Result<Rows> {
        let now = Zoned::now();
        self.told(|store| lumenna_records::review::review(&repaired(store), &now))
    }

    /// Marks the project `project` (a name or an identifier) reviewed now.
    ///
    /// # Errors
    ///
    /// If no project matches, or it is the Inbox, which is emptied rather than reviewed.
    pub fn mark_reviewed(&self, project: &str) -> Result<Change> {
        self.told(|store| {
            let snapshot = repaired(store);
            let found = resolve::project(&snapshot, project)?;
            if found.is_inbox {
                return Err(LumennaError::new(lumenna_records::not_offered(
                    lumenna_records::ActionKind::MarkReviewed,
                    lumenna_records::Subject::Project,
                    false,
                )));
            }
            let change = edit::mark_reviewed(&snapshot, found.id, lumenna_core::time::now())?;
            record(store, &change)
        })
    }

    /// Sets how often `project` is reviewed, from words: "every 2 weeks", "month", "14 days",
    /// "never".
    ///
    /// # Errors
    ///
    /// If no project matches, it is the Inbox, or the words say no interval.
    pub fn review_every(&self, project: &str, every: &str) -> Result<Change> {
        let days = lumenna_records::review::parse_interval(every)?;
        self.told(|store| {
            let snapshot = repaired(store);
            let found = resolve::project(&snapshot, project)?;
            if found.is_inbox {
                return Err(LumennaError::new(lumenna_records::not_offered(
                    lumenna_records::ActionKind::ReviewEvery,
                    lumenna_records::Subject::Project,
                    false,
                )));
            }
            let change = edit::review_every(&snapshot, found.id, Some(days))?;
            if change.is_empty() {
                return Ok(Change::unchanged(format!(
                    "{} is already reviewed {}",
                    found.name,
                    lumenna_core::review::interval_words(lumenna_core::review::review_every(found))
                )));
            }
            record(store, &change)
        })
    }

    /// Answers the planner's question about task `id`: `keep`, `drop` (to the trash),
    /// `less-often` (a repeating one half as often) or `no-date`.
    ///
    /// # Errors
    ///
    /// If no task matches, or the answer is none of those it can take.
    pub fn reconsider(&self, id: &str, answer: &str) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            let snapshot = repaired(store);
            let id = resolve::task_id(&snapshot, id)?;
            let task = snapshot.tasks.get(&id).ok_or_else(|| LumennaError::new("no such task"))?;
            // Every answer settles the question: asked again only once put off as often again.
            let facts = snapshot.facts();
            let put_off = lumenna_core::review::Counts::new(&snapshot, &facts, &now).put_off(&snapshot, id);
            let change = match answer.trim() {
                KEEP => edit::keep_task(&snapshot, id, put_off, now.timestamp())?,
                DROP => edit::trash_task(&snapshot, id)?,
                LESS_OFTEN => edit::make_less_often(&snapshot, id, put_off)?,
                NO_DATE => {
                    if task.due.is_none() {
                        return Ok(Change::unchanged(format!("{} has no due date", task.title)));
                    }
                    let after = lumenna_core::model::Task { due: None, kept_put_off: Some(put_off), ..task.clone() };
                    let change = edit::update_task(task.clone(), after);
                    store.apply_recorded(&change)?;
                    return Ok(Change::announced(format!("{} has no due date now", task.title), &change));
                }
                other => {
                    return Err(LumennaError::new(format!(
                        "the answer is {KEEP}, {DROP}, {LESS_OFTEN} or {NO_DATE}, not {other}"
                    )));
                }
            };
            record(store, &change)
        })
    }
}
