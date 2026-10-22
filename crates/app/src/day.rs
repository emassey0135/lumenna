//! A day as it is lived, as rows: the summary first, then blocks in time order with their
//! sittings beneath them, free time and now as rows of their own, and the blocks cancelled for
//! the day last.

use std::collections::BTreeSet;

use crux_core::Command;
use jiff::civil::Date;
use crux_core::render::render;
use lumenna_records::places::Place;
use lumenna_records::types::{CancelledBlock, Plan, PlanAssignment, PlanBlock, PlanItem};
use lumenna_records::{Action, ActionKind, Offered, Question, Subject};
use serde::{Deserialize, Serialize};

use crate::asking::{Open, Purpose};
use crate::model::Screen;
use crate::{DayMove, Effect, Event, Lumenna};

/// What a row of the day is.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DayKind {
    /// The day's summary, which is also where its own actions are.
    Summary,
    /// A block.
    Block,
    /// A task's sitting in a block, beneath it.
    Sitting,
    /// Free time between blocks.
    Free,
    /// Where now falls, outside any block.
    Now,
    /// A repeating block cancelled for this day.
    Cancelled,
}

/// Free time, as a row says it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreeTime {
    /// `HH:MM`.
    pub start: String,
    /// `HH:MM`.
    pub end: String,
    /// How long.
    pub minutes: u32,
    /// "Free".
    pub title: String,
    /// The core's parts: how long, in words.
    pub details: Vec<String>,
}

/// Now, outside a block.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NowRow {
    /// "Now".
    pub title: String,
    /// `HH:MM`.
    pub time: String,
}

/// One row of the day.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayItem {
    /// What identifies it across a reload: `summary`, `block:<id>`, `sitting:<id>`,
    /// `free:<start>`, `now`, `cancelled:<series>`.
    pub key: String,
    /// What it is.
    pub kind: DayKind,
    /// 0, or 1 for a sitting beneath its block.
    pub depth: u32,
    /// What can be done to it, the core's, with Expand or Collapse for a block with sittings.
    pub actions: Vec<Action>,
    /// What can be done to it, in short, as the view offers it (see [`Offered`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<Offered>,
    /// Folded away under a collapsed block.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// For a block with sittings: whether they are shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// "level 2" where the level differs from the row shown before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// A block's record, for a block or for a sitting's block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<PlanBlock>,
    /// A sitting's record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sitting: Option<PlanAssignment>,
    /// Free time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free: Option<FreeTime>,
    /// Now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<NowRow>,
    /// A cancelled day's block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancelled: Option<CancelledBlock>,
}

impl DayItem {
    fn new(key: String, kind: DayKind, depth: u32, actions: Vec<Action>) -> Self {
        Self {
            key,
            kind,
            depth,
            actions,
            offered: Vec::new(),
            hidden: false,
            expanded: None,
            level: None,
            block: None,
            sitting: None,
            free: None,
            now: None,
            cancelled: None,
        }
    }
}

/// A day, as shown.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayView {
    /// The day, ISO.
    pub date: String,
    /// Whether it is today.
    pub today: bool,
    /// The core's summary: "2 blocks, 3 hours of work, 1 task assigned".
    pub summary: String,
    /// The summary as a braille display shows it: "2 blk, 3:00 wrk, 1 tsk".
    pub summary_braille: String,
    /// The day's own actions: Plan the Day, Replan from Now, What Should I Work On?, …
    pub actions: Vec<Action>,
    /// What can be done to the day itself, in short, as the view offers it (see [`Offered`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offered: Vec<Offered>,
    /// Its rows, those folded away marked hidden.
    pub items: Vec<DayItem>,
}

/// The day's rows, with what `folded` names folded away.
#[must_use]
pub fn day(plan: &Plan, today: bool, folded: &BTreeSet<String>) -> DayView {
    let mut items = vec![DayItem::new("summary".to_owned(), DayKind::Summary, 0, plan.actions.clone())];
    for entry in &plan.timeline {
        match entry {
            PlanItem::Block { row } => {
                let Some(block) = plan.blocks.iter().find(|b| b.row == *row) else { continue };
                let key = format!("block:{}", block.id);
                let collapsed = folded.contains(&key);
                let mut item = DayItem::new(key.clone(), DayKind::Block, 0, block.actions.clone());
                if !block.assignments.is_empty() {
                    item.expanded = Some(!collapsed);
                    let (kind, title) =
                        if collapsed { (ActionKind::Expand, "Expand") } else { (ActionKind::Collapse, "Collapse") };
                    let mut fold = Action::new(kind, Subject::Block, title, key.clone());
                    fold.question = Question::Immediate;
                    item.actions.push(fold);
                }
                item.block = Some(block.clone());
                items.push(item);
                for sitting in &block.assignments {
                    let mut row =
                        DayItem::new(format!("sitting:{}", sitting.id), DayKind::Sitting, 1, sitting.actions.clone());
                    row.hidden = collapsed;
                    row.sitting = Some(sitting.clone());
                    row.block = Some(block.clone());
                    items.push(row);
                }
            }
            PlanItem::Free { start, end, minutes, title, details, actions } => {
                let mut item = DayItem::new(format!("free:{start}"), DayKind::Free, 0, actions.clone());
                item.free = Some(FreeTime {
                    start: start.clone(),
                    end: end.clone(),
                    minutes: *minutes,
                    title: title.clone(),
                    details: details.clone(),
                });
                items.push(item);
            }
            PlanItem::Now { time, title } => {
                let mut item = DayItem::new("now".to_owned(), DayKind::Now, 0, Vec::new());
                item.now = Some(NowRow { title: title.clone(), time: time.clone() });
                items.push(item);
            }
        }
    }
    for cancelled in &plan.cancelled {
        let mut item =
            DayItem::new(format!("cancelled:{}", cancelled.series), DayKind::Cancelled, 0, cancelled.actions.clone());
        item.cancelled = Some(cancelled.clone());
        items.push(item);
    }
    let mut previous = 0;
    for item in &mut items {
        if item.hidden {
            continue;
        }
        if item.depth != previous {
            item.level = Some(format!("level {}", item.depth + 1));
        }
        previous = item.depth;
    }
    DayView {
        date: plan.date.clone(),
        today,
        summary: plan.summary.clone(),
        summary_braille: plan.summary_braille.clone(),
        actions: plan.actions.clone(),
        offered: Vec::new(),
        items,
    }
}

/// Where a day opens: on now — the block under way, or the now row — else its summary.
#[must_use]
pub fn opening(view: &DayView) -> Option<String> {
    view.items
        .iter()
        .find(|item| item.kind == DayKind::Now || item.block.as_ref().is_some_and(|b| b.when == "now" && item.kind == DayKind::Block))
        .or_else(|| view.items.first())
        .map(|item| item.key.clone())
}

impl Lumenna {
    pub(crate) fn show_block(&self, series: &str, date: &str, model: &mut Screen) -> Command<Effect, Event> {
        let (Some(store), Ok(day)) = (&model.store, date.parse::<Date>()) else { return Command::done() };
        let today = store.now.date();
        let shown = model.show_day((day != today).then_some(day));
        model.hold(&format!("block:{series}@{date}"));
        shown
    }

    pub(crate) fn day_move(&self, step: DayMove, model: &mut Screen) -> Command<Effect, Event> {
        let Some(store) = &model.store else { return Command::done() };
        let today = store.now.date();
        let shown = if model.place() == Place::Today { model.day.unwrap_or(today) } else { today };
        match step {
            DayMove::Previous | DayMove::Next => {
                let days = if step == DayMove::Next { 1 } else { -1 };
                let Ok(to) = shown.checked_add(jiff::Span::new().days(days)) else { return Command::done() };
                model.show_day((to != today).then_some(to))
            }
            DayMove::Today => model.show_day(None),
            DayMove::Now => {
                let shown = model.show_day(None);
                if let Some(key) = model.day_view().as_ref().and_then(opening) {
                    model.hold(&key);
                }
                shown
            }
            DayMove::Ask => {
                model.open = Some(Open::new(Purpose::GoToDay));
                render()
            }
        }
    }
}
