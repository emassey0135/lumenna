//! The day: blocks, what is assigned to them, and the time spent (§3.7).

use jiff::Zoned;
use lumenna_core::edit::{self, EditError};
use lumenna_core::id::AssignmentId;
use lumenna_core::model::{BlockAssignment, BlockKind, BlockRef, BlockSeries, ExceptionAction};
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::{LumennaError, Result};
use crate::resolve;
use crate::tasks::{record, record_or};
use crate::types::{
    Announced, BlockEdit, BlockScope, BlockShown, Change, NewBlock, Plan, PlanAssignment, PlanBlock, PlanItem,
    Rows, Timer,
};
use crate::words::{count_line, duration, time_text};
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

                let when = if day != now.date() {
                    ""
                } else if occurrence.end_time() <= now.time() {
                    "past"
                } else if occurrence.start_time <= now.time() {
                    "now"
                } else {
                    "upcoming"
                };
                blocks.push(PlanBlock {
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
                });
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
            Ok(Plan {
                announcement: format!("{day}, {}", count_line(blocks.len(), "block")),
                notices: Vec::new(),
                date: day.to_string(),
                count: to_u32(blocks.len()),
                summary: summary(&blocks, overdue),
                timeline,
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

    /// Changes a block: every occurrence, or only the one on a given day (§4.3 — always
    /// asked of a repeating block, never guessed).
    ///
    /// A day's change is an exception; the series stays as it was, and later changes to the
    /// series leave the overridden fields alone (§3.6).
    ///
    /// # Errors
    ///
    /// If no block matches `id`, a field cannot be read, the day is not one the block
    /// happens on, a single day is asked to repeat differently, or a block that happens once
    /// is asked about a single day.
    pub fn edit_block(&self, id: &str, edit: BlockEdit, scope: BlockScope) -> Result<Change> {
        let now = Zoned::now();
        self.with(|store| {
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
                            // A series anchored on a day it never occurs is a trap (§5).
                            let rule = lumenna_core::recur::Rule::parse(&rrule)?;
                            if let Some(first) = rule.first_from(after.start_date)? {
                                after.start_date = first;
                            }
                            after.rrule = Some(rrule);
                            after.end_date = None;
                        }
                    }
                    edit::update_series(before, after)
                }
                BlockScope::Occurrence { date } => {
                    if edit.repeat.is_some() {
                        return Err(LumennaError::new(
                            "one day of a block cannot repeat differently; change every \
                             occurrence to change how it repeats",
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
                    let action = ExceptionAction::Modified {
                        start_time: start.or(earlier.0),
                        duration_mins: edit.minutes.or(earlier.1),
                        title: edit.title.or(earlier.2),
                        kind: kind.or(earlier.3),
                        flags: kind.map(BlockKind::default_flags).or(earlier.4),
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
        self.with(|store| {
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
        self.with(|store| {
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
        self.with(|store| {
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
            })
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

/// Free time shorter than this is a seam between two blocks, not time to plan into, and a
/// row for every one would be noise (§13).
const FREE_THRESHOLD_MINS: i64 = 15;

fn block_kind(word: &str) -> Result<BlockKind> {
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
fn repetition(phrase: &str) -> Result<String> {
    let tokens = lumenna_parse::words(phrase);
    let (spec, _, _) = lumenna_parse::date::parse_recurrence(&tokens, 0).ok_or_else(|| {
        LumennaError::new(format!("could not read a repetition from '{phrase}'"))
    })?;
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
/// where now falls (§13).
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

/// §13's opening summary row, in words.
fn summary(blocks: &[PlanBlock], overdue: Option<usize>) -> String {
    let work: u32 =
        blocks.iter().filter(|b| b.kind == "work").map(|b| b.duration_mins).sum();
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
