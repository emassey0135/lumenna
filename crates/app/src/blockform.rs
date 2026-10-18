//! Adding or changing a block: the form, as the app keeps it.
//!
//! Every setting a block has: what it is and when — title, start, length, kind, and the three
//! flags, which start from the kind and follow it while it changes — then how it repeats and
//! behaves. The form for one day of a repeating block has only the first half: that is all one
//! day can change. What saving sends is the core's (`new_block`, `block_edit`): only what
//! changed, so a concurrent edit to another field elsewhere stands. A change refused keeps the
//! form open with the reason in it.

use lumenna_core::snapshot::Snapshot;
use lumenna_records::{BlockFields, FormField};
use serde::{Deserialize, Serialize};

/// What the person does in the block form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub enum BlockEvent {
    /// A new block: on the day shown, or today; at `start` for `minutes` when free time asks.
    New {
        /// `HH:MM`, from free time.
        start: Option<String>,
        /// How long, from free time.
        minutes: Option<u32>,
    },
    /// A field typed or chosen, by its key in the core's block form: a flag as `true` or `false`.
    Field {
        /// The field's key: `title`, `start`, `kind`, `accepts_tasks`, `date`, ….
        key: String,
        /// What it now holds.
        value: String,
    },
    /// Add the block, or save what changed.
    Save,
    /// Close the form without saving.
    Cancel,
}

/// What the form is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// A new block.
    Add {
        /// Whether the start is the core's for the day, following the Day field until the
        /// person changes it.
        start_follows_day: bool,
    },
    /// Every occurrence of a series.
    Series(String),
    /// One day of a repeating series.
    Occurrence {
        /// The series.
        series: String,
        /// The day, ISO.
        date: String,
    },
}

/// The form open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Form {
    /// What it is for.
    pub purpose: Purpose,
    /// What it is called.
    pub heading: String,
    /// What it started from: what saving compares with.
    pub initial: BlockFields,
    /// The fields as they now are.
    pub fields: BlockFields,
    /// A new block's day, as typed: the form's own, not the block's.
    pub date: String,
    /// The start the core offered for the day, which follows the Day field until changed.
    pub offered: String,
    /// The rule a series repeats by, when the repetition words cannot say it.
    pub rule: Option<String>,
    /// Why saving was refused.
    pub refusal: Option<String>,
    /// A save on its way to the host.
    pub saving: bool,
}

/// The form as shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
pub struct BlockFormView {
    /// What it is called: "New block", "Change Standup, this day only".
    pub heading: String,
    /// The fields shown, in order, in the core's words.
    pub fields: Vec<FormField>,
    /// Where the second column starts, by position in `fields`: how it repeats and behaves,
    /// after what it is and when. None for one day's form, which has one column.
    pub second_column: Option<u32>,
    /// What the fields hold.
    pub values: BlockFields,
    /// A new block's day, as typed.
    pub date: String,
    /// The button that saves: "Add" or "Save".
    pub yes: String,
    /// Why saving was refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// A new block's fields: a work block at `start` for `minutes`, with a work block's flags.
#[must_use]
pub fn fresh(start: String, minutes: u32) -> BlockFields {
    let defaults = lumenna_records::block_defaults("work".to_owned());
    BlockFields {
        title: String::new(),
        start,
        minutes: minutes.to_string(),
        kind: "work".to_owned(),
        accepts_tasks: defaults.as_ref().is_none_or(|d| d.accepts_tasks),
        counts_capacity: defaults.as_ref().is_none_or(|d| d.counts_capacity),
        anchored: defaults.as_ref().is_some_and(|d| d.anchored),
        ..BlockFields::default()
    }
}

impl Form {
    /// A form for a new block on `date` (a phrase).
    #[must_use]
    pub fn add(date: String, start: Option<String>, minutes: u32, offered: String) -> Self {
        let start_follows_day = start.is_none();
        let start = start.unwrap_or_else(|| offered.clone());
        let fields = fresh(start, minutes.min(720));
        Self {
            purpose: Purpose::Add { start_follows_day },
            heading: "New block".to_owned(),
            initial: fields.clone(),
            fields,
            date,
            offered,
            rule: None,
            refusal: None,
            saving: false,
        }
    }

    /// A form changing a block, from what it holds now.
    #[must_use]
    pub fn change(purpose: Purpose, heading: String, initial: BlockFields, rule: Option<String>) -> Self {
        Self {
            purpose,
            heading,
            fields: initial.clone(),
            initial,
            date: String::new(),
            offered: String::new(),
            rule,
            refusal: None,
            saving: false,
        }
    }

    /// The form with `key` set to `value`: a kind brings its flags, and a new day the start the
    /// core offers for it, unless the person has set one.
    pub fn set(&mut self, key: &str, value: String, snapshot: &Snapshot, now: &jiff::Zoned) {
        let on = value == "true";
        let fields = &mut self.fields;
        match key {
            "title" => fields.title = value,
            "start" => fields.start = value,
            "minutes" => fields.minutes = value,
            "kind" => {
                if let Some(defaults) = lumenna_records::block_defaults(value.clone()) {
                    fields.accepts_tasks = defaults.accepts_tasks;
                    fields.counts_capacity = defaults.counts_capacity;
                    fields.anchored = defaults.anchored;
                }
                fields.kind = value;
            }
            "accepts_tasks" => fields.accepts_tasks = on,
            "counts_capacity" => fields.counts_capacity = on,
            "anchored" => fields.anchored = on,
            "repeat" => fields.repeat = value,
            "until" => fields.until = value,
            "min_minutes" => fields.min_minutes = value,
            "task_filter" => fields.task_filter = value,
            "colour" => fields.colour = value,
            "notes" => fields.notes = value,
            "date" => {
                // A phrase half typed may not read yet; the start stays until one does.
                if let Purpose::Add { start_follows_day: true } = self.purpose
                    && let Ok(start) = lumenna_records::fields::new_block_start(snapshot, Some(&value), now)
                {
                    if fields.start == self.offered {
                        fields.start = start.clone();
                    }
                    self.offered = start;
                }
                self.date = value;
            }
            _ => {}
        }
        self.refusal = None;
    }

    /// The form as shown: the fields this purpose has.
    #[must_use]
    pub fn view(&self) -> BlockFormView {
        let once = matches!(self.purpose, Purpose::Occurrence { .. });
        let adding = matches!(self.purpose, Purpose::Add { .. });
        let kept_rule = self.rule.is_some() && self.initial.repeat.is_empty();
        let fields: Vec<FormField> = lumenna_records::block_form()
            .into_iter()
            .filter(|field| field.key != "date" || adding)
            .filter(|field| !once || field.one_day)
            .filter(|field| !field.repeating_only || !self.fields.repeat.trim().is_empty() || kept_rule)
            .map(|mut field| {
                // A rule the repetition words cannot say is kept while the field stays empty.
                if field.key == "repeat"
                    && kept_rule
                    && let Some(rule) = &self.rule
                {
                    field.hint = format!(
                        "{} It repeats by the rule {rule}, which the repetition words cannot say; leave this empty to keep it.",
                        field.hint
                    );
                }
                field
            })
            .collect();
        let second_column = (!once)
            .then(|| fields.iter().position(|f| f.key == "repeat"))
            .flatten()
            .and_then(|at| u32::try_from(at).ok());
        BlockFormView {
            heading: self.heading.clone(),
            fields,
            second_column,
            values: self.fields.clone(),
            date: self.date.clone(),
            yes: if adding { "Add" } else { "Save" }.to_owned(),
            refusal: self.refusal.clone(),
        }
    }
}
