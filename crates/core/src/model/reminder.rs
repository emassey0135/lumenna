//! Reminders and their acknowledgements (§3.8, §3.9).

use std::collections::BTreeSet;

use jiff::civil;
use jiff::{Timestamp, Zoned};

use super::ModelError;
use crate::id::{NodeId, ReminderId, SeriesId, TaskId};

/// What a reminder is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReminderTarget {
    /// A task, anchored to its due datetime.
    Task(TaskId),
    /// A block series, anchored to the start or end of each occurrence.
    Block(SeriesId),
}

/// What a reminder's offset is measured from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReminderAnchor {
    /// The task's due datetime.
    ///
    /// Requires the task's `due.time` to be set. For a date-only due date, resolve against
    /// [`Settings::all_day_reminder_hour`](super::Settings::all_day_reminder_hour) —
    /// otherwise "30 minutes before" has no referent (§3.8).
    Due,
    /// The start of a block occurrence.
    BlockStart,
    /// The end of a block occurrence.
    BlockEnd,
    /// A fixed moment, independent of the target.
    Absolute(Zoned),
}

impl ReminderAnchor {
    /// Whether this anchor means anything for the given target.
    ///
    /// A block has a start and an end; a task has neither. §3.8 says to validate this at
    /// construction, and [`Reminder::new`] does.
    #[must_use]
    pub const fn applies_to(&self, target: &ReminderTarget) -> bool {
        match target {
            ReminderTarget::Task(_) => matches!(self, Self::Due | Self::Absolute(_)),
            ReminderTarget::Block(_) => {
                matches!(self, Self::BlockStart | Self::BlockEnd | Self::Absolute(_))
            }
        }
    }
}

/// An anchor and an offset, without a target.
///
/// The same pair appears three times — on a [`Reminder`], and in both of
/// [`Settings`](super::Settings)' default lists — so it is one type. A default reminder is
/// exactly a reminder that has not been attached to anything yet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Trigger {
    /// What the offset is measured from.
    pub anchor: ReminderAnchor,
    /// Minutes: negative is before, zero is at, positive is after.
    ///
    /// Positive offsets come free and are genuinely useful — "five minutes after this block
    /// ends, log what you did."
    pub offset_mins: i32,
}

impl Trigger {
    /// A trigger some minutes before its anchor.
    #[must_use]
    pub const fn before(anchor: ReminderAnchor, mins: i32) -> Self {
        Self { anchor, offset_mins: -mins }
    }

    /// A trigger at its anchor.
    #[must_use]
    pub const fn at(anchor: ReminderAnchor) -> Self {
        Self { anchor, offset_mins: 0 }
    }

    /// A trigger some minutes after its anchor.
    #[must_use]
    pub const fn after(anchor: ReminderAnchor, mins: i32) -> Self {
        Self { anchor, offset_mins: mins }
    }
}

/// Which devices a reminder fires on (§3.8).
///
/// The default is [`Delivery::AllDevices`], and that is what makes the architecture simple
/// rather than merely the behaviour desirable: because every device holds the same synced
/// reminder data, each schedules its own local notifications. There is no coordinator and no
/// "which device owns this reminder" election.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Delivery {
    /// Everywhere.
    #[default]
    AllDevices,
    /// Only on these — "only notify me on my work laptop".
    OnlyDevices(BTreeSet<NodeId>),
    /// Everywhere except these.
    ExceptDevices(BTreeSet<NodeId>),
}

impl Delivery {
    /// Whether a given device should fire this reminder.
    ///
    /// A device not in the roster still fires an `AllDevices` reminder: a newly paired
    /// device that has not yet merged the `devices` document must not go silent.
    #[must_use]
    pub fn includes(&self, device: &NodeId) -> bool {
        match self {
            Self::AllDevices => true,
            Self::OnlyDevices(set) => set.contains(device),
            Self::ExceptDevices(set) => !set.contains(device),
        }
    }
}

/// A reminder (§3.8).
///
/// One shape covers every case. A task takes any number anchored to its due datetime; a
/// block takes any number anchored to either end of each occurrence.
///
/// Reminders attach to the **series**, not to occurrences, and fire for each expanded
/// occurrence. A cancelled occurrence must suppress its reminders — easy to miss, and very
/// annoying when missed.
#[derive(Debug, Clone, PartialEq)]
pub struct Reminder {
    /// Identity.
    pub id: ReminderId,
    /// What it is about.
    pub target: ReminderTarget,
    /// When it fires, relative to the target.
    pub trigger: Trigger,
    /// Where it fires.
    pub delivery: Delivery,
    /// Trash, for undo.
    pub deleted_at: Option<Timestamp>,
}

impl Reminder {
    /// Attaches a trigger to a target.
    ///
    /// # Errors
    ///
    /// [`ModelError::AnchorNotApplicable`] if the anchor means nothing for the target —
    /// a block start on a task, or a due date on a block.
    pub fn new(target: ReminderTarget, trigger: Trigger) -> Result<Self, ModelError> {
        if !trigger.anchor.applies_to(&target) {
            return Err(ModelError::AnchorNotApplicable { anchor: trigger.anchor });
        }
        Ok(Self {
            id: ReminderId::new(),
            target,
            trigger,
            delivery: Delivery::default(),
            deleted_at: None,
        })
    }
}

/// A record that a reminder was dealt with, on some device (§3.9).
///
/// If a reminder fires on five devices and you dismiss it on one, the other four should stop
/// nagging. This is what does that, and it propagates over the ordinary sync channel.
///
/// Be honest about the limit: propagation is **best-effort**. A device that is asleep or
/// offline will still show a stale notification, and there is no way around that in a
/// peer-to-peer design without a central broker. Dismissal is eventually consistent, like
/// everything else here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderAck {
    /// Which reminder.
    pub reminder_id: ReminderId,
    /// Which firing of it, by occurrence date.
    pub occurrence_date: civil::Date,
    /// What the user did.
    pub action: ReminderAction,
    /// When.
    pub acked_at: Timestamp,
    /// Which device it happened on.
    pub acked_by: NodeId,
}

/// What the user did with a reminder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReminderAction {
    /// Dealt with; do not show it again.
    Dismissed,
    /// Show it again later.
    Snoozed {
        /// When to show it again.
        until: Zoned,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_anchors_are_rejected_on_tasks() {
        let task = ReminderTarget::Task(TaskId::new());
        let err = Reminder::new(task, Trigger::at(ReminderAnchor::BlockEnd));
        assert!(matches!(err, Err(ModelError::AnchorNotApplicable { .. })));

        let block = ReminderTarget::Block(SeriesId::new());
        assert!(Reminder::new(block, Trigger::before(ReminderAnchor::BlockStart, 5)).is_ok());
        assert!(Reminder::new(block, Trigger::at(ReminderAnchor::Due)).is_err());
    }

    #[test]
    fn offsets_carry_their_sign() {
        assert_eq!(Trigger::before(ReminderAnchor::Due, 30).offset_mins, -30);
        assert_eq!(Trigger::at(ReminderAnchor::Due).offset_mins, 0);
        assert_eq!(Trigger::after(ReminderAnchor::BlockEnd, 5).offset_mins, 5);
    }

    #[test]
    fn delivery_defaults_to_every_device_including_unknown_ones() {
        let laptop = NodeId::from_bytes([1; 32]);
        let phone = NodeId::from_bytes([2; 32]);
        let brand_new = NodeId::from_bytes([3; 32]);

        assert!(Delivery::AllDevices.includes(&brand_new));

        let only = Delivery::OnlyDevices([laptop].into_iter().collect());
        assert!(only.includes(&laptop));
        assert!(!only.includes(&phone));

        let except = Delivery::ExceptDevices([phone].into_iter().collect());
        assert!(except.includes(&laptop));
        assert!(!except.includes(&phone));
    }
}
