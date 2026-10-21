//! The day as it is lived, and the blocks it is made of: a function of the snapshot and the
//! moment, for every screen that shows a day and for `lum plan`.

use jiff::Zoned;
use lumenna_core::dayplan::SHORTEST_SITTING;
use lumenna_core::edit::EditError;
use lumenna_core::model::{AssignmentStatus, BlockAssignment, BlockKind, BlockSeries, ExceptionAction};
use lumenna_core::recur::Occurrence;
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;

use crate::error::Result;
use crate::resolve;
use crate::types::{TaskDetail, TaskShown, WorkBlock, WorkBlocks, BlockShown, CancelledBlock, Plan, PlanAssignment, PlanBlock, PlanItem, Rows, repetition_phrase};
use crate::words::{block_details, count_line, duration, sitting_details, time_text, to_u32};


/// A day's blocks and what is assigned to them, as it stands at `now`: what a day's screen
/// shows and `lum plan` says.
///
/// # Errors
///
/// If a block's rule cannot be expanded.
pub fn plan(snapshot: &Snapshot, day: jiff::civil::Date, now: &Zoned) -> Result<Plan> {
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
                    actions: Vec::new(),
                };
                sitting.details = sitting_details(&sitting);
                sitting
            })
            .collect();

        let when = if day != now.date() {
            ""
        } else if occurrence.is_over_at(now.time()) {
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
            timezone: series.and_then(|s| s.timezone.as_ref()).map(ToString::to_string),
            notes: series.map(|s| s.notes.clone()).unwrap_or_default(),
            details: Vec::new(),
            actions: Vec::new(),
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
            .filter(|t| facts.notable_states_of(t, now).contains(&lumenna_core::State::Overdue))
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
            details: vec!["cancelled for this day".to_owned()],
            actions: Vec::new(),
        })
        .collect();
    cancelled.sort_by(|a, b| (&a.start, &a.title).cmp(&(&b.start, &b.title)));
    let mut plan = Plan {
        announcement: format!("{day}, {}", count_line(blocks.len(), "block")),
        notices: Vec::new(),
        date: day.to_string(),
        count: to_u32(blocks.len()),
        summary: summary(&blocks, overdue),
        timeline,
        blocks,
        cancelled,
        actions: Vec::new(),
    };
    let carries = crate::day::carries(snapshot, &plan, now);
    let what_now = what_now_offered(snapshot, now);
    crate::actions::fill_plan(&mut plan, &carries, what_now);
    Ok(plan)
}

/// One block series: what an editor starts from.
///
/// # Errors
///
/// If no block matches `id`.
pub fn block_shown(snapshot: &Snapshot, id: &str) -> Result<BlockShown> {
    let series_id = resolve::series_id(snapshot, id)?;
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
}

/// Every block series, by when it starts.
///
/// # Errors
///
/// Never; a `Result` as every listing is.
pub fn blocks(snapshot: &Snapshot, today: jiff::civil::Date) -> Result<Rows> {
    let mut live: Vec<&BlockSeries> =
        snapshot.series.values().filter(|s| s.deleted_at.is_none()).collect();
    live.sort_by_key(|s| (s.start_date, s.start_time, s.id));
    let count = to_u32(live.len());
    let rows: Vec<Row> = live
        .iter()
        .enumerate()
        .map(|(index, series)| {
            // When, as a task's due date is said: in words, with the start as
            // `due_time` for the client's clock — "every weekday at 9:00 AM" — and
            // never digits a screen reader reads as numbers.
            let day = lumenna_core::time::day_words(series.start_date, today);
            let starts_later = series.start_date > today;
            let (when, mut value) = match &series.rrule {
                // In words where the grammar can say it; a rule from outside is
                // given as it is rather than approximated.
                Some(rrule) => (
                    repetition_phrase(rrule, false).unwrap_or_else(|| format!("repeats by the rule {rrule}")),
                    vec![crate::words::duration(series.duration_mins)],
                ),
                None if matches!(day.as_str(), "today" | "tomorrow" | "yesterday") => {
                    (day.clone(), vec![crate::words::duration(series.duration_mins)])
                }
                None => (format!("on {day}"), vec![crate::words::duration(series.duration_mins)]),
            };
            if series.rrule.is_some() && starts_later {
                value.push(format!("from {day}"));
            }
            let value = value.join(", ");
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
                due: Some(when),
                due_time: Some(series.start_time),
                value: Some(value),
                hint: None,
            }
        })
        .collect();
    let mut rows = Rows::new(&rows, "block");
    crate::actions::fill_rows(&mut rows, snapshot);
    Ok(rows)
}

/// Free time shorter than this is a seam between two blocks, not time to plan into, and a
/// row for every one would be noise.
const FREE_THRESHOLD_MINS: i64 = 15;

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
                    title: "Free".to_owned(),
                    details: vec![crate::words::duration(u32::try_from(to - from).unwrap_or(u32::MAX))],
                    actions: Vec::new(),
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
            items.insert(position, (at, PlanItem::Now { time: clock(at), title: "Now".to_owned() }));
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

/// A block kind as its word: work, break or event.
pub const fn kind_word(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Work => "work",
        BlockKind::Break => "break",
        BlockKind::Event => "event",
    }
}

/// Minutes since midnight.
pub fn minutes_of(time: jiff::civil::Time) -> i64 {
    i64::from(time.hour()) * 60 + i64::from(time.minute())
}

/// Minutes since midnight as `HH:MM`.
pub fn clock(minutes: i64) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// Where now falls in today: in a block, or in the free time before the next one.
pub enum Now {
    /// A block under way, and the minutes left of it.
    Block(Occurrence, u32),
    /// Free time: its minutes until the next block or the day's end, and whether a block is next.
    Free(u32, bool),
}

/// Where `now` falls in its day.
pub fn where_now(snapshot: &Snapshot, now: &Zoned) -> Result<Now> {
    let at = minutes_of(now.time());
    let mut day = snapshot.day(now.date())?;
    day.sort_by_key(|o| o.start_time);
    if let Some(current) = day.iter().find(|o| {
        let start = minutes_of(o.start_time);
        start <= at && at < start + i64::from(o.duration_mins)
    }) {
        let left = minutes_of(current.start_time) + i64::from(current.duration_mins) - at;
        return Ok(Now::Block(current.clone(), u32::try_from(left).unwrap_or(0)));
    }
    let next = day.iter().map(|o| minutes_of(o.start_time)).filter(|start| *start > at).min();
    // With nothing after, free time runs to the day's end, or past it to midnight.
    let window = minutes_of(snapshot.settings.day_window.1);
    let end = next.unwrap_or(if at < window { window } else { 24 * 60 });
    Ok(Now::Free(u32::try_from(end - at).unwrap_or(0), next.is_some()))
}

/// Whether today offers the question now: not inside a block that takes no tasks.
pub fn what_now_offered(snapshot: &Snapshot, now: &Zoned) -> bool {
    match where_now(snapshot, now) {
        Ok(Now::Block(occurrence, _)) => occurrence.flags.accepts_tasks,
        Ok(Now::Free(minutes, _)) => minutes >= SHORTEST_SITTING,
        Err(_) => false,
    }
}

/// Everything about one task.
///
/// # Errors
///
/// If no task matches `id`.
pub fn task_shown(snapshot: &Snapshot, id: &str, now: &Zoned) -> Result<TaskShown> {
    let id = resolve::task_id(snapshot, id)?;
    let task = snapshot.tasks.get(&id).ok_or(EditError::NotFound { kind: "task" })?;
    let task = TaskDetail::of(task, snapshot, now);
    Ok(TaskShown { announcement: task.title.clone(), notices: Vec::new(), task })
}

/// How far ahead putting a task in a block looks when asked from the task: a week, so the
/// list stays short enough to choose from by ear. The planner reaches any other day.
pub const WORK_BLOCK_DAYS: u32 = 7;

/// The work blocks a task could go in over `days` days from `first`, at most 31: breaks and
/// events left out, since only work blocks take tasks.
///
/// # Errors
///
/// If a block's rule cannot be expanded.
pub fn work_blocks(snapshot: &Snapshot, first: jiff::civil::Date, days: u32, now: &Zoned) -> Result<WorkBlocks> {
    let days = days.clamp(1, 31);
    let mut blocks = Vec::new();
    for offset in 0..days {
        let Ok(day) = first.checked_add(jiff::Span::new().days(i64::from(offset))) else { break };
        let plan = plan(snapshot, day, now)?;
        blocks.extend(plan.blocks.into_iter().filter(|block| block.accepts_tasks).map(|block| WorkBlock {
            assigned: to_u32(block.assignments.len()),
            id: block.id,
            date: plan.date.clone(),
            title: block.title,
            start: block.start,
            end: block.end,
            duration_mins: block.duration_mins,
            when: block.when,
        }));
    }
    Ok(WorkBlocks {
        announcement: format!("{} over {}", count_line(blocks.len(), "work block"), count_line(days as usize, "day")),
        notices: Vec::new(),
        from: first.to_string(),
        days,
        blocks,
    })
}

/// How sittings name an occurrence: a day of a repeating block, or a one-off block itself.
#[must_use]
pub fn block_ref(snapshot: &Snapshot, occurrence: &Occurrence) -> lumenna_core::model::BlockRef {
    match snapshot.series.get(&occurrence.series_id) {
        Some(series) if series.is_recurring() => lumenna_core::model::BlockRef::Occurrence(occurrence.series_id, occurrence.date),
        _ => lumenna_core::model::BlockRef::OneOff(occurrence.series_id),
    }
}
