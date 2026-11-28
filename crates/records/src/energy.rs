//! Energy's words ([`lumenna_core::energy`]): a task's rating, a block's hours, the day's
//! check-in and what the planner weighed, said once for every client.

use lumenna_core::energy::{Capacity, Energy, Fit, Hours};
use lumenna_core::model::Settings;
use serde::{Deserialize, Serialize};

use crate::actions::{Choice, Question};
use crate::words::duration;

/// A level's name, in Title Case: "Low".
#[must_use]
pub const fn energy_title(energy: Energy) -> &'static str {
    match energy {
        Energy::Low => "Low",
        Energy::Medium => "Medium",
        Energy::High => "High",
    }
}

/// What a task row and its details say of its rating: "high energy".
#[must_use]
pub fn energy_phrase(energy: Energy) -> String {
    format!("{} energy", energy.as_str())
}

/// What a block's details say of its hours: "your best hours".
#[must_use]
pub const fn hours_phrase(hours: Hours) -> &'static str {
    match hours {
        Hours::Best => "your best hours",
        Hours::Worst => "your worst hours",
    }
}

/// What a task's energy can be, as the task form and Set Energy offer it: not rated, then low,
/// medium and high, the one it is marked.
#[must_use]
pub fn energy_choices(current: Option<Energy>) -> Vec<Choice> {
    let now = |is: bool| is.then(|| "as it is now".to_owned());
    std::iter::once(Choice { id: "none".to_owned(), title: "Not Rated".to_owned(), detail: now(current.is_none()), ..Choice::default() })
        .chain(Energy::ALL.into_iter().map(|e| Choice {
            id: e.as_str().to_owned(),
            title: energy_title(e).to_owned(),
            detail: now(current == Some(e)),
            ..Choice::default()
        }))
        .collect()
}

/// What a block's hours can be, as the block form offers them: ordinary is empty, as the form
/// holds it.
#[must_use]
pub fn hours_choices() -> Vec<Choice> {
    [("", "Ordinary"), ("best", "My Best Hours"), ("worst", "My Worst Hours")]
        .into_iter()
        .map(|(id, title)| Choice { id: id.to_owned(), title: title.to_owned(), ..Choice::default() })
        .collect()
}

/// What the task form says under Energy.
pub const ENERGY_HINT: &str =
    "What it takes of your energy. Planning puts demanding work in your best hours and plans less on a low day. Not rated counts as medium.";

/// What the block form says under its hours.
pub const HOURS_HINT: &str = "Whether this is when your energy is best or worst, for planning demanding work. Ordinary leaves it to the time of day in Planning's settings.";

/// One reason the planner gives for or against a task, from how its energy meets the hours or
/// the day: whether it counts for it, and its words.
#[must_use]
pub const fn fit_words(fit: Fit) -> (bool, &'static str) {
    match fit {
        Fit::DemandingInBest => (true, "it takes high energy, and these are your best hours"),
        Fit::LightInWorst => (true, "it takes little energy, for your worst hours"),
        Fit::LightOnLowDay => (true, "it takes little energy, for a day you are low"),
        Fit::DemandingInWorst => (false, "it takes high energy, in your worst hours"),
        Fit::LightInBest => (false, "your best hours are kept for demanding work"),
        Fit::DemandingOnLowDay => (false, "it takes high energy, on a day you are low"),
    }
}

/// The check-in's question, the answer given today marked.
#[must_use]
pub fn check_in_question(today: Option<Energy>) -> Question {
    let now = |is: bool| is.then(|| "as you said".to_owned());
    Question::Choose {
        title: "How Is Your Energy Today?".to_owned(),
        message: "Planning weighs what you say for today alone: low plans less and leaves what takes the most for another day; high plans more."
            .to_owned(),
        answers: [(Energy::Low, "Low"), (Energy::Medium, "As Usual"), (Energy::High, "High")]
            .into_iter()
            .map(|(level, title)| Choice {
                id: level.as_str().to_owned(),
                title: title.to_owned(),
                detail: now(today == Some(level)),
                ..Choice::default()
            })
            .collect(),
    }
}

/// Why the day has the room it has, in words, where it is not the usual: "half the usual 4
/// hours".
#[must_use]
pub fn room_why(capacity: &Capacity) -> Option<String> {
    match (capacity.level, capacity.usual) {
        (Some(Energy::Low), Some(usual)) => Some(format!("half the usual {}", duration(usual))),
        (Some(Energy::Low), None) => Some("half the time of today's work blocks".to_owned()),
        (Some(Energy::High), Some(usual)) => Some(format!("half again the usual {}", duration(usual))),
        _ => None,
    }
}

/// What checking in says: what was said, and what planning makes of it. `capacity` is today's
/// room after, if it has a limit; `lighter` whether Replan from Now has something to take out.
#[must_use]
pub fn checked_in(level: Energy, capacity: Option<&Capacity>, lighter: bool) -> String {
    let said = match level {
        Energy::Low => "You are low today",
        Energy::Medium => "Your energy is as usual today",
        Energy::High => "You are high on energy today",
    };
    let room = match capacity {
        Some(capacity) => match room_why(capacity) {
            Some(why) => format!("Plan the Day plans up to {} of work, {why}", duration(capacity.minutes)),
            None => format!("Plan the Day plans up to {} of work", duration(capacity.minutes)),
        },
        None => "Plan the Day plans as much as your blocks hold".to_owned(),
    };
    let lighten = if lighter { ", and Replan from Now can lighten the rest of the day" } else { "" };
    format!("{said}. {room}{lighten}.")
}

/// Whether Replan from Now would lighten the rest of the day `now` falls on: only a day the
/// person said they are low, planned past its room, is.
#[must_use]
pub fn lightens(snapshot: &lumenna_core::snapshot::Snapshot, now: &jiff::Zoned) -> bool {
    let window = lumenna_core::time::minutes_of_day(snapshot.settings.day_window.1).max(24 * 60);
    lumenna_core::energy::checked_in(&snapshot.settings, now.date()) == Some(Energy::Low)
        && lumenna_core::reflow::propose_reflow(snapshot, now.date(), lumenna_core::time::minutes_of_day(now.time()), window, &[])
            .is_ok_and(|p| p.changes.iter().any(|c| matches!(c, lumenna_core::reflow::Proposed::Lighten { .. })))
}

/// How the day's energy stands, for a glance, the details of a day and an assistant: "Low
/// today", or what the person has not said.
#[must_use]
pub fn today_line(settings: &Settings, day: jiff::civil::Date) -> String {
    match lumenna_core::energy::checked_in(settings, day) {
        Some(Energy::Low) => "Low today".to_owned(),
        Some(Energy::Medium) => "As usual today".to_owned(),
        Some(Energy::High) => "High today".to_owned(),
        None => "Not said today".to_owned(),
    }
}

/// What Plan the Day says of the day's energy: how much it planned against what the day has room
/// for, and what it left for another day. None for a day with no limit.
#[must_use]
pub fn plan_words(capacity: Option<&Capacity>, spent: u32, planned: u32, spared: usize) -> Option<String> {
    let capacity = capacity?;
    let mut said = vec![match room_why(capacity) {
        Some(why) => format!("Planned to your energy: {} of the {} today has room for, {why}", duration(planned), duration(capacity.minutes)),
        None => format!("Planned to your energy: {} of the {} today has room for", duration(planned), duration(capacity.minutes)),
    }];
    if spent > 0 {
        said.push(format!("{} of it was planned already", duration(spent)));
    }
    match spared {
        0 => {}
        1 => said.push("1 task is left for another day".to_owned()),
        n => said.push(format!("{n} tasks are left for another day")),
    }
    Some(said.join(". "))
}

/// The day's energy, as a glance or a control shows it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnergyToday {
    /// The line said whole: "Energy: low today".
    pub announcement: String,
    /// Anything else worth saying.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    /// What the person said of today, `low`, `medium` or `high`; empty if nothing.
    pub level: String,
    /// The line to show: "Low today", "Not said today".
    pub line: String,
    /// What a press offers, in Title Case: "Energy Today", which asks.
    pub action: String,
    /// The same in sentence case.
    pub action_sentence: String,
}

crate::announced!(EnergyToday);

/// The day's energy at `day`, as a glance or a control shows it.
#[must_use]
pub fn energy_today(settings: &Settings, day: jiff::civil::Date) -> EnergyToday {
    EnergyToday {
        announcement: format!("Energy: {}", today_line(settings, day).to_lowercase()),
        notices: Vec::new(),
        level: lumenna_core::energy::checked_in(settings, day).map(|e| e.as_str().to_owned()).unwrap_or_default(),
        line: today_line(settings, day),
        action: CHECK_IN.to_owned(),
        action_sentence: crate::fields::sentence_case(CHECK_IN.to_owned()),
    }
}

/// The check-in's name, in Title Case: as an action, a command, a launch shortcut and a control.
pub const CHECK_IN: &str = "Energy Today";

/// The check-in reminder's button that says the person is low, in Title Case.
pub const LOW_TODAY: &str = "I'm Low Today";

/// What the check-in reminder's I'm Low Today says once pressed outside the app, where the
/// app is not there to say the host's own sentence.
pub const LOW_SAID: &str = "Noted: you are low today";

/// The check-in reminder's title and words, and the same said of the past.
#[must_use]
pub fn reminder_words() -> (String, String, String) {
    ("Energy".to_owned(), "How is your energy today?".to_owned(), "You were asked how your energy was".to_owned())
}

/// What a usual day's room can be, as its setting offers it: no limit, or some hours.
#[must_use]
pub fn capacity_options() -> Vec<(String, String)> {
    std::iter::once(("none".to_owned(), "No limit".to_owned()))
        .chain([2u32, 3, 4, 5, 6, 8].into_iter().map(|h| (format!("{h}h"), duration(h * 60))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_low_day_says_what_it_has_room_for_against_the_usual() {
        let low = Capacity { minutes: 120, level: Some(Energy::Low), usual: Some(240) };
        assert_eq!(
            checked_in(Energy::Low, Some(&low), true),
            "You are low today. Plan the Day plans up to 2 hours of work, half the usual 4 hours, and Replan from Now can lighten the rest of the day."
        );
        assert_eq!(
            plan_words(Some(&low), 30, 90, 2).as_deref(),
            Some("Planned to your energy: 1 hour 30 minutes of the 2 hours today has room for, half the usual 4 hours. 30 minutes of it was planned already. 2 tasks are left for another day")
        );
    }
}
