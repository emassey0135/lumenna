//! User preferences (§3.10).

use jiff::civil;

use super::{ReminderAnchor, Trigger};

/// Preferences, synced.
///
/// These are decisions about the person, not about the machine, so they belong in a
/// document. Device-specific choices — muting all reminders on one laptop, which tree nodes
/// are expanded — are local-only state (§3.12) and are deliberately not here.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Settings {
    /// Whether completing a parent completes its subtasks.
    ///
    /// Because this can change, completions record what caused them rather than deriving it
    /// (§3.3).
    pub cascade_complete_subtasks: bool,
    /// How much the core says when it announces something.
    ///
    /// A **core** setting, not a per-UI one, because the core generates the announcement
    /// text — one setting, honoured identically on all eleven targets (§13).
    pub verbosity: Verbosity,
    /// Offered in the UI for new tasks.
    pub default_task_reminders: Vec<Trigger>,
    /// Offered in the UI for new blocks.
    pub default_block_reminders: Vec<Trigger>,
    /// What a date-only due date means when a reminder needs a time to count back from
    /// (§3.8).
    pub all_day_reminder_hour: civil::Time,
    /// Waking hours. Bounds re-flow: the planner may not push a block past the end of your
    /// day to make the arithmetic work (§10.1).
    pub day_window: (civil::Time, civil::Time),
    /// Which day a week starts on.
    #[cfg_attr(feature = "serde", serde(with = "weekday"))]
    pub week_start: civil::Weekday,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cascade_complete_subtasks: true,
            verbosity: Verbosity::Full,
            default_task_reminders: Vec::new(),
            default_block_reminders: vec![Trigger::before(ReminderAnchor::BlockStart, 5)],
            all_day_reminder_hour: civil::time(9, 0, 0, 0),
            day_window: (civil::time(8, 0, 0, 0), civil::time(22, 0, 0, 0)),
            week_start: civil::Weekday::Monday,
        }
    }
}

/// How much detail announcements carry (§13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Verbosity {
    /// *"Review PR, overdue."*
    Terse,
    /// The full sentence every time: *"2:00 PM, one hour, Deep work, work block, three
    /// tasks assigned."*
    #[default]
    Full,
}

/// `jiff` serializes dates and times but not weekdays, so a weekday is written as its number,
/// Monday being one.
#[cfg(feature = "serde")]
mod weekday {
    use jiff::civil::Weekday;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(day: &Weekday, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i8(day.to_monday_one_offset())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Weekday, D::Error> {
        let number = i8::deserialize(d)?;
        Weekday::from_monday_one_offset(number).map_err(serde::de::Error::custom)
    }
}
