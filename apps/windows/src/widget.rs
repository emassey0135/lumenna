//! A widget on the Windows 11 Widgets board: the glance (`lumenna_records::glance`, worded by
//! `lumenna_app::glance`) as the data of an Adaptive Card, which the templates beside this file
//! draw in two sizes. Every word is the core's: the glance's lines, a task's line as its row
//! reads (`speech::row_line`), its Mark Done as its row offers it, and a timer's time as the
//! core says a length. What is decided here is only which of them each size shows, and when
//! the card must be drawn again.
//!
//! The text goes in the card's data, never its template: a title holding `${` is then shown
//! as typed rather than read as a binding.

use jiff::Timestamp;
use lumenna_app::glance::GlanceEntry;
use lumenna_app::speech::{self, Clock, Line};
use lumenna_records::{ActionKind, Rows};
use serde_json::{Value, json};

/// The small card: what is happening now, and the timer running.
pub const SMALL: &str = include_str!("widget/small.json");
/// The medium card: what is happening now, the next block, and what is due, each with its
/// Mark Done.
pub const MEDIUM: &str = include_str!("widget/medium.json");

/// The verb a task's Mark Done sends back, with the task's `id` as its data.
pub const COMPLETE: &str = "complete";

/// A task the glance names, as its row in the listing reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    pub id: String,
    pub line: Line,
    pub overdue: bool,
    /// What its Mark Done is called, while its row offers one.
    pub done: Option<String>,
}

/// The tasks `entry` names, each as its row in `rows` (the same listing the glance counts,
/// "overdue | today") reads and with the Mark Done that row offers. A task the listing no
/// longer has, written in the moment between, is named by its title alone and offers nothing.
pub fn due(entry: &GlanceEntry, rows: &Rows, clock: &dyn Clock) -> Vec<Due> {
    entry
        .moment
        .tasks
        .iter()
        .map(|task| match rows.rows.iter().find(|row| row.id == task.id) {
            Some(row) => Due {
                id: task.id.clone(),
                line: speech::row_line(row, false, clock),
                overdue: task.overdue,
                done: row.actions.iter().find(|action| action.kind == ActionKind::MarkDone).map(|action| action.title.clone()),
            },
            None => Due { id: task.id.clone(), line: Line::of(task.title.clone(), Vec::new()), overdue: task.overdue, done: None },
        })
        .collect()
}

/// The card's data at `now`, for either template.
pub fn data(entry: &GlanceEntry, due: &[Due], now: Timestamp) -> Value {
    let timer = entry.moment.timer.as_ref().and_then(|timer| logged(&timer.counting_from, now));
    let shown = |text: Option<&str>| json!({ "shown": text.is_some_and(|text| !text.is_empty()), "text": text.unwrap_or_default() });
    json!({
        "headline": {
            "title": entry.headline.title,
            "shown": !entry.headline.detail.is_empty(),
            "detail": entry.headline.detail,
        },
        "timer": shown(timer.as_deref()),
        "next": shown(entry.next.as_ref().map(|next| next.text.as_str())),
        "due": entry.due.text,
        "tasks": due.iter().map(|task| json!({
            "id": task.id,
            "title": task.line.title,
            "shown": !task.line.detail.is_empty(),
            "detail": task.line.detail,
            "colour": if task.overdue { "Attention" } else { "Default" },
            "offered": task.done.is_some(),
            "done": task.done.clone().unwrap_or_default(),
        })).collect::<Vec<_>>(),
    })
}

/// What a running timer has logged at `now`, counting from `counting_from`, as the core says a
/// length: "25 minutes".
fn logged(counting_from: &str, now: Timestamp) -> Option<String> {
    let from: Timestamp = counting_from.parse().ok()?;
    let minutes = now.duration_since(from).as_mins().max(0);
    Some(lumenna_records::words::duration(u32::try_from(minutes).unwrap_or(u32::MAX)))
}

/// The entry to show at `now`: the last of the timeline's moments to have begun.
pub fn current(entries: &[GlanceEntry], now: Timestamp) -> Option<&GlanceEntry> {
    entries.iter().rev().find(|entry| entry.moment.at.parse::<Timestamp>().is_ok_and(|at| at <= now)).or(entries.first())
}

/// When the card must next be drawn again after `now`, the store unchanged: the timeline's next
/// moment, or sooner the next minute a running timer reaches.
pub fn redraw_at(entries: &[GlanceEntry], now: Timestamp) -> Option<Timestamp> {
    let next = entries.iter().filter_map(|entry| entry.moment.at.parse::<Timestamp>().ok()).find(|at| *at > now);
    let minute = current(entries, now)
        .and_then(|entry| entry.moment.timer.as_ref())
        .and_then(|timer| timer.counting_from.parse::<Timestamp>().ok())
        .and_then(|from| {
            let passed = now.duration_since(from).as_mins().max(0);
            from.checked_add(jiff::SignedDuration::from_mins(passed + 1)).ok()
        });
    [next, minute].into_iter().flatten().min()
}

/// The task a Mark Done names, from the verb and data the card sent back.
pub fn completing(verb: &str, data: &str) -> Option<String> {
    if verb != COMPLETE {
        return None;
    }
    let data: Value = serde_json::from_str(data).ok()?;
    data["id"].as_str().filter(|id| !id.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumenna_records::glance::{GlanceMoment, GlanceTask, GlanceTimer};

    struct TwentyFour;

    impl Clock for TwentyFour {
        fn time(&self, clock: &str) -> String {
            clock.to_owned()
        }

        fn day(&self, iso: &str) -> String {
            iso.to_owned()
        }
    }

    fn moment(at: &str) -> GlanceMoment {
        GlanceMoment {
            at: at.to_owned(),
            date: "2026-10-25".to_owned(),
            current: None,
            next: None,
            timer: None,
            tasks: Vec::new(),
            due: 0,
            overdue: 0,
            blocks_left: 0,
        }
    }

    fn entry(moment: GlanceMoment) -> GlanceEntry {
        lumenna_app::glance::worded(moment, &TwentyFour)
    }

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    #[test]
    fn both_templates_are_adaptive_cards() {
        for template in [SMALL, MEDIUM] {
            let card: Value = serde_json::from_str(template).unwrap();
            assert_eq!(card["type"], "AdaptiveCard");
        }
        assert!(MEDIUM.contains(&format!("\"verb\": \"{COMPLETE}\"")), "Mark Done sends the verb taken back");
    }

    #[test]
    fn a_running_timer_shows_what_it_has_logged_in_the_cores_words() {
        let timer = GlanceTimer {
            sitting: "s".to_owned(),
            task: "Write the report".to_owned(),
            block: "Deep work".to_owned(),
            counting_from: "2026-10-25T09:00:00Z".to_owned(),
        };
        let shown = entry(GlanceMoment { timer: Some(timer), ..moment("2026-10-25T09:00:00Z") });
        let data = data(&shown, &[], at("2026-10-25T10:25:30Z"));
        assert_eq!(data["headline"]["title"], "Write the report");
        assert_eq!(data["timer"]["text"], "1 hour 25 minutes");
        assert_eq!(data["timer"]["shown"], true);
    }

    #[test]
    fn nothing_running_shows_no_timer_and_no_next_block() {
        let data = data(&entry(moment("2026-10-25T09:00:00Z")), &[], at("2026-10-25T09:00:00Z"));
        assert_eq!(data["timer"]["shown"], false);
        assert_eq!(data["next"]["shown"], false);
        assert_eq!(data["headline"]["shown"], false);
        assert_eq!(data["due"], "Nothing due");
    }

    #[test]
    fn a_task_the_listing_still_has_reads_as_its_row_and_offers_its_mark_done() {
        let task = GlanceTask { id: "t1".to_owned(), title: "Call the bank".to_owned(), due: None, due_time: None, overdue: true };
        let shown = entry(GlanceMoment { tasks: vec![task.clone()], due: 1, overdue: 1, ..moment("2026-10-25T09:00:00Z") });
        let mut row = lumenna_records::RowView {
            row: 1,
            id: "t1".to_owned(),
            role: "task".to_owned(),
            title: "Call the bank".to_owned(),
            state: vec!["overdue".to_owned()],
            due: Some("due yesterday".to_owned()),
            ..Default::default()
        };
        let rows = |row: &lumenna_records::RowView| Rows { rows: vec![row.clone()], ..Rows::default() };
        let named = due(&shown, &rows(&row), &TwentyFour);
        assert_eq!(named[0].line.title, "Call the bank");
        assert!(named[0].line.detail.starts_with("due yesterday"), "{}", named[0].line.detail);
        assert_eq!(named[0].done, None, "a row that offers no Mark Done has none");
        row.actions = vec![lumenna_records::Action::new(ActionKind::MarkDone, lumenna_records::Subject::Task, "Mark Done", "t1".to_owned())];
        let named = due(&shown, &rows(&row), &TwentyFour);
        assert_eq!(named[0].done.as_deref(), Some("Mark Done"));
        let data = data(&shown, &named, at("2026-10-25T09:00:00Z"));
        assert_eq!(data["tasks"][0]["colour"], "Attention");
        assert_eq!(data["tasks"][0]["offered"], true);
        let gone = due(&shown, &Rows::default(), &TwentyFour);
        assert_eq!(gone[0].line.text, "Call the bank");
        assert_eq!(gone[0].done, None);
    }

    #[test]
    fn the_card_is_drawn_again_at_the_next_moment_or_sooner_at_the_timers_next_minute() {
        let entries = vec![entry(moment("2026-10-25T09:00:00Z")), entry(moment("2026-10-25T11:00:00Z"))];
        assert_eq!(redraw_at(&entries, at("2026-10-25T09:30:00Z")), Some(at("2026-10-25T11:00:00Z")));
        assert_eq!(current(&entries, at("2026-10-25T11:30:00Z")).unwrap().moment.at, "2026-10-25T11:00:00Z");
        let timer = GlanceTimer {
            sitting: "s".to_owned(),
            task: "Write".to_owned(),
            block: String::new(),
            counting_from: "2026-10-25T09:00:10Z".to_owned(),
        };
        let running = vec![entry(GlanceMoment { timer: Some(timer), ..moment("2026-10-25T09:00:00Z") })];
        assert_eq!(redraw_at(&running, at("2026-10-25T09:30:20Z")), Some(at("2026-10-25T09:31:10Z")));
    }

    #[test]
    fn only_mark_done_with_a_task_completes_one() {
        assert_eq!(completing(COMPLETE, r#"{"id":"t1"}"#).as_deref(), Some("t1"));
        assert_eq!(completing("open", r#"{"id":"t1"}"#), None);
        assert_eq!(completing(COMPLETE, "{}"), None);
        assert_eq!(completing(COMPLETE, "not json"), None);
    }
}
