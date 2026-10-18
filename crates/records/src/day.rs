//! What can be done to a day itself, and which of its sittings can carry forward.

use std::collections::BTreeSet;

use jiff::Zoned;
use lumenna_core::model::AssignmentStatus;
use lumenna_core::snapshot::Snapshot;

use crate::actions::{Action, ActionKind, Question, Subject};
use crate::types::Plan;
use crate::words::count_line;

/// How much longer Add Minutes makes the block under way.
pub const EXTEND_MINS: u32 = 15;

/// The sittings of a day that can carry forward: in a block already over, short of their
/// planned time, their task still open, not already moved on.
pub fn carries(snapshot: &Snapshot, plan: &Plan, now: &Zoned) -> BTreeSet<String> {
    let facts = snapshot.facts();
    let past_day = plan.date.parse::<jiff::civil::Date>().is_ok_and(|d| d < now.date());
    plan.blocks
        .iter()
        .filter(|block| past_day || block.when == "past")
        .flat_map(|block| &block.assignments)
        .filter(|sitting| sitting.planned_mins.is_some_and(|p| sitting.minutes < p))
        .filter(|sitting| {
            let Ok(id) = sitting.id.parse() else { return false };
            let Some(assignment) = snapshot.assignments.get(&id) else { return false };
            !matches!(assignment.status, AssignmentStatus::Deferred | AssignmentStatus::Skipped)
                && snapshot.tasks.get(&assignment.task_id).is_some_and(|t| !t.is_deleted() && !facts.is_completed(t))
        })
        .map(|sitting| sitting.id.clone())
        .collect()
}

/// What can be done to today itself, while something is left of it.
pub fn day_actions(plan: &Plan, what_now: bool) -> Vec<Action> {
    let today = Zoned::now().date().to_string();
    // A day still to come, or the rest of today, with a work block still ahead to fill.
    let to_fill = plan.date >= today && plan.blocks.iter().any(|b| b.accepts_tasks && b.when != "past");
    let plan_day = || {
        let mut day = Action::new(ActionKind::PlanDay, Subject::Day, "Plan the Day", plan.date.clone());
        day.question = Question::Form;
        day
    };
    if plan.date != today {
        return if to_fill { vec![plan_day()] } else { Vec::new() };
    }
    let current = plan.blocks.iter().find(|b| b.when == "now");
    let later = plan.blocks.iter().filter(|b| b.when == "upcoming").count();
    let cleared = |later: usize| match later {
        0 => String::new(),
        1 => "The block after it is cleared for today.".to_owned(),
        n => format!("The {n} blocks after it are cleared for today."),
    };
    let mut actions = Vec::new();
    if to_fill {
        actions.push(plan_day());
    }
    // What is left of the day, worked out again from now: a proposal, said before it is done.
    if current.is_some() || later > 0 {
        actions.push(Action::new(ActionKind::Replan, Subject::Day, "Replan from Now", plan.date.clone()));
    }
    if what_now {
        let mut ask = Action::new(ActionKind::WhatNow, Subject::Day, "What Should I Work On?", plan.date.clone());
        ask.question = Question::Pick { title: "What should I work on?".to_owned(), length: None, yes: "Start".to_owned() };
        actions.push(ask);
    }
    if let Some(current) = current {
        let mut end = Action::new(ActionKind::EndDay, Subject::Day, "End the Day", plan.date.clone());
        end.destructive = later > 0;
        end.question = Question::Confirm {
            title: "End the day?".to_owned(),
            message: format!("{} ends now. {} Undo brings it back.", current.title, cleared(later)).replace("  ", " "),
            yes: "End the Day".to_owned(),
        };
        actions.push(end);
    }
    if later > 0 {
        let mut clear = Action::new(ActionKind::ClearDay, Subject::Day, "Clear the Rest of the Day", plan.date.clone());
        clear.destructive = true;
        clear.question = Question::Confirm {
            title: "Clear the rest of the day?".to_owned(),
            message: format!(
                "{} {} cleared for today: a repeating one is cancelled for today alone, one that happens once goes to the trash. Undo brings them back.",
                crate::words::capitalised(&count_line(later, "block")),
                if later == 1 { "is" } else { "are" }
            ),
            yes: "Clear the Rest of the Day".to_owned(),
        };
        actions.push(clear);
    }
    actions
}
