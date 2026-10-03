//! Time blocks, their exceptions, and the assignments that join them to tasks
//! (§3.6, §3.7).

use jiff::civil;
use jiff::{SignedDuration, Timestamp};

use super::{ExternalRef, ModelError, TzName};
use crate::id::{AssignmentId, SeriesId, TaskId};
use crate::order::OrderKey;

/// A block of time, one-off or recurring.
///
/// **One type for both**, distinguished by [`rrule`](Self::rrule). This avoids two
/// near-identical shapes and lets a one-off block become recurring without changing type.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockSeries {
    /// Identity.
    pub id: SeriesId,
    /// Display name.
    pub title: String,
    /// Automerge `Text` in the document.
    pub notes: String,
    /// The preset this block was made from. Behaviour comes from [`flags`](Self::flags),
    /// which the preset only seeds.
    pub kind: BlockKind,
    /// What the core is allowed to do with it.
    pub flags: BlockFlags,
    /// Wall-clock start.
    pub start_time: civil::Time,
    /// **Duration, not end time.** It cannot be negative, cannot be inverted, and survives
    /// DST transitions cleanly (§3.6).
    pub duration_mins: u32,
    /// How far re-flow may compress this block. See [`BlockKind::default_min_duration`] for
    /// what this defends against and why the default depends on the kind.
    pub min_duration_mins: Option<u32>,
    /// First day the block occurs.
    pub start_date: civil::Date,
    /// Last day it occurs, if the series ends.
    pub end_date: Option<civil::Date>,
    /// An RFC 5545 `RRULE`. Absent for a one-off block.
    pub rrule: Option<String>,
    /// Normally absent: a 9am block is 9am after you fly to another continent. Set it only
    /// for genuinely anchored blocks (§3.6).
    pub timezone: Option<TzName>,
    /// Presentation only.
    pub color: Option<String>,
    /// Presentation only.
    pub icon: Option<String>,
    /// A filter expression scoping which tasks the core will suggest here — "this is my
    /// `#work` block, don't offer me personal errands" (§10).
    ///
    /// This is what makes auto-suggestion useful rather than noisy, and it is why the
    /// filter query language is load-bearing rather than a nice-to-have.
    pub task_filter: Option<String>,
    /// Set when imported from a calendar.
    ///
    /// A day planner that does not know about real meetings will schedule work on top of
    /// them and report fictional capacity, which is why import is close to table stakes.
    pub external: Option<ExternalRef>,
    /// Trash, for undo.
    pub deleted_at: Option<Timestamp>,
}

impl BlockSeries {
    /// A one-off block on a single day.
    ///
    /// # Errors
    ///
    /// [`ModelError::ZeroDuration`] if the block does not last at least a minute.
    pub fn one_off(
        title: impl Into<String>,
        kind: BlockKind,
        date: civil::Date,
        start_time: civil::Time,
        duration_mins: u32,
    ) -> Result<Self, ModelError> {
        if duration_mins == 0 {
            return Err(ModelError::ZeroDuration);
        }
        Ok(Self {
            id: SeriesId::new(),
            title: title.into(),
            notes: String::new(),
            kind,
            flags: kind.default_flags(),
            start_time,
            duration_mins,
            min_duration_mins: None,
            start_date: date,
            end_date: Some(date),
            rrule: None,
            timezone: None,
            color: None,
            icon: None,
            task_filter: None,
            external: None,
            deleted_at: None,
        })
    }

    /// Sets a compression floor, overriding the kind's default.
    ///
    /// # Errors
    ///
    /// [`ModelError::MinimumExceedsDuration`] if the floor is longer than the block.
    pub fn with_min_duration(mut self, mins: u32) -> Result<Self, ModelError> {
        if mins > self.duration_mins {
            return Err(ModelError::MinimumExceedsDuration {
                min_duration_mins: mins,
                duration_mins: self.duration_mins,
            });
        }
        self.min_duration_mins = Some(mins);
        Ok(self)
    }

    /// How short re-flow may make this block: the per-block override if set, otherwise the
    /// kind's default.
    #[must_use]
    pub fn compression_floor_mins(&self) -> u32 {
        self.min_duration_mins
            .filter(|&m| m <= self.duration_mins)
            .unwrap_or_else(|| self.kind.default_min_duration(self.duration_mins))
    }

    /// Whether the block repeats.
    #[must_use]
    pub const fn is_recurring(&self) -> bool {
        self.rrule.is_some()
    }
}

/// A preset that seeds [`BlockFlags`].
///
/// Kind exists only where the *core* behaves differently; everything else is presentation.
/// There is deliberately no `Custom(String)` variant — a free-text kind the core cannot
/// reason about is a title with extra steps (§3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BlockKind {
    /// Accepts tasks, counts as capacity, movable. The core concept.
    #[default]
    Work,
    /// No tasks, no capacity, movable. Meals, rest, sleep, commute.
    Break,
    /// No tasks, no capacity, anchored. Meetings, appointments, classes.
    Event,
}

impl BlockKind {
    /// The flags this preset expands to. They remain individually overridable: a commute is
    /// a `Break`, but someone who works on the train wants it to accept tasks, and
    /// "protected admin time" is anchored *and* accepts tasks, which no preset covers.
    #[must_use]
    pub const fn default_flags(self) -> BlockFlags {
        match self {
            Self::Work => {
                BlockFlags { accepts_tasks: true, counts_capacity: true, anchored: false }
            }
            Self::Break => {
                BlockFlags { accepts_tasks: false, counts_capacity: false, anchored: false }
            }
            Self::Event => {
                BlockFlags { accepts_tasks: false, counts_capacity: false, anchored: true }
            }
        }
    }

    /// How short re-flow may make a block of this kind.
    ///
    /// This is what stops re-flow destroying a block in order to save the schedule. When
    /// the day slips, §10.1 may shrink later blocks to absorb the overrun — but whether
    /// shrinking is acceptable depends entirely on what the block is *for*. A work block
    /// cut from 90 minutes to 60 still does work. A fifteen-minute break cut to four is not
    /// a break; it has been deleted while appearing to survive, which is worse than being
    /// told it does not fit.
    ///
    /// - **Break** — incompressible. A shortened break is a failed break; it moves instead.
    /// - **Work** — compressible to about half, floored at fifteen minutes, below which the
    ///   context switch costs more than the block yields.
    /// - **Event** — irrelevant, since anchored blocks are never re-flowed at all.
    #[must_use]
    pub const fn default_min_duration(self, duration_mins: u32) -> u32 {
        match self {
            Self::Work => {
                let half = duration_mins / 2;
                let floor = if half > 15 { half } else { 15 };
                if floor > duration_mins { duration_mins } else { floor }
            }
            Self::Break | Self::Event => duration_mins,
        }
    }
}

/// The behaviourally meaningful axes of a block. Orthogonal, hence flags rather than more
/// kinds (§3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockFlags {
    /// Whether tasks can be assigned into it.
    pub accepts_tasks: bool,
    /// Whether it contributes to "hours available for work today".
    pub counts_capacity: bool,
    /// Fixed in time; cannot be shifted when the day slips.
    ///
    /// Operationally the most important of the three: it is what lets the core do anything
    /// sensible when the day runs late (§10.1), by determining which blocks can absorb a
    /// slip.
    pub anchored: bool,
}

/// A single occurrence that differs from its series (§3.6).
///
/// **Sparse.** Unmodified occurrences are never stored; they are expanded from the rule at
/// read time. Only edits and cancellations become records. Without this a daily routine
/// generates thousands of rows and every sync becomes a bulk transfer.
///
/// This is also where "I finished early" and "I ran over" live. Occurrences have no
/// lifecycle state — see [`BlockAssignment`] for why — so finishing at 10:40 instead of
/// 11:00 is a [`ExceptionAction::Modified`] with a shorter duration. That is honest: you
/// genuinely changed that day's schedule, and it should persist and sync like any other
/// change.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockException {
    /// Which series.
    pub series_id: SeriesId,
    /// Which occurrence, by the date the rule would have placed it on.
    pub original_date: civil::Date,
    /// What changed.
    pub action: ExceptionAction,
}

/// What a [`BlockException`] does to its occurrence.
#[derive(Debug, Clone, PartialEq)]
pub enum ExceptionAction {
    /// The occurrence does not happen.
    ///
    /// Its reminders must be suppressed too (§3.8) — easy to miss, and very annoying when
    /// missed.
    Cancelled,
    /// The occurrence happens differently. Every field is an override; `None` keeps the
    /// series' value.
    Modified {
        /// Overrides the series' start time.
        start_time: Option<civil::Time>,
        /// Overrides the series' duration.
        duration_mins: Option<u32>,
        /// Overrides the series' title.
        title: Option<String>,
        /// Overrides the series' kind.
        kind: Option<BlockKind>,
        /// Overrides the series' flags.
        flags: Option<BlockFlags>,
    },
}

impl ExceptionAction {
    /// A modification with nothing overridden yet.
    #[must_use]
    pub const fn modified() -> Self {
        Self::Modified {
            start_time: None,
            duration_mins: None,
            title: None,
            kind: None,
            flags: None,
        }
    }
}

/// Which block an assignment points at.
///
/// An enum because a recurring block's occurrence has **no stable identifier** until it is
/// excepted. Assigning a task to next Tuesday's instance of a daily focus block means
/// referencing the series and that date. This is the main reason the exception model must
/// exist rather than being an optimization (§3.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlockRef {
    /// A block that happens once.
    OneOff(SeriesId),
    /// One occurrence of a recurring block.
    Occurrence(SeriesId, civil::Date),
}

impl BlockRef {
    /// The series either way.
    #[must_use]
    pub const fn series_id(&self) -> SeriesId {
        match *self {
            Self::OneOff(id) | Self::Occurrence(id, _) => id,
        }
    }

    /// The occurrence date, for a recurring block.
    #[must_use]
    pub const fn date(&self) -> Option<civil::Date> {
        match *self {
            Self::OneOff(_) => None,
            Self::Occurrence(_, date) => Some(date),
        }
    }
}

/// A task placed into a block: the join between the two halves of the app (§3.7).
///
/// # Why occurrences have no lifecycle state
///
/// **Blocks are not timers.** Nothing is ever started or ended by the user. A block occupies
/// its scheduled window and leaves it, and whether that window contains `now` is derived
/// from the clock, never stored. Three reasons, the last decisive:
///
/// 1. A punch clock is friction people forget, and a forgotten "end block" leaves data worse
///    than no data — an occurrence that appears to have run for nineteen hours.
/// 2. Actual effort is already recorded here, in [`running_since`](Self::running_since) and
///    [`accumulated_mins`](Self::accumulated_mins). A block-level actual-start would be a
///    second clock that could disagree with the first, and then something has to decide
///    which one is true.
/// 3. **A lifecycle state machine cannot survive CRDT merge.** Two devices starting the same
///    occurrence, or one ending it while another extends it, produce states with no sensible
///    merge and no correct answer. Derived-from-clock has nothing to converge, so it cannot
///    conflict.
///
/// # Why the timer stores a fact rather than a counter
///
/// Elapsed time is derived, so nothing ticks in storage and nothing is written every second.
/// Running state is the most conflict-prone thing in the model, and this makes the conflicts
/// harmless: two devices starting a timer is last-write-wins on one timestamp. The real
/// failure mode is an *orphaned* timer — started on a phone that then died — which
/// [`elapsed`](Self::elapsed) reports rather than silently logging fourteen hours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockAssignment {
    /// Identity.
    pub id: AssignmentId,
    /// Which block, and which occurrence of it.
    pub block_ref: BlockRef,
    /// Which task. May reference a task this device's `core` document has not seen yet
    /// (§3.1).
    pub task_id: TaskId,
    /// How long this sitting is meant to take.
    pub planned_mins: Option<u32>,
    /// Completed timer runs, plus anything entered by hand. The timer is optional, not
    /// mandatory.
    pub accumulated_mins: u32,
    /// `Some` exactly while a timer is running.
    pub running_since: Option<Timestamp>,
    /// How this sitting went.
    pub status: AssignmentStatus,
    /// Position within the block.
    pub order: OrderKey,
    /// When the assignment was made.
    pub created_at: Timestamp,
}

impl AssignmentStatus {
    /// How to say it. Two words where it is two words — `{:?}` would give `InProgress`,
    /// and lowercasing that gives `inprogress`, which is not a phrase.
    #[must_use]
    pub const fn speech(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::InProgress => "in progress",
            Self::Worked => "worked",
            Self::Skipped => "skipped",
            Self::Deferred => "deferred",
        }
    }
}

impl BlockAssignment {
    /// Places a task into a block.
    #[must_use]
    pub fn new(block_ref: BlockRef, task_id: TaskId, order: OrderKey) -> Self {
        Self {
            id: AssignmentId::new(),
            block_ref,
            task_id,
            planned_mins: None,
            accumulated_mins: 0,
            running_since: None,
            status: AssignmentStatus::Planned,
            order,
            created_at: crate::time::now(),
        }
    }

    /// Whether a timer is running.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running_since.is_some()
    }

    /// Starts the timer, if it is not already running.
    pub fn start(&mut self, now: Timestamp) {
        if self.running_since.is_none() {
            self.running_since = Some(now);
            self.status = AssignmentStatus::InProgress;
        }
    }

    /// Stops the timer, folding the running interval into
    /// [`accumulated_mins`](Self::accumulated_mins).
    ///
    /// `cap_mins` is the containing block's duration, as for [`elapsed`](Self::elapsed): a
    /// timer left running past the end of its block contributes at most the block's length.
    pub fn pause(&mut self, now: Timestamp, cap_mins: Option<u32>) -> Elapsed {
        let elapsed = self.elapsed(now, cap_mins);
        self.accumulated_mins = elapsed.mins;
        self.running_since = None;
        elapsed
    }

    /// Time spent on this sitting: accumulated, plus the running interval if there is one.
    ///
    /// `cap_mins` should be the containing block's duration. An orphaned timer — started on
    /// a device that then died — would otherwise report the wall-clock time since, so the
    /// total is capped and [`Elapsed::capped`] is set. Callers must **confirm with the user
    /// rather than record the capped figure silently**; a truncated number presented as fact
    /// is its own kind of wrong.
    ///
    /// A clock that has gone backwards (`now` before the start) contributes nothing rather
    /// than subtracting.
    #[must_use]
    pub fn elapsed(&self, now: Timestamp, cap_mins: Option<u32>) -> Elapsed {
        let running_mins = self.running_since.map_or(0, |since| {
            let secs = now.duration_since(since).max(SignedDuration::ZERO).as_secs();
            u32::try_from(secs / 60).unwrap_or(u32::MAX)
        });
        let raw = self.accumulated_mins.saturating_add(running_mins);
        match cap_mins {
            Some(cap) if raw > cap => Elapsed { mins: cap, capped: true },
            _ => Elapsed { mins: raw, capped: false },
        }
    }
}

/// The result of [`BlockAssignment::elapsed`].
///
/// **Accessibility constraint (§3.7):** this must never be put in a live region or an
/// announcement channel. A continuously updating value makes a screen reader announce
/// constantly and renders the screen unusable. Expose it as *polled on demand* — a
/// keystroke or button that reports "43 minutes elapsed, 17 remaining against your
/// estimate."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Elapsed {
    /// Minutes spent.
    pub mins: u32,
    /// Set when the figure was truncated to the block's duration, meaning the timer has
    /// almost certainly been orphaned. Ask before recording it.
    pub capped: bool,
}

/// How one sitting went (§3.7).
///
/// Completion lives on the [`Task`](super::Task); this is the assignment's own state. A task
/// worked on Monday but not finished gets `Worked` on Monday's assignment and a *fresh*
/// assignment on Tuesday. Both persist, and over time this accumulates a genuine work
/// history — how many sittings something took, where estimates were wrong — which
/// Structured discards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum AssignmentStatus {
    /// Placed into the block, not yet started.
    #[default]
    Planned,
    /// A timer is running, or the user says they are on it.
    InProgress,
    /// Time went into it. The task itself may or may not be finished.
    Worked,
    /// The sitting did not happen and is not being carried forward.
    Skipped,
    /// The sitting did not happen and has been moved to another block.
    Deferred,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i16, m: i8, d: i8) -> civil::Date {
        civil::date(y, m, d)
    }

    #[test]
    fn presets_expand_to_the_documented_flags() {
        assert!(BlockKind::Work.default_flags().accepts_tasks);
        assert!(BlockKind::Work.default_flags().counts_capacity);
        assert!(!BlockKind::Work.default_flags().anchored);
        assert!(!BlockKind::Break.default_flags().anchored);
        assert!(BlockKind::Event.default_flags().anchored);
    }

    #[test]
    fn breaks_are_incompressible_and_work_compresses_to_half() {
        assert_eq!(BlockKind::Break.default_min_duration(15), 15);
        assert_eq!(BlockKind::Work.default_min_duration(90), 45);
        // Floored at fifteen minutes: half of twenty is not worth the context switch.
        assert_eq!(BlockKind::Work.default_min_duration(20), 15);
        // ...but the floor can never exceed the block itself.
        assert_eq!(BlockKind::Work.default_min_duration(10), 10);
    }

    #[test]
    fn per_block_override_beats_the_kind_default() {
        let walk = BlockSeries::one_off(
            "Walk the dog",
            BlockKind::Break,
            date(2026, 9, 1),
            civil::time(12, 0, 0, 0),
            20,
        )
        .unwrap();
        assert_eq!(walk.compression_floor_mins(), 20, "breaks default to incompressible");

        let walk = walk.with_min_duration(5).unwrap();
        assert_eq!(walk.compression_floor_mins(), 5);

        assert!(matches!(
            walk.with_min_duration(999),
            Err(ModelError::MinimumExceedsDuration { .. })
        ));
    }

    #[test]
    fn zero_duration_blocks_are_rejected() {
        let err = BlockSeries::one_off(
            "Nothing",
            BlockKind::Work,
            date(2026, 9, 1),
            civil::time(9, 0, 0, 0),
            0,
        );
        assert!(matches!(err, Err(ModelError::ZeroDuration)));
    }

    fn assignment() -> BlockAssignment {
        BlockAssignment::new(
            BlockRef::Occurrence(SeriesId::new(), date(2026, 9, 1)),
            TaskId::new(),
            OrderKey::middle(),
        )
    }

    #[test]
    fn elapsed_derives_from_the_running_fact() {
        let mut a = assignment();
        let t0 = Timestamp::from_second(1_700_000_000).unwrap();
        assert_eq!(a.elapsed(t0, None).mins, 0);

        a.start(t0);
        assert!(a.is_running());
        assert_eq!(a.status, AssignmentStatus::InProgress);

        let t1 = t0 + SignedDuration::from_secs(43 * 60);
        assert_eq!(a.elapsed(t1, None).mins, 43);

        // Nothing has been written; the fact is still just the start time.
        assert_eq!(a.accumulated_mins, 0);

        a.pause(t1, None);
        assert!(!a.is_running());
        assert_eq!(a.accumulated_mins, 43);

        // A second sitting adds to the first.
        let t2 = t1 + SignedDuration::from_secs(60 * 60);
        a.start(t2);
        assert_eq!(a.elapsed(t2 + SignedDuration::from_secs(7 * 60), None).mins, 50);
    }

    #[test]
    fn an_orphaned_timer_is_capped_and_flagged() {
        let mut a = assignment();
        let t0 = Timestamp::from_second(1_700_000_000).unwrap();
        a.start(t0);
        let next_day = t0 + SignedDuration::from_hours(19);

        let uncapped = a.elapsed(next_day, None);
        assert_eq!(uncapped.mins, 19 * 60);
        assert!(!uncapped.capped);

        let capped = a.elapsed(next_day, Some(90));
        assert_eq!(capped.mins, 90);
        assert!(capped.capped, "callers must confirm rather than log nineteen hours");
    }

    #[test]
    fn a_backwards_clock_does_not_subtract_time() {
        let mut a = assignment();
        let t0 = Timestamp::from_second(1_700_000_000).unwrap();
        a.accumulated_mins = 30;
        a.start(t0);
        let earlier = t0 - SignedDuration::from_hours(2);
        assert_eq!(a.elapsed(earlier, None).mins, 30);
    }

    #[test]
    fn starting_a_running_timer_does_not_reset_it() {
        let mut a = assignment();
        let t0 = Timestamp::from_second(1_700_000_000).unwrap();
        a.start(t0);
        a.start(t0 + SignedDuration::from_secs(600));
        assert_eq!(a.running_since, Some(t0));
    }
}
