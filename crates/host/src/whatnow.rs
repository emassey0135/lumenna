//! "What should I work on?", asked of the day: in a block under way, its own unfinished
//! sittings first, then what its suggestions rank first; in free time, what is suggested for
//! the time until the next block. Choosing one starts on it at once: its timer, and for a
//! task not yet in a block, a sitting in the block under way or a block of its own in the
//! free time. One change, which one undo takes back.

use jiff::Zoned;
use lumenna_core::dayplan::UNESTIMATED_MINS;
use lumenna_core::edit::{self, Edit};
use lumenna_core::model::{BlockKind, BlockRef, BlockSeries};

use crate::actions::Choices;
use crate::error::{LumennaError, Result};
use crate::types::Change;
use crate::{Lumenna, repaired, resolve};
use crate::plan::{Now, where_now};


impl Lumenna {
    /// What to work on now, best first, as the day's What Should I Work On? offers it. `now`
    /// is for tests, a local date and time.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn what_should_i_work_on(&self, now: Option<String>) -> Result<Choices> {
        self.what_now_choices(&crate::replan::now_or(now.as_deref())?)
    }

    /// Starts on one of [`Lumenna::what_should_i_work_on`]'s choices now.
    ///
    /// # Errors
    ///
    /// If it is gone.
    pub fn start_on(&self, choice: &str, now: Option<String>) -> Result<Change> {
        self.work_on(choice, &crate::replan::now_or(now.as_deref())?)
    }

    /// What the timer's control shows now (`lumenna_records::controls`): Control Center's, a
    /// Quick Settings tile, a watch's. `now` is for tests, a local date and time.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn timer_control(&self, now: Option<String>) -> Result<lumenna_records::controls::TimerControl> {
        let now = crate::replan::now_or(now.as_deref())?;
        self.with(|store| {
            store.load_all_years()?;
            lumenna_records::controls::timer_control(&repaired(store), &now)
        })
    }

    /// The timer control pressed: stops the timer running, else starts on the first of What
    /// Should I Work On?'s choices, else says why there is nothing to start. `now` is for tests,
    /// a local date and time.
    ///
    /// # Errors
    ///
    /// If the store cannot be read or written.
    pub fn toggle_timer(&self, now: Option<String>) -> Result<Change> {
        let now = crate::replan::now_or(now.as_deref())?;
        let running = self.with(|store| {
            store.load_all_years()?;
            Ok(lumenna_records::controls::running_sitting(&repaired(store)))
        })?;
        if let Some(sitting) = running {
            let timer = self.stop_timer(&sitting, None)?;
            return Ok(Change { notices: timer.notices, changed: timer.changed, ..Change::unchanged(timer.announcement) });
        }
        let choices = self.what_now_choices(&now)?;
        match choices.choices.first() {
            Some(first) => self.work_on(&first.id, &now),
            None => Ok(Change::unchanged(choices.announcement)),
        }
    }
}

impl Lumenna {
    /// What to work on now, best first: `sitting:<id>` for a sitting already in the block under
    /// way, `task:<id>` for anything else.
    pub(crate) fn what_now_choices(&self, now: &Zoned) -> Result<Choices> {
        self.with(|store| {
            store.load_all_years()?;
            lumenna_records::choices::what_now(&repaired(store), now)
        })
    }

    /// Starts on `choice` (from [`Lumenna::what_now_choices`]) now.
    pub(crate) fn work_on(&self, choice: &str, now: &Zoned) -> Result<Change> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let started = lumenna_core::time::truncate(now.timestamp());
            if let Some(sitting) = choice.strip_prefix("sitting:") {
                let id = resolve::assignment_id(&snapshot, sitting)?;
                let year = crate::planning::assignment_year(&snapshot, id)?;
                let change = edit::start_timer(&snapshot, id, year, now)?;
                if change.is_empty() {
                    return Ok(Change::unchanged("its timer is already running"));
                }
                let title = snapshot.assignments.get(&id).and_then(|a| snapshot.tasks.get(&a.task_id)).map(|t| t.title.clone());
                let change = Edit { description: format!("Started on {}", title.unwrap_or_default()), ..change };
                store.apply_recorded(&change)?;
                return Ok(Change::of(&change));
            }
            let task_id = resolve::task_id(&snapshot, choice.strip_prefix("task:").unwrap_or(choice))?;
            let task = snapshot.tasks.get(&task_id).ok_or_else(|| LumennaError::new("that task is not here"))?.clone();
            let mut edits = Vec::new();
            let mut world = snapshot.clone();
            let (block, year, block_title) = match where_now(&snapshot, now)? {
                Now::Block(occurrence, _) => {
                    let title = snapshot.series.get(&occurrence.series_id).map(|s| s.title.clone()).unwrap_or_default();
                    let year = match lumenna_records::plan::block_ref(&snapshot, &occurrence) {
                        BlockRef::Occurrence(_, date) => date.year(),
                        BlockRef::OneOff(series) => snapshot.series.get(&series).map_or(now.date().year(), |s| s.start_date.year()),
                    };
                    (lumenna_records::plan::block_ref(&snapshot, &occurrence), year, title)
                }
                Now::Free(left, _) => {
                    // A block of its own for the free time: as long as the task is meant to take,
                    // or until the next block.
                    let minutes = task.estimate_mins.unwrap_or(UNESTIMATED_MINS).min(left).max(1);
                    let at = now.time();
                    let start = jiff::civil::time(at.hour(), at.minute(), 0, 0);
                    let series = BlockSeries::one_off(&task.title, BlockKind::Work, now.date(), start, minutes)
                        .map_err(|e| LumennaError::new(e.to_string()))?;
                    let id = series.id;
                    world.series.insert(id, series.clone());
                    edits.push(edit::create_series(series));
                    (BlockRef::OneOff(id), now.date().year(), task.title.clone())
                }
            };
            let mut assign = edit::assign_task(&world, task_id, block, year)?;
            if let Some(edit::Change::Assignment { transition, .. }) = assign.changes.first_mut()
                && let Some(new) = transition.after.as_mut()
            {
                new.start(started);
            }
            edits.push(assign);
            let description = if snapshot.settings.verbosity == lumenna_core::model::Verbosity::Terse {
                format!("Started on {}", task.title)
            } else if block_title == task.title {
                format!("Started on {} in a block of its own", task.title)
            } else {
                format!("Started on {} in {block_title}", task.title)
            };
            let change = Edit { description, changes: edits.into_iter().flat_map(|e| e.changes).collect() };
            store.apply_recorded(&change)?;
            Ok(Change::of(&change))
        })
    }
}
