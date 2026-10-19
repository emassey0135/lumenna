//! A task's history and urgency, asked for by identifier.

use jiff::Zoned;
use lumenna_core::urgency::urgency;

use crate::error::{LumennaError, Result};
use crate::history::{TaskHistory, history_words, round, urgency_view};
use crate::{Lumenna, repaired, resolve};

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
