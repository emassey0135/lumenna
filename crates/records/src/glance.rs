//! A glance: what a widget, a complication, a tile or a Smart Stack shows, worked out once from
//! the store for every platform, so none decides for itself what the day looks like at a
//! glance. A platform words it with `lumenna_app::glance` in its own clock and draws it.
//!
//! Widget systems ask for a day ahead, not a redraw every minute, so a glance is a
//! [`timeline`]: one [`GlanceMoment`] from now, then one at each moment what it shows changes
//! (a block starts or ends, a due time passes, midnight), each whole. A running timer needs no
//! moment of its own: its start is given, and the platform counts from it.

use std::collections::BTreeSet;

use jiff::{Timestamp, Zoned};
use lumenna_core::snapshot::Snapshot;
use serde::{Deserialize, Serialize};

use crate::{PlanBlock, Result};

/// How many of the tasks due a glance names; the rest are counted.
pub const TASKS_NAMED: usize = 3;

/// A block, as a glance shows it.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlanceBlock {
    /// `<series>@<date>`, which addresses this occurrence.
    pub id: String,
    /// Its name.
    pub title: String,
    /// When it starts, `HH:MM`, for the platform's clock.
    pub start: String,
    /// When it ends, `HH:MM`.
    pub end: String,
    /// When it starts, as an instant, for a platform that counts down or draws progress.
    pub starts_at: String,
    /// When it ends, as an instant.
    pub ends_at: String,
    /// `work`, `break` or `event`.
    pub kind: String,
    /// Its colour, by name, drawn beside its name, never in place of it.
    pub colour: Option<String>,
    /// The titles of the tasks in it, in order.
    pub tasks: Vec<String>,
}

/// A sitting whose timer runs.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlanceTimer {
    /// The sitting's identifier.
    pub sitting: String,
    /// The task's title.
    pub task: String,
    /// The block it is in.
    pub block: String,
    /// The instant counting from which gives everything logged: when the timer started, less
    /// what was logged before. A platform shows the time since, and needs no other moment.
    pub counting_from: String,
}

/// A task a glance names.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlanceTask {
    /// Its identifier.
    pub id: String,
    /// Its title.
    pub title: String,
    /// When it is due, in words without the time ("due today").
    pub due: Option<String>,
    /// Its time, `HH:MM`, when it has one.
    pub due_time: Option<String>,
    /// Whether it is overdue.
    pub overdue: bool,
}

/// What a glance shows from one moment until the next.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlanceMoment {
    /// When it starts being true, as an instant.
    pub at: String,
    /// The day it is of, as an ISO date.
    pub date: String,
    /// The block under way, if one is.
    pub current: Option<GlanceBlock>,
    /// The next block that day, if there is one.
    pub next: Option<GlanceBlock>,
    /// The timer running, if one is.
    pub timer: Option<GlanceTimer>,
    /// The most urgent of the tasks due by that day or overdue, at most [`TASKS_NAMED`].
    pub tasks: Vec<GlanceTask>,
    /// How many tasks are due by that day or overdue.
    pub due: u32,
    /// How many of those are overdue.
    pub overdue: u32,
    /// How many blocks that day are still to come, the one under way included.
    pub blocks_left: u32,
}

/// A part of a worded glance (`lumenna_app::glance::GlanceEntry`) a widget shows.
#[boltffi::data]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlancePart {
    /// What is happening now: the timer running, the block under way, free time.
    Headline,
    /// The next block.
    Next,
    /// How many tasks are due, and how many overdue.
    Due,
    /// The tasks due named, each with its Mark Done.
    Tasks,
}

/// A widget every platform offers, as its widget picker or gallery lists it: Android's widget
/// list, Windows' Widgets board, Apple's widget gallery, the complications a watch face offers.
/// The same kinds everywhere, in these words, each showing these parts of the glance, so no
/// platform names or fills a widget its own way. A platform whose list is a file written ahead
/// (Android's strings, Windows' package manifest) keeps it as this says, which a test of the
/// core checks; the rest set it at run time.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlanceWidget {
    /// What the platform knows it by: "now".
    pub id: String,
    /// Its name in the picker: "Now".
    pub name: String,
    /// What it shows, for the picker's second line.
    pub description: String,
    /// The parts it shows, in order. A smaller size shows the first of them that fit.
    pub parts: Vec<GlancePart>,
}

/// Every widget, smallest first.
#[must_use]
pub fn widgets() -> Vec<GlanceWidget> {
    use GlancePart::{Due, Headline, Next, Tasks};
    [
        ("now", "Now", "What is happening now, and the timer running", vec![Headline]),
        ("today", "Today", "What is happening now, what is next, and the tasks due", vec![Headline, Next, Due, Tasks]),
    ]
    .into_iter()
    .map(|(id, name, description, parts)| GlanceWidget {
        id: id.to_owned(),
        name: name.to_owned(),
        description: description.to_owned(),
        parts,
    })
    .collect()
}

/// What a glance shows from `now` until `until`: the moment now, then one wherever that
/// changes.
///
/// # Errors
///
/// If a day's blocks cannot be worked out.
pub fn timeline(snapshot: &Snapshot, now: &Zoned, until: Timestamp) -> Result<Vec<GlanceMoment>> {
    let mut changes: BTreeSet<Timestamp> = BTreeSet::new();
    let mut day = now.date();
    while let Ok(start) = day.to_zoned(now.time_zone().clone()) {
        if start.timestamp() > until {
            break;
        }
        if start.timestamp() > now.timestamp() {
            changes.insert(start.timestamp());
        }
        for block in crate::plan::plan(snapshot, day, now)?.blocks {
            for time in [&block.start, &block.end] {
                if let Some(at) = instant(day, time, now) {
                    changes.insert(at);
                }
            }
        }
        for task in snapshot.tasks.values().filter(|t| !t.is_deleted()) {
            if let Some(due) = &task.due
                && due.date == day
                && let Some(time) = due.time
                && let Some(at) = instant(day, &time.strftime("%H:%M").to_string(), now)
            {
                changes.insert(at);
            }
        }
        let Ok(next) = day.tomorrow() else { break };
        day = next;
    }
    let mut moments = vec![moment(snapshot, now)?];
    for at in changes.into_iter().filter(|at| *at > now.timestamp() && *at <= until) {
        moments.push(moment(snapshot, &at.to_zoned(now.time_zone().clone()))?);
    }
    Ok(moments)
}

/// The glance at `now`.
fn moment(snapshot: &Snapshot, now: &Zoned) -> Result<GlanceMoment> {
    let today = now.date();
    let plan = crate::plan::plan(snapshot, today, now)?;
    let block = |b: &PlanBlock| GlanceBlock {
        id: b.id.clone(),
        title: b.title.clone(),
        start: b.start.clone(),
        end: b.end.clone(),
        starts_at: instant(today, &b.start, now).map(|t| t.to_string()).unwrap_or_default(),
        ends_at: instant(today, &b.end, now).map(|t| t.to_string()).unwrap_or_default(),
        kind: b.kind.clone(),
        colour: b.colour.clone(),
        tasks: b.assignments.iter().map(|a| a.title.clone()).collect(),
    };
    let current = plan.blocks.iter().find(|b| b.when == "now");
    let upcoming: Vec<&PlanBlock> = plan.blocks.iter().filter(|b| b.when == "upcoming").collect();
    let timer = snapshot.assignments.values().find_map(|a| {
        let since = a.running_since?;
        let counting_from = since.checked_sub(jiff::SignedDuration::from_mins(i64::from(a.accumulated_mins))).ok()?;
        let in_block = plan.blocks.iter().find(|b| b.assignments.iter().any(|s| s.id == a.id.to_string()));
        Some(GlanceTimer {
            sitting: a.id.to_string(),
            task: snapshot.tasks.get(&a.task_id).map(|t| t.title.clone()).unwrap_or_default(),
            block: in_block.map(|b| b.title.clone()).unwrap_or_default(),
            counting_from: counting_from.to_string(),
        })
    });
    let rows = crate::lists::tasks(snapshot, "overdue | today", now)?;
    let overdue_of = |id: &str| {
        let Some(task) = id.parse().ok().and_then(|id| snapshot.tasks.get(&id)) else { return false };
        task.due.as_ref().is_some_and(|due| due.date < today || (due.date == today && due.time.is_some_and(|time| time <= now.time())))
    };
    let tasks: Vec<GlanceTask> = rows
        .rows
        .iter()
        .map(|row| GlanceTask {
            id: row.id.clone(),
            title: row.title.clone(),
            due: row.due.clone(),
            due_time: row.due_time.clone(),
            overdue: overdue_of(&row.id),
        })
        .collect();
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Ok(GlanceMoment {
        at: now.timestamp().to_string(),
        date: today.to_string(),
        current: current.map(block),
        next: upcoming.first().map(|b| block(b)),
        timer,
        due: count(tasks.len()),
        overdue: count(tasks.iter().filter(|t| t.overdue).count()),
        tasks: tasks.into_iter().take(TASKS_NAMED).collect(),
        blocks_left: count(upcoming.len() + usize::from(current.is_some())),
    })
}

/// `HH:MM` on `day`, in `now`'s zone, as an instant; a day's end ("24:00") is midnight after.
fn instant(day: jiff::civil::Date, time: &str, now: &Zoned) -> Option<Timestamp> {
    if time == "24:00" {
        return day.tomorrow().ok()?.to_zoned(now.time_zone().clone()).ok().map(|z| z.timestamp());
    }
    let time: jiff::civil::Time = time.parse().ok()?;
    day.at(time.hour(), time.minute(), 0, 0).to_zoned(now.time_zone().clone()).ok().map(|z| z.timestamp())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// The repository's root, from this crate.
    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The value of `attribute="…"` in `element`, XML's escapes undone.
    fn attribute(element: &str, attribute: &str) -> Option<String> {
        let start = element.find(&format!(" {attribute}=\""))? + attribute.len() + 3;
        let end = start + element[start..].find('"')?;
        Some(element[start..end].replace("&apos;", "'").replace("&quot;", "\"").replace("&amp;", "&"))
    }

    #[test]
    fn every_widget_shows_what_is_happening_now_first() {
        for widget in widgets() {
            assert_eq!(widget.parts.first(), Some(&GlancePart::Headline), "{}", widget.id);
        }
    }

    #[test]
    fn android_offers_every_widget_as_the_core_words_it() {
        let strings = std::fs::read_to_string(root().join("apps/android/app/src/main/res/values/strings.xml")).unwrap();
        let string = |name: &str| {
            let element = format!("<string name=\"{name}\">");
            let start = strings.find(&element).unwrap_or_else(|| panic!("no {name} in strings.xml")) + element.len();
            strings[start..start + strings[start..].find("</string>").unwrap()].replace(r"\'", "'")
        };
        for widget in widgets() {
            assert_eq!(string(&format!("widget_{}", widget.id)), widget.name);
            assert_eq!(string(&format!("widget_{}_description", widget.id)), widget.description);
            let info = root().join(format!("apps/android/app/src/main/res/xml/widget_{}.xml", widget.id));
            assert!(info.exists(), "{}", info.display());
        }
    }

    #[test]
    fn the_windows_package_declares_every_widget_as_the_core_words_it() {
        let manifest = std::fs::read_to_string(root().join("apps/windows/installer/msix/AppxManifest.xml")).unwrap();
        let declared: Vec<(String, String, String)> = manifest
            .split("<Definition ")
            .skip(1)
            .map(|element| {
                let element = format!(" {}", &element[..element.find('>').unwrap()]);
                let get = |name| attribute(&element, name).unwrap_or_default();
                (get("Id"), get("DisplayName"), get("Description"))
            })
            .collect();
        let wanted: Vec<(String, String, String)> =
            widgets().into_iter().map(|w| (w.id, w.name, w.description)).collect();
        assert_eq!(declared, wanted);
    }
}
