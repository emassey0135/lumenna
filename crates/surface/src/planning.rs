//! The day: blocks, what is assigned to them, and the time spent.

use jiff::Zoned;
use lumenna_core::edit::{self, EditError};
use lumenna_core::id::AssignmentId;
use lumenna_core::model::{AssignmentStatus, BlockAssignment, BlockKind, BlockRef, BlockSeries, ExceptionAction};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::resolve;
use crate::tasks::{record, record_or};
use crate::types::{
    Announced, BlockEdit, BlockScope, BlockShown, CancelledBlock, Change, NewBlock, Plan, PlanAssignment,
    PlanBlock, PlanItem, Rows, Timer, WorkBlock, WorkBlocks, repetition_phrase,
};
use crate::words::{block_details, count_line, duration, sitting_details, time_text};
use crate::{Lumenna, repaired};

fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// How far ahead putting a task in a block looks when asked from the task: a week, so the
/// list stays short enough to choose from by ear. The planner reaches any other day.
pub const WORK_BLOCK_DAYS: u32 = 7;

#[cfg_attr(feature = "uniffi", uniffi::export)]
impl Lumenna {
    /// The work blocks a task could go in, over `days` days from `from`: what putting
    /// a task in a block offers from the task itself, rather than from a day. `from` is a date
    /// phrase, today when absent; `days` is [`WORK_BLOCK_DAYS`] when absent, and at most 31.
    /// Breaks and events are left out: only work blocks take tasks.
    ///
    /// # Errors
    ///
    /// If the date cannot be read, or a block's rule cannot be expanded.
    pub fn work_blocks(&self, from: Option<String>, days: Option<u32>) -> Result<WorkBlocks> {
        let first = resolve::date(from.as_deref(), &Zoned::now())?;
        let days = days.unwrap_or(WORK_BLOCK_DAYS).clamp(1, 31);
        let mut blocks = Vec::new();
        for offset in 0..days {
            let Ok(day) = first.checked_add(jiff::Span::new().days(i64::from(offset))) else { break };
            let plan = self.plan(Some(day.to_string()))?;
            blocks.extend(plan.blocks.into_iter().filter(|block| block.accepts_tasks).map(
                |block| WorkBlock {
                    assigned: to_u32(block.assignments.len()),
                    id: block.id,
                    date: plan.date.clone(),
                    title: block.title,
                    start: block.start,
                    end: block.end,
                    duration_mins: block.duration_mins,
                    when: block.when,
                },
            ));
        }
        Ok(WorkBlocks {
            announcement: format!("{} over {}", count_line(blocks.len(), "work block"), count_line(days as usize, "day")),
            notices: Vec::new(),
            from: first.to_string(),
            days,
            blocks,
        })
    }

    /// A day's blocks and what is assigned to them. `date` is a date phrase; today when
    /// absent.
    ///
    /// Blocks and assignments are each numbered across the day, which is what lets a terminal
    /// say `lum start 1` straight afterwards.
    ///
    /// # Errors
    ///
    /// If the date cannot be read, or a block's rule cannot be expanded.
    pub fn plan(&self, date: Option<String>) -> Result<Plan> {
        let now = Zoned::now();
        self.told(|store| {
            let day = resolve::date(date.as_deref(), &now)?;
            // Year documents load only when a year is viewed, which is what keeps the watch
            // viable. This is that moment.
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
                        // A sitting in progress whose timer is off is paused: it ran, and has
                        // not been ended.
                        let paused = assignment.status == AssignmentStatus::InProgress
                            && !assignment.is_running();
                        let mut sitting = PlanAssignment {
                            row: assignment_row,
                            id: assignment.id.to_string(),
                            task: assignment.task_id.to_string(),
                            title: snapshot
                                .tasks
                                .get(&assignment.task_id)
                                .map_or_else(|| "(not loaded)".to_owned(), |t| t.title.clone()),
                            status: if paused { "paused" } else { assignment.status.speech() }.to_owned(),
                            planned_mins: assignment.planned_mins,
                            minutes: elapsed.mins,
                            capped: elapsed.capped,
                            running: assignment.is_running(),
                            details: Vec::new(),
                        };
                        sitting.details = sitting_details(&sitting);
                        sitting
                    })
                    .collect();

                let when = if day != now.date() {
                    ""
                } else if occurrence.end_time() <= now.time() {
                    "past"
                } else if occurrence.start_time <= now.time() {
                    "now"
                } else {
                    "upcoming"
                };
                let mut block = PlanBlock {
                    row: to_u32(index + 1),
                    id: format!("{}@{}", occurrence.series_id, occurrence.date),
                    series: occurrence.series_id.to_string(),
                    title: occurrence.title.clone(),
                    start: time_text(occurrence.start_time),
                    end: time_text(occurrence.end_time()),
                    duration_mins: occurrence.duration_mins,
                    kind: kind_word(occurrence.kind).to_owned(),
                    when: when.to_owned(),
                    repeats: series.is_some_and(BlockSeries::is_recurring),
                    changed_for_this_day: occurrence.modified,
                    assignments,
                    accepts_tasks: occurrence.flags.accepts_tasks,
                    counts_capacity: occurrence.flags.counts_capacity,
                    anchored: occurrence.flags.anchored,
                    colour: series.and_then(|s| s.color.clone()),
                    notes: series.map(|s| s.notes.clone()).unwrap_or_default(),
                    details: Vec::new(),
                };
                block.details = block_details(&block);
                blocks.push(block);
            }

            let overdue = (day == now.date()).then(|| {
                let facts = snapshot.facts();
                snapshot
                    .tasks
                    .values()
                    .filter(|t| !t.is_deleted())
                    .filter(|t| facts.notable_states_of(t, &now).contains(&lumenna_core::State::Overdue))
                    .count()
            });
            let timeline = timeline(
                &occurrences,
                snapshot.settings.day_window,
                (day == now.date()).then(|| now.time()),
            );
            let mut cancelled: Vec<CancelledBlock> = snapshot
                .exceptions
                .values()
                .filter(|e| e.original_date == day && e.action == ExceptionAction::Cancelled)
                .filter_map(|e| snapshot.series.get(&e.series_id))
                .filter(|series| series.deleted_at.is_none())
                .map(|series| CancelledBlock {
                    series: series.id.to_string(),
                    title: series.title.clone(),
                    start: time_text(series.start_time),
                })
                .collect();
            cancelled.sort_by(|a, b| (&a.start, &a.title).cmp(&(&b.start, &b.title)));
            Ok(Plan {
                announcement: format!("{day}, {}", count_line(blocks.len(), "block")),
                notices: Vec::new(),
                date: day.to_string(),
                count: to_u32(blocks.len()),
                summary: summary(&blocks, overdue),
                timeline,
                blocks,
                cancelled,
            })
        })
    }

    /// Adds a block, once or repeating.
    ///
    /// A repeating block whose start date the rule never lands on starts on the rule's first
    /// real occurrence instead — a series anchored on a day it never occurs is a trap.
    ///
    /// # Errors
    ///
    /// If the time, date, kind or repetition cannot be read, or the block is malformed.
    pub fn add_block(&self, block: NewBlock) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            let day = resolve::date(block.date.as_deref(), &now)?;
            let start = resolve::time(&block.at)?;
            let kind = block_kind(&block.kind)?;
            let mut series = BlockSeries::one_off(&block.title, kind, day, start, block.minutes)
                .map_err(|e| LumennaError::new(e.to_string()))?;

            if let Some(repeat) = &block.repeat {
                let rrule = repetition(repeat)?;
                let rule = lumenna_core::recur::Rule::parse(&rrule)?;
                if let Some(first) = rule.first_from(day)? {
                    series.start_date = first;
                }
                series.rrule = Some(rrule);
                series.end_date = None;
            }
            apply_extras(
                &repaired(store),
                &mut series,
                &Extras {
                    notes: block.notes.as_deref(),
                    accepts_tasks: block.accepts_tasks,
                    counts_capacity: block.counts_capacity,
                    anchored: block.anchored,
                    min_minutes: block.min_minutes,
                    task_filter: block.task_filter.as_deref(),
                    until: block.until.as_deref(),
                    colour: block.colour.as_deref(),
                },
                &now,
            )?;

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

    /// Changes a block: every occurrence, or only the one on a given day (always
    /// asked of a repeating block, never guessed).
    ///
    /// A day's change is an exception; the series stays as it was, and later changes to the
    /// series leave the overridden fields alone.
    ///
    /// # Errors
    ///
    /// If no block matches `id`, a field cannot be read, the day is not one the block
    /// happens on, a single day is asked to repeat differently, or a block that happens once
    /// is asked about a single day.
    pub fn edit_block(&self, id: &str, edit: BlockEdit, scope: BlockScope) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            // Looked up by identifier, so every year is opened (CLAUDE.md, `load_all_years`).
            store.load_all_years()?;
            let snapshot = repaired(store);
            let series_id = resolve::series_id(&snapshot, id)?;
            let before =
                snapshot.series.get(&series_id).ok_or(EditError::NotFound { kind: "block" })?.clone();
            let start = edit.at.as_deref().map(resolve::time).transpose()?;
            let kind = edit.kind.as_deref().map(block_kind).transpose()?;
            if edit.minutes == Some(0) {
                return Err(LumennaError::new("a block has to last at least a minute"));
            }

            let change = match scope {
                BlockScope::Series => {
                    let mut after = before.clone();
                    if let Some(title) = edit.title {
                        after.title = title;
                    }
                    if let Some(start) = start {
                        after.start_time = start;
                    }
                    if let Some(minutes) = edit.minutes {
                        after.duration_mins = minutes;
                    }
                    if let Some(kind) = kind {
                        after.kind = kind;
                        after.flags = kind.default_flags();
                    }
                    if let Some(repeat) = &edit.repeat {
                        if repeat.eq_ignore_ascii_case("none") {
                            after.rrule = None;
                            after.end_date = Some(after.start_date);
                        } else {
                            let rrule = repetition(repeat)?;
                            // A series anchored on a day it never occurs is a trap.
                            let rule = lumenna_core::recur::Rule::parse(&rrule)?;
                            if let Some(first) = rule.first_from(after.start_date)? {
                                after.start_date = first;
                            }
                            after.rrule = Some(rrule);
                            after.end_date = None;
                        }
                    }
                    apply_extras(
                        &snapshot,
                        &mut after,
                        &Extras {
                            notes: edit.notes.as_deref(),
                            accepts_tasks: edit.accepts_tasks,
                            counts_capacity: edit.counts_capacity,
                            anchored: edit.anchored,
                            min_minutes: edit.min_minutes,
                            task_filter: edit.task_filter.as_deref(),
                            until: edit.until.as_deref(),
                            colour: edit.colour.as_deref(),
                        },
                        &now,
                    )?;
                    edit::update_series(before, after)
                }
                BlockScope::Occurrence { date } => {
                    if edit.repeat.is_some() {
                        return Err(LumennaError::new(
                            "one day of a block cannot repeat differently; change every \
                             occurrence to change how it repeats",
                        ));
                    }
                    // An exception holds the time, length, title, kind and flags; the
                    // rest belongs to the series.
                    if edit.notes.is_some()
                        || edit.min_minutes.is_some()
                        || edit.task_filter.is_some()
                        || edit.until.is_some()
                        || edit.colour.is_some()
                    {
                        return Err(LumennaError::new(
                            "notes, a task filter, a shortest length, a last day and a colour \
                             belong to every occurrence; change every occurrence to change them",
                        ));
                    }
                    let day = resolve::date(Some(&date), &now)?;
                    // Overrides already written for that day are kept unless replaced here.
                    let earlier = match snapshot.exceptions.get(&(series_id, day)).map(|e| &e.action)
                    {
                        Some(ExceptionAction::Modified { start_time, duration_mins, title, kind, flags }) => {
                            (*start_time, *duration_mins, title.clone(), *kind, *flags)
                        }
                        _ => (None, None, None, None, None),
                    };
                    // Flags start from the kind chosen now, else what the day already had, else
                    // the series', and take any flag set here.
                    let base = kind.map(BlockKind::default_flags).or(earlier.4);
                    let flags = if edit.accepts_tasks.is_some() || edit.counts_capacity.is_some() || edit.anchored.is_some() {
                        let mut flags = base.unwrap_or(before.flags);
                        if let Some(value) = edit.accepts_tasks {
                            flags.accepts_tasks = value;
                        }
                        if let Some(value) = edit.counts_capacity {
                            flags.counts_capacity = value;
                        }
                        if let Some(value) = edit.anchored {
                            flags.anchored = value;
                        }
                        Some(flags)
                    } else {
                        base
                    };
                    let action = ExceptionAction::Modified {
                        start_time: start.or(earlier.0),
                        duration_mins: edit.minutes.or(earlier.1),
                        title: edit.title.or(earlier.2),
                        kind: kind.or(earlier.3),
                        flags,
                    };
                    edit::except_occurrence(&snapshot, series_id, day, action)?
                }
            };
            record_or(store, &change, "nothing changed")
        })
    }

    /// Cancels one occurrence of a repeating block, leaving every other day alone.
    ///
    /// # Errors
    ///
    /// If no block matches `id`, it does not happen that day, or it happens only once —
    /// which is deleting it.
    pub fn cancel_occurrence(&self, id: &str, date: &str) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let series_id = resolve::series_id(&snapshot, id)?;
            let day = resolve::date(Some(date), &now)?;
            let change =
                edit::except_occurrence(&snapshot, series_id, day, ExceptionAction::Cancelled)?;
            record_or(store, &change, "that day is already cancelled")
        })
    }

    /// Puts one occurrence of a repeating block back as its series has it, undoing a
    /// cancellation or a change made to that day alone.
    ///
    /// # Errors
    ///
    /// If no block matches `id`.
    pub fn restore_occurrence(&self, id: &str, date: &str) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let series_id = resolve::series_id(&snapshot, id)?;
            let day = resolve::date(Some(date), &now)?;
            let change = edit::restore_occurrence(&snapshot, series_id, day)?;
            record_or(store, &change, "that day is already as the series has it")
        })
    }

    /// One block series: what an editor starts from.
    ///
    /// # Errors
    ///
    /// If no block matches `id`.
    pub fn show_block(&self, id: &str) -> Result<BlockShown> {
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let series_id = resolve::series_id(&snapshot, id)?;
            let series =
                snapshot.series.get(&series_id).ok_or(EditError::NotFound { kind: "block" })?;
            Ok(BlockShown {
                announcement: series.title.clone(),
                notices: Vec::new(),
                id: series.id.to_string(),
                title: series.title.clone(),
                start: time_text(series.start_time),
                minutes: series.duration_mins,
                kind: kind_word(series.kind).to_owned(),
                start_date: series.start_date.to_string(),
                repeats: series.is_recurring(),
                rrule: series.rrule.clone(),
                repetition: series.rrule.as_deref().and_then(|rule| repetition_phrase(rule, false)),
                notes: series.notes.clone(),
                accepts_tasks: series.flags.accepts_tasks,
                counts_capacity: series.flags.counts_capacity,
                anchored: series.flags.anchored,
                min_minutes: series.min_duration_mins,
                task_filter: series.task_filter.clone(),
                until: series.is_recurring().then_some(series.end_date).flatten().map(|d| d.to_string()),
                colour: series.color.clone(),
            })
        })
    }

    /// Every block series, by when it starts.
    ///
    /// # Errors
    ///
    /// If a year's document cannot be loaded.
    pub fn list_blocks(&self) -> Result<Rows> {
        self.told(|store| {
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
                        // In words where the grammar can say it; a rule from outside is
                        // given as it is rather than approximated.
                        match repetition_phrase(rrule, false) {
                            Some(phrase) => value.push_str(&format!(", {phrase}")),
                            None => value.push_str(&format!(", repeats by the rule {rrule}")),
                        }
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
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let change = edit::trash_series(&snapshot, resolve::series_id(&snapshot, id)?)?;
            record_or(store, &change, "that block is already in the trash")
        })
    }

    /// Puts a task into a block for one sitting. `date` picks the day of a repeating
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
        if minutes == Some(0) {
            return Err(LumennaError::new("a sitting has to be planned for at least a minute"));
        }
        let now = Zoned::now();
        // An occurrence's own identifier, `<series>@<date>`, names its day — what `lum plan
        // friday` lists — so it is not quietly put into today's instead.
        let date = date.or_else(|| block.split_once('@').map(|(_, day)| day.to_owned()));
        self.told(|store| {
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
            // carries them with it. A repeating block's name the day, and are sharded
            // by it; a one-off's follow the series' year.
            let (block_ref, year, day) = if series.is_recurring() {
                (BlockRef::Occurrence(series_id, day), day.year(), day)
            } else {
                (BlockRef::OneOff(series_id), series.start_date.year(), series.start_date)
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
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let change = edit::unassign(&snapshot, id, assignment_year(&snapshot, id)?)?;
            record(store, &change)
        })
    }

    /// Sets how long a sitting is meant to take, or clears it with `None`. What was
    /// logged is left alone.
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`, or `minutes` is zero.
    pub fn plan_minutes(&self, assignment: &str, minutes: Option<u32>) -> Result<Change> {
        if minutes == Some(0) {
            return Err(LumennaError::new("a sitting has to be planned for at least a minute"));
        }
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let change =
                edit::plan_minutes(&snapshot, id, assignment_year(&snapshot, id)?, minutes)?;
            record_or(store, &change, "that is already the plan")
        })
    }

    /// Starts the timer on a sitting.
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`.
    pub fn start_timer(&self, assignment: &str) -> Result<Change> {
        let now = Zoned::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let change =
                edit::start_timer(&snapshot, id, assignment_year(&snapshot, id)?, &now)?;
            if change.is_empty() {
                return Ok(Change::unchanged("that timer is already running"));
            }
            let resumed = snapshot.assignments[&id].status == AssignmentStatus::InProgress;
            store.apply_recorded(&change)?;
            Ok(Change::announced(if resumed { "Resumed timer" } else { "Started timer" }, &change))
        })
    }

    /// Pauses the timer: the time so far is kept and the sitting stays in progress, to be
    /// resumed with [`start_timer`](Self::start_timer) or ended with
    /// [`stop_timer`](Self::stop_timer).
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`.
    pub fn pause_timer(&self, assignment: &str) -> Result<Timer> {
        let now = Zoned::now();
        self.told(|store| {
            store.load_all_years()?;
            let snapshot = repaired(store);
            let id = resolve::assignment_id(&snapshot, assignment)?;
            let year = assignment_year(&snapshot, id)?;
            let sitting = &snapshot.assignments[&id];
            let cap = edit::occurrence_of(&snapshot, sitting.block_ref)
                .map(|occurrence| occurrence.duration_mins)
                .ok()
                .or_else(|| snapshot.series.get(&sitting.block_ref.series_id()).map(|s| s.duration_mins));
            let (change, elapsed) = edit::pause_timer(&snapshot, id, year, cap, &now)?;
            let timer = Timer {
                announcement: change.description.clone(),
                notices: Vec::new(),
                changed: !change.is_empty(),
                assignment: id.to_string(),
                minutes: elapsed.mins,
                capped: elapsed.capped,
            };
            if change.is_empty() {
                return Ok(Timer {
                    announcement: format!(
                        "that timer is not running; {} logged",
                        count_line(elapsed.mins as usize, "minute")
                    ),
                    ..timer
                });
            }
            store.apply_recorded(&change)?;
            Ok(if elapsed.capped {
                timer.note(format!(
                    "that timer ran past the end of its block, so it was capped at {} minutes; if \
                     that is wrong, log the real figure in its place",
                    elapsed.mins
                ))
            } else {
                timer
            })
        })
    }

    /// Stops the timer and logs the minutes — or, with `minutes`, records that figure as the
    /// whole of the sitting instead, which is how time is logged without a timer and how a
    /// capped figure is put right.
    ///
    /// A timer that ran past the end of its block is capped at the block's length and says
    /// so: a truncated figure is not a fact.
    ///
    /// # Errors
    ///
    /// If no assignment matches `assignment`.
    pub fn stop_timer(&self, assignment: &str, minutes: Option<u32>) -> Result<Timer> {
        let now = Zoned::now();
        self.told(|store| {
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

/// What a block can be given beyond its time, length, kind and repetition.
struct Extras<'a> {
    notes: Option<&'a str>,
    accepts_tasks: Option<bool>,
    counts_capacity: Option<bool>,
    anchored: Option<bool>,
    min_minutes: Option<u32>,
    task_filter: Option<&'a str>,
    until: Option<&'a str>,
    colour: Option<&'a str>,
}

/// Sets what `extras` gives on `series`, after its kind, so a flag given here overrides the
/// kind's default rather than being reset by it.
fn apply_extras(snapshot: &Snapshot, series: &mut BlockSeries, extras: &Extras<'_>, now: &Zoned) -> Result<()> {
    if let Some(notes) = extras.notes {
        notes.clone_into(&mut series.notes);
    }
    if let Some(value) = extras.accepts_tasks {
        series.flags.accepts_tasks = value;
    }
    if let Some(value) = extras.counts_capacity {
        series.flags.counts_capacity = value;
    }
    if let Some(value) = extras.anchored {
        series.flags.anchored = value;
    }
    if let Some(minutes) = extras.min_minutes {
        if minutes > series.duration_mins {
            return Err(LumennaError::new(format!(
                "a block of {} cannot be kept to at least {}",
                duration(series.duration_mins),
                duration(minutes)
            )));
        }
        series.min_duration_mins = (minutes > 0).then_some(minutes);
    }
    if let Some(filter) = extras.task_filter.map(str::trim) {
        series.task_filter = if filter.is_empty() {
            None
        } else {
            // Read now, so a filter that means nothing is said when it is set rather than
            // when it is first used.
            resolve::query(snapshot, filter)?;
            Some(filter.to_owned())
        };
    }
    if let Some(until) = extras.until.map(str::trim) {
        if until.eq_ignore_ascii_case("none") {
            if series.is_recurring() {
                series.end_date = None;
            }
        } else {
            if !series.is_recurring() {
                return Err(LumennaError::new(
                    "a block that happens once has no last day; make it repeat first",
                ));
            }
            let day = resolve::date(Some(until), now)?;
            if day < series.start_date {
                return Err(LumennaError::new(format!(
                    "it would end on {day}, before it starts on {}",
                    series.start_date
                )));
            }
            series.end_date = Some(day);
        }
    }
    if let Some(colour) = extras.colour.map(str::trim) {
        series.color = (!colour.is_empty()).then(|| colour.to_lowercase());
    }
    Ok(())
}

/// Free time shorter than this is a seam between two blocks, not time to plan into, and a
/// row for every one would be noise.
const FREE_THRESHOLD_MINS: i64 = 15;

pub(crate) fn block_kind(word: &str) -> Result<BlockKind> {
    match word.to_lowercase().as_str() {
        "work" => Ok(BlockKind::Work),
        "break" => Ok(BlockKind::Break),
        "event" => Ok(BlockKind::Event),
        other => Err(LumennaError::new(format!(
            "'{other}' is not a block kind; use work, break, or event"
        ))),
    }
}

/// A repetition phrase — `every weekday` — as an RFC 5545 rule.
/// A block's repetition as a rule. A block has no completion to count from, so `every!` is
/// refused rather than quietly read as `every`.
fn repetition(phrase: &str) -> Result<String> {
    let (spec, from_completion) = resolve::repetition(phrase)?;
    if from_completion {
        return Err(LumennaError::new(
            "a block repeats on the calendar; 'every!' is for tasks that count from when \
             they are finished",
        ));
    }
    Ok(spec.to_rrule())
}

const fn kind_word(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Work => "work",
        BlockKind::Break => "break",
        BlockKind::Event => "event",
    }
}

fn minutes_of(time: jiff::civil::Time) -> i64 {
    i64::from(time.hour()) * 60 + i64::from(time.minute())
}

fn clock(minutes: i64) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}


/// The day as it is lived: blocks, the free time around them within the day's window, and
/// where now falls.
fn timeline(
    occurrences: &[lumenna_core::recur::Occurrence],
    window: (jiff::civil::Time, jiff::civil::Time),
    now: Option<jiff::civil::Time>,
) -> Vec<PlanItem> {
    let mut items: Vec<(i64, PlanItem)> = Vec::new();
    let (day_start, day_end) = (minutes_of(window.0), minutes_of(window.1));
    let mut free_from = day_start;
    let free = |from: i64, to: i64, items: &mut Vec<(i64, PlanItem)>| {
        if to - from >= FREE_THRESHOLD_MINS {
            items.push((
                from,
                PlanItem::Free {
                    start: clock(from),
                    end: clock(to),
                    minutes: u32::try_from(to - from).unwrap_or(u32::MAX),
                },
            ));
        }
    };
    for (index, occurrence) in occurrences.iter().enumerate() {
        let start = minutes_of(occurrence.start_time);
        // Overlapping blocks leave no gap between them; only time after the latest end so
        // far is free.
        free(free_from, start.min(day_end), &mut items);
        items.push((start, PlanItem::Block { row: to_u32(index + 1) }));
        let end = start + i64::from(occurrence.duration_mins);
        free_from = free_from.max(end);
    }
    free(free_from, day_end, &mut items);

    if let Some(now) = now {
        let at = minutes_of(now);
        // Inside a block, that block is where now is, and says so in its `when`.
        let inside = occurrences.iter().any(|o| {
            let start = minutes_of(o.start_time);
            start <= at && at < start + i64::from(o.duration_mins)
        });
        if !inside {
            let position = items.iter().position(|(start, _)| *start > at).unwrap_or(items.len());
            items.insert(position, (at, PlanItem::Now { time: clock(at) }));
        }
    }
    items.into_iter().map(|(_, item)| item).collect()
}

/// The day's opening summary row, in words.
fn summary(blocks: &[PlanBlock], overdue: Option<usize>) -> String {
    // The flag, not the kind: a kind only presets it, and any block can be set apart.
    let work: u32 =
        blocks.iter().filter(|b| b.counts_capacity).map(|b| b.duration_mins).sum();
    let assigned: usize = blocks.iter().map(|b| b.assignments.len()).sum();
    if blocks.is_empty() {
        return "No blocks".to_owned();
    }
    let mut parts = vec![count_line(blocks.len(), "block")];
    if work > 0 {
        parts.push(format!("{} of work", duration(work)));
    }
    parts.push(format!("{} assigned", count_line(assigned, "task")));
    if let Some(overdue) = overdue.filter(|n| *n > 0) {
        parts.push(format!("{overdue} overdue"));
    }
    let sentence = parts.join(", ");
    sentence[..1].to_uppercase() + &sentence[1..]
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
