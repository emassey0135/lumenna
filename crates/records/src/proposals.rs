//! The planner's proposals as records: Replan from Now's and Plan the Day's, the pins a person
//! sets on them, and what applying one made. The proposing is the host's.

use serde::{Deserialize, Serialize};

use crate::types::Change;

/// The clock a client says times in.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[serde(rename_all = "snake_case")]
pub enum TimeStyle {
    /// 9:45 AM.
    TwelveHour,
    /// 09:45.
    TwentyFourHour,
}

/// One change of a proposal, said, with what applying it takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReplanChange {
    /// The whole change in a sentence: "Lunch moves from 10:30 to 11:00".
    pub sentence: String,
    /// `block`, `skip` or `sitting`.
    pub kind: String,
    /// The block (series) it moves or skips, or the sitting it moves.
    pub target: String,
    /// For a block: its start before and after, minutes from midnight, and its length before
    /// and after. For a sitting: where it goes, `<series>@<date>`, or empty for back to the list.
    pub from_start: i64,
    /// See `from_start`.
    pub to_start: i64,
    /// See `from_start`.
    pub from_mins: u32,
    /// See `from_start`.
    pub to_mins: u32,
    /// For a sitting, where it goes.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub destination: String,
}

/// What replanning the rest of today proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Replan {
    /// The whole proposal in one sentence, said before anything changes; or that nothing
    /// needs to.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The changes, each to accept or not.
    pub changes: Vec<ReplanChange>,
    /// What does not fit, said: the planner cannot make time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unplaceable: Vec<String>,
    /// What the person decided that this proposal keeps to, handed back to apply it.
    pub pins: ReplanPins,
    /// The clock its sentences are in, for the proposal that follows.
    pub style: TimeStyle,
    /// Other options, each a block let shorten below its minimum or skipped, with what that
    /// costs and saves: "Let Lunch shorten from 30 minutes to 15 minutes: 15 minutes less
    /// break. Then Review moves from 11:00 to 11:15 and shortens from 1 hour to 45 minutes." Choosing one asks
    /// [`Lumenna::replan`] again with its `pins`. Offered only where it does better.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<ReplanAlternative>,
}

crate::announced!(Replan);

/// Another way through the rest of the day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReplanAlternative {
    /// What it is, what it costs and what follows, in a sentence or two.
    pub sentence: String,
    /// The pins to propose with, to take it.
    pub pins: ReplanPins,
}


/// What the person decided about the rest of today, which a proposal keeps to: each a
/// block's series, or a sitting's identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReplanPins {
    /// Blocks left where and as long as they are.
    #[serde(default)]
    pub keep: Vec<String>,
    /// Blocks skipped today.
    #[serde(default)]
    pub skip: Vec<String>,
    /// Sittings left in their block.
    #[serde(default)]
    pub sittings: Vec<String>,
    /// Blocks let shorten below their own minimum, as an option chose.
    #[serde(default)]
    #[cfg_attr(feature = "uniffi", uniffi(default = []))]
    pub floors: Vec<ReplanFloor>,
}

/// A block let shorten below its own minimum, to as little as `minutes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReplanFloor {
    /// The block's series.
    pub block: String,
    /// The least it may be.
    pub minutes: u32,
}


/// A replan's accepted changes made, and what is proposed now.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Replanned {
    /// What was changed, then what is proposed now, or that the rest of the day fits.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The change made: one undo takes it back.
    pub change: Change,
    /// The proposal for the day as it is now, the changes not accepted kept to as pins.
    pub next: Replan,
}

crate::announced!(Replanned);

/// One sitting a day plan proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ProposedSitting {
    /// The sitting in a sentence: "Report goes in Morning at 9:00, for 45 minutes".
    pub sentence: String,
    /// Why this task, as suggestions say it: "Suggested because priority 1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The task.
    pub task: String,
    /// The block, `<series>@<date>`.
    pub block: String,
    /// Its planned length.
    pub minutes: u32,
}

/// What planning a day proposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DayPlan {
    /// The whole plan in a sentence or two, said before anything changes.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The day planned, `YYYY-MM-DD`.
    pub date: String,
    /// The sittings, in the order of the day, each to accept or not.
    pub sittings: Vec<ProposedSitting>,
    /// What is due by then and fits nowhere, by title.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unfit: Vec<String>,
    /// What the person decided that this plan keeps to, handed back to apply it.
    pub pins: DayPins,
    /// The clock its sentences are in, for the plan that follows.
    pub style: TimeStyle,
}

crate::announced!(DayPlan);

/// What the person decided about a day's plan, which the next plan keeps to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DayPins {
    /// Tasks not to plan that day.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Sittings turned down, `<task>@<series>`: that task may still go in another block.
    #[serde(default)]
    pub declined: Vec<String>,
}

/// A day plan's accepted sittings made, and what is proposed now.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DayPlanned {
    /// What was planned, then what is proposed now, if anything.
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// The change made: one undo takes it back.
    pub change: Change,
    /// The plan for the day as it is now, the sittings turned down kept to.
    pub next: DayPlan,
}

crate::announced!(DayPlanned);
