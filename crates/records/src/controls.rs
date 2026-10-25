//! The controls a platform offers outside the app, each a press with no screen behind it:
//! Control Center's on the iPhone, the Mac and the Apple Watch, Android's Quick Settings tiles, a
//! watch's tile button and complication. Which there are, their words, and what the timer's
//! control shows and does are worked out here once, so no platform decides what a press starts.

use jiff::Zoned;
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::Result;

/// What kind of control a platform draws.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    /// On or off, as the store says: the timer.
    Toggle,
    /// A press that opens the app somewhere: its `id` is a launch shortcut's.
    Button,
}

/// A control every platform that has them offers, as its gallery lists it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Control {
    /// What the platform knows it by: "timer", or a launch shortcut's id ("new-task").
    pub id: String,
    /// Its name, in Title Case: "Timer".
    pub name: String,
    /// The same in sentence case.
    pub sentence: String,
    /// What it does, for the gallery's second line.
    pub description: String,
    /// Whether it is a toggle or a button.
    pub kind: ControlKind,
}

/// Every control, in the order a gallery lists them: the timer, then New Task, whose words are
/// its launch shortcut's.
#[must_use]
pub fn controls() -> Vec<Control> {
    let timer = Control {
        id: "timer".to_owned(),
        name: "Timer".to_owned(),
        sentence: "Timer".to_owned(),
        description: "Starts on what to work on now, or stops the timer running".to_owned(),
        kind: ControlKind::Toggle,
    };
    let new_task = crate::launch::launch_shortcuts().into_iter().find(|s| s.id == "new-task").map(|s| Control {
        id: s.id,
        name: s.title,
        sentence: s.sentence,
        description: s.description,
        kind: ControlKind::Button,
    });
    std::iter::once(timer).chain(new_task).collect()
}

/// What the timer's control shows now, and what a press does (`Lumenna::toggle_timer`): with a
/// timer running, it stops that one; else it starts on the first of What Should I Work On?'s
/// choices, as choosing it there would.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerControl {
    /// Whether a timer runs: the toggle's state.
    pub running: bool,
    /// What it says beside its name: the task whose timer runs; else what a press would start;
    /// else why nothing would be.
    pub detail: String,
    /// What a press does, in Title Case: "Stop Timer" or "Start Timer".
    pub action: String,
    /// The same in sentence case.
    pub action_sentence: String,
    /// Whether a press does anything: not when nothing runs and nothing can be started.
    pub enabled: bool,
}

/// The sitting whose timer runs, if one does.
#[must_use]
pub fn running_sitting(snapshot: &Snapshot) -> Option<String> {
    snapshot.assignments.values().find(|a| a.running_since.is_some()).map(|a| a.id.to_string())
}

/// The timer's control at `now`.
///
/// # Errors
///
/// If the day cannot be worked out.
pub fn timer_control(snapshot: &Snapshot, now: &Zoned) -> Result<TimerControl> {
    let control = |running: bool, detail: String, action: &str, enabled: bool| TimerControl {
        running,
        detail,
        action: action.to_owned(),
        action_sentence: crate::fields::sentence_case(action.to_owned()),
        enabled,
    };
    let running = snapshot.assignments.values().find(|a| a.running_since.is_some());
    if let Some(sitting) = running {
        let task = snapshot.tasks.get(&sitting.task_id).map(|t| t.title.clone()).unwrap_or_default();
        return Ok(control(true, task, "Stop Timer", true));
    }
    let choices = crate::choices::what_now(snapshot, now)?;
    Ok(match choices.choices.first() {
        Some(first) => control(false, first.title.clone(), "Start Timer", true),
        None => control(false, choices.announcement, "Start Timer", false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_controls_are_the_timer_then_new_task_in_its_launch_shortcuts_words() {
        let controls = controls();
        let ids: Vec<&str> = controls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["timer", "new-task"]);
        let new_task = crate::launch::launch_shortcuts().into_iter().find(|s| s.id == "new-task").unwrap();
        assert_eq!((controls[1].name.as_str(), controls[1].kind), (new_task.title.as_str(), ControlKind::Button));
    }

    /// The text of `<string name="{name}">` in the Android resources file at `path`.
    fn android_string(path: &str, name: &str) -> String {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path);
        let strings = std::fs::read_to_string(&file).unwrap();
        let element = format!("<string name=\"{name}\">");
        let start = strings.find(&element).unwrap_or_else(|| panic!("no {name} in {path}")) + element.len();
        strings[start..start + strings[start..].find("</string>").unwrap()].replace(r"\'", "'")
    }

    #[test]
    fn android_names_its_controls_ahead_as_the_core_words_them() {
        // The Quick Settings tiles' manifest labels, before a tile is first drawn, and the watch's
        // New Task complication as the complication picker lists it: sentence case, as Android's.
        for control in controls() {
            let name = format!("control_{}", control.id.replace('-', "_"));
            assert_eq!(android_string("apps/android/app/src/main/res/values/strings.xml", &name), control.sentence);
        }
        let new_task = controls().into_iter().find(|c| c.id == "new-task").unwrap();
        assert_eq!(
            android_string("apps/android/wear/src/main/res/values/strings.xml", "complication_new_task"),
            new_task.sentence
        );
    }

    #[test]
    fn with_nothing_running_and_nothing_to_start_the_timer_says_why_and_does_nothing() {
        let snapshot = Snapshot::default();
        let now = jiff::civil::date(2026, 10, 25).at(10, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC).unwrap();
        let control = timer_control(&snapshot, &now).unwrap();
        assert!(!control.running && !control.enabled, "{control:?}");
        assert_eq!(control.action, "Start Timer");
        assert!(!control.detail.is_empty(), "the reason nothing would start");
    }
}
