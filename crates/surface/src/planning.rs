//! The day: blocks, what is assigned to them, and the time spent (§3.7).

use jiff::Zoned;
use lumenna_core::edit::{self, EditError};
use lumenna_core::id::AssignmentId;
use lumenna_core::model::{BlockAssignment, BlockKind, BlockRef, BlockSeries};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::resolve;
use crate::tasks::{record, record_or};
use crate::types::{Announced, Change, NewBlock, Plan, PlanAssignment, PlanBlock, Rows, Timer};
use crate::words::{count_line, time_text};
use crate::{Lumenna, repaired};

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// A day's blocks and what is assigned to them. `date` is a date phrase; today when
    /// absent.
    ///
    /// Blocks and assignments are each numbered across the day, which is what lets a terminal
    /// say `lum start 1` straight afterwards (§15).
    ///
    /// # Errors
    ///
    /// If the date cannot be read, or a block's rule cannot be expanded.
    pub fn plan(&self, date: Option<String>) -> Result<Plan> {
        let now = Zoned::now();
        self.with(|store| {
            let day = resolve::date(date.as_deref(), &now)?;
            // Year documents load only when a year is viewed, which is what keeps the watch
            // viable (§8). This is that moment.
            store.load_year(day.year())?;

            let snapshot = repaired(store);
            let occurrences = snapshot.day(day)?;

            let mut assignment_row = 0;
            let mut blocks = Vec::new();
            for (index, occurrence) in occurrences.iter().enumerate() {
                let series = snapshot.series.get(&occurrence.series_id);
                let block_ref = series.map(|s| occurrence.block_ref(s));
                let mut assigned: Vec<&BlockAssignment> = snapshot
                    .assignments
                    .values()
                    .filter(|a| Some(a.block_ref) == block_ref)
                    .collect();
                assigned.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));

                let assignments = assigned
                    .iter()
                    .map(|assignment| {
                        let elapsed =
                            assignment.elapsed(now.timestamp(), Some(occurrence.duration_mins));
                        assignment_row += 1;
                        PlanAssignment {
                            row: assignment_row,
                            id: assignment.id.to_string(),
                            task: assignment.task_id.to_string(),
                            title: snapshot
                                .tasks
                                .get(&assignment.task_id)
                                .map_or_else(|| "(not loaded)".to_owned(), |t| t.title.clone()),
                            status: assignment.status.speech().to_owned(),
                            planned_mins: assignment.planned_mins,
                            minutes: elapsed.mins,
                            capped: elapsed.capped,
                        }
                    })
                    .collect();

                blocks.push(PlanBlock {
                    row: to_u32(index + 1),
                    id: format!("{}@{}", occurrence.series_id, occurrence.date),
                    series: occurrence.series_id.to_string(),
                    title: occurrence.title.clone(),
                    start: time_text(occurrence.start_time),
                    end: time_text(occurrence.end_time()),
                    duration_mins: occurrence.duration_mins,
                    assignments,
                });
            }

            Ok(Plan {
                announcement: format!("{day}, {}", count_line(blocks.len(), "block")),
                notices: Vec::new(),
                date: day.to_string(),
                count: to_u32(blocks.len()),
                blocks,
            })
        })
    }

    /// Adds a block, once or repeating.
    ///
    /// A repeating block whose start date the rule never lands on starts on the rule's first
    /// real occurrence instead — a series anchored on a day it never occurs is a trap (§5).
    ///
    /// # Errors
    ///
    /// If the time, date, kind or repetition cannot be read, or the block is malformed.
    pub fn add_block(&self, block: NewBlock) -> Result<Change> {
        let now = Zoned::now();
        self.with(|store| {
            let day = resolve::date(block.date.as_deref(), &now)?;
            let start = resolve::time(&block.at)?;
            let kind = match block.kind.to_lowercase().as_str() {
                "work" => BlockKind::Work,
                "break" => BlockKind::Break,
                "event" => BlockKind::Event,
                other => {
                    return Err(LumennaError::new(format!(
                        "'{other}' is not a block kind; use work, break, or event"
                    )));
                }
            };
            let mut series = BlockSeries::one_off(&block.title, kind, day, start, block.minutes)
                .map_err(|e| LumennaError::new(e.to_string()))?;

            if let Some(repeat) = &block.repeat {
                let tokens = lumenna_parse::words(repeat);
                let (spec, _, _) = lumenna_parse::date::parse_recurrence(&tokens, 0)
                    .ok_or_else(|| {
                        LumennaError::new(format!("could not read a repetition from '{repeat}'"))
                    })?;
                let rrule = spec.to_rrule();
                let rule = lumenna_core::recur::Rule::parse(&rrule)?;
                if let Some(first) = rule.first_from(day)? {
                    series.start_date = first;
                }
                series.rrule = Some(rrule);
                series.end_date = None;
            }

            store.load_year(series.start_date.year())?;
            let (start, first) = (series.start_time, series.start_date);
            let change = edit::create_series(series);
            store.apply_recorded(&change)?;
            Ok(Change::announced(
                format!("Added block {} at {} on {first}", block.title, time_text(start)),
                &change,
            ))
        })
    }

    /// Every block series, by when it starts.
    ///
    /// # Errors
    ///
    /// If a year's document cannot be loaded.
    pub fn list_blocks(&self) -> Result<Rows> {
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let mut live: Vec<&BlockSeries> =
                snapshot.series.values().filter(|s| s.deleted_at.is_none()).collect();
            live.sort_by_key(|s| (s.start_date, s.start_time, s.id));
            let count = to_u32(live.len());
            let rows: Vec<Row> = live
                .iter()
                .enumerate()
                .map(|(index, series)| {
                    let mut value = format!(
                        "{} for {} minutes from {}",
                        time_text(series.start_time),
                        series.duration_mins,
                        series.start_date
                    );
                    if let Some(rrule) = &series.rrule {
                        value.push_str(&format!(", repeats: {rrule}"));
                    }
                    Row {
                        id: RowId::Occurrence(series.id, series.start_date),
                        role: Role::Block,
                        depth: 0,
                        index: to_u32(index) + 1,
                        count,
                        expanded: None,
                        checked: None,
                        title: series.title.clone(),
                        state: Vec::new(),
                        value: Some(value),
                        hint: None,
                    }
                })
                .collect();
            Ok(Rows::new(&rows, "block"))
        })
    }

    /// Deletes a block series, every occurrence of it.
    ///
    /// # Errors
    ///
    /// If no block matches `id`.
    pub fn delete_block(&self, id: &str) -> Result<Change> {
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let change = edit::trash_series(&snapshot, resolve::series_id(&snapshot, id)?)?;
            record_or(store, &change, "that block is already in the trash")
        })
    }

    /// Puts a task into a block for one sitting (§3.7). `date` picks the day of a repeating
    /// block, today when absent; `minutes` is how long the sitting is meant to take.
    ///
    /// # Errors
    ///
    /// If the task or block cannot be found, or the block does not take tasks.
    pub fn assign(
        &self,
        task: &str,
        block: &str,
        date: Option<String>,
        minutes: Option<u32>,
    ) -> Result<Change> {
        let now = Zoned::now();
        self.with(|store| {
            let day = resolve::date(date.as_deref(), &now)?;
            // Looking a block up by identifier cannot know which year to open, so it opens
            // them all rather than failing to find one that is on disk.
            store.load_all_years()?;
            let snapshot = repaired(store);

            let task_id = resolve::task_id(&snapshot, task)?;
            let series_id = resolve::series_id(&snapshot, block)?;
            let series =
                snapshot.series.get(&series_id).ok_or(EditError::NotFound { kind: "block" })?;

            // A one-off block's assignments name only the series, so that moving the block
            // carries them with it (§3.7). A repeating block's name the day, and are sharded
            // by it; a one-off's follow the series' year.
            let (block_ref, year) = if series.is_recurring() {
                (BlockRef::Occurrence(series_id, day), day.year())
            } else {
                (BlockRef::OneOff(series_id), series.start_date.year())
            };

            let mut change = edit::assign_task(&snapshot, task_id, block_ref, year)?;
            if let Some(minutes) = minutes
                && let Some(edit::Change::Assignment { transition, .. }) = change.changes.first_mut()
                && let Some(assignment) = transition.after.as_mut()
            {
                assignment.planned_mins = Some(minutes);
            }
            let announcement = format!("{} into {} on {day}", change.description, series.title);
            store.apply_recorded(&change)?;
            Ok(Change::announced(announcement, &change))
        })
    }

    /// Takes a task back out of a block.
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`.
    pub fn unassign(&self, assignment: &str) -> Result<Change> {
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let change = edit::unassign(&snapshot, id, assignment_year(&snapshot, id)?)?;
            record(store, &change)
        })
    }

    /// Starts the timer on a sitting.
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`.
    pub fn start_timer(&self, assignment: &str) -> Result<Change> {
        let now = Zoned::now();
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let change =
                edit::start_timer(&snapshot, id, assignment_year(&snapshot, id)?, &now)?;
            if change.is_empty() {
                return Ok(Change::unchanged("that timer is already running"));
            }
            store.apply_recorded(&change)?;
            Ok(Change::announced("Started timer", &change))
        })
    }

    /// Stops the timer and logs the minutes — or, with `minutes`, records that figure as the
    /// whole of the sitting instead, which is how time is logged without a timer and how a
    /// capped figure is put right.
    ///
    /// A timer that ran past the end of its block is capped at the block's length and says
    /// so: a truncated figure is not a fact (§3.7).
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`.
    pub fn stop_timer(&self, assignment: &str, minutes: Option<u32>) -> Result<Timer> {
        let now = Zoned::now();
        self.with(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let year = assignment_year(&snapshot, id)?;
            let timer = |announcement: String, changed: bool, minutes: u32, capped: bool| Timer {
                announcement,
                notices: Vec::new(),
                changed,
                assignment: id.to_string(),
                minutes,
                capped,
            };

            if let Some(minutes) = minutes {
                let change = edit::log_minutes(&snapshot, id, year, minutes)?;
                if change.is_empty() {
                    return Ok(timer(
                        format!("{minutes} minutes were already logged"),
                        false,
                        minutes,
                        false,
                    ));
                }
                store.apply_recorded(&change)?;
                return Ok(timer(change.description, true, minutes, false));
            }

            // The cap is the occurrence's own length — an exception may have changed this
            // day's block — falling back to the series' when the occurrence cannot be found,
            // since a timer is still worth stopping then.
            let sitting = &snapshot.assignments[&id];
            let cap = edit::occurrence_of(&snapshot, sitting.block_ref)
                .map(|occurrence| occurrence.duration_mins)
                .ok()
                .or_else(|| {
                    snapshot.series.get(&sitting.block_ref.series_id()).map(|s| s.duration_mins)
                });

            let (change, elapsed) = edit::stop_timer(&snapshot, id, year, cap, &now)?;
            if change.is_empty() {
                return Ok(timer(
                    format!(
                        "that timer is not running; {} logged",
                        count_line(elapsed.mins as usize, "minute")
                    ),
                    false,
                    elapsed.mins,
                    elapsed.capped,
                ));
            }
            store.apply_recorded(&change)?;

            let stopped =
                timer(format!("Logged {} minutes", elapsed.mins), true, elapsed.mins, elapsed.capped);
            Ok(if elapsed.capped {
                // Recorded so the sitting is not lost, and said so that it can be replaced
                // with the real figure.
                stopped.note(format!(
                    "that timer ran past the end of its block, so it was capped at {} \
                     minutes; if that is wrong, log the real figure in its place",
                    elapsed.mins
                ))
            } else {
                stopped
            })
        })
    }
}

/// Which `blocks-<year>` document an assignment lives in.
///
/// A one-off block's assignment names no date, so its year is its series' — and a series
/// this device has not loaded gives no answer. Guessing would write the edit into a document
/// for the wrong year, holding a fragment of the record, so it is an error instead.
fn assignment_year(snapshot: &Snapshot, id: AssignmentId) -> Result<i16> {
    snapshot
        .assignments
        .get(&id)
        .and_then(|assignment| {
            lumenna_store::doc::assignment_year(
                assignment,
                snapshot.series.get(&assignment.block_ref.series_id()),
            )
        })
        .ok_or_else(|| {
            LumennaError::new(
                "that assignment's block is not in this store, so there is no telling which \
                 year it belongs to",
            )
        })
}
