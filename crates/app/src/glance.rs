//! A glance worded (`lumenna_records::glance`): what a widget, complication or tile says, in
//! this device's clock, the same on every platform. Each [`GlanceEntry`] keeps its moment's
//! parts beside the lines, for a platform that draws a block's progress or counts a timer.

use lumenna_records::glance::{GlanceBlock, GlanceMoment};
use lumenna_records::words;

use crate::speech::{Clock, Line};

/// One moment of a glance, worded.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GlanceEntry {
    /// The moment, as parts.
    pub moment: GlanceMoment,
    /// What is happening now: the timer running, else the block under way, else the free time
    /// before the next block, else that nothing more is planned. A complication's one line.
    pub headline: Line,
    /// The emoji of what the headline names first (the task timed, the block under way), to
    /// draw before it: never in a line, nor in `spoken`. The next block's is `moment.next`'s,
    /// and each task's `moment.tasks`', in the order of `tasks`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headline_emoji: Option<String>,
    /// The next block, as the day says one: its times, then its name.
    pub next: Option<Line>,
    /// What is due: how many tasks, and how many overdue.
    pub due: Line,
    /// Each task the moment names, as a task row says it: its title, then when it is due and
    /// whether it is overdue.
    pub tasks: Vec<Line>,
    /// What a control that marks one of them done is called, in Title Case and in sentence
    /// case, for a widget's button: the core's own action's name.
    pub mark_done: String,
    /// The same in sentence case.
    pub mark_done_sentence: String,
    /// How the person said their energy is today: "Energy, Low today", "Energy, Not said today".
    pub energy: Line,
    /// What the button that opens the day's check-in is called, in Title Case: the
    /// `energy-today` launch shortcut's "Energy Today".
    pub check_in: String,
    /// The same in sentence case.
    pub check_in_sentence: String,
    /// The whole glance as one sentence, for a screen reader's label of the widget.
    pub spoken: String,
}

/// `moment` worded in `clock`.
#[must_use]
pub fn worded(moment: GlanceMoment, clock: &dyn Clock) -> GlanceEntry {
    let headline = match (&moment.timer, &moment.current, &moment.next) {
        (Some(timer), _, _) if timer.block.is_empty() => Line::of(timer.task.clone(), vec![words::timer_running()]),
        (Some(timer), _, _) => Line::of(timer.task.clone(), vec![words::timer_running(), words::in_block(&timer.block)]),
        (None, Some(block), _) => Line::of(block.title.clone(), vec![words::until(&clock.time(&block.end))]),
        (None, None, Some(next)) => {
            Line::of(words::free(), vec![words::until(&clock.time(&next.start)), words::then(&next.title)])
        }
        (None, None, None) => Line::of(words::nothing_more_planned(), Vec::new()),
    };
    let headline_emoji = match (&moment.timer, &moment.current) {
        (Some(timer), _) => timer.emoji.clone(),
        (None, Some(block)) => block.emoji.clone(),
        (None, None) => None,
    };
    let next = moment.next.as_ref().map(|block| block_line(block, clock));
    let due = match (moment.due, moment.overdue) {
        (0, _) | (_, 0) => Line::of(words::tasks_due(moment.due), Vec::new()),
        (_, overdue) => Line::of(words::tasks_due(moment.due), vec![words::overdue_count(overdue)]),
    };
    let mut said = vec![headline.text.clone()];
    if let (Some(next), true) = (&next, moment.current.is_some() || moment.timer.is_some()) {
        said.push(words::next(&next.text));
    }
    said.push(due.text.clone());
    let level: Option<lumenna_core::energy::Energy> = moment.energy.parse().ok();
    let mut settings = lumenna_core::model::Settings::default();
    let day = moment.date.parse::<jiff::civil::Date>().unwrap_or_default();
    settings.energy_today = level.map(|level| lumenna_core::energy::CheckIn { date: day, level });
    let energy = Line::of("Energy".to_owned(), vec![lumenna_records::energy::today_line(&settings, day)]);
    // Said only once the person has said something of the day.
    if level.is_some() {
        said.push(energy.text.clone());
    }
    let tasks = moment
        .tasks
        .iter()
        .map(|task| {
            let mut rest: Vec<String> = crate::speech::due_words(task.due.as_deref(), task.due_time.as_deref(), clock).into_iter().collect();
            if task.overdue {
                rest.push("overdue".to_owned());
            }
            // A repeating task's streak last, as its details say it.
            rest.extend(task.streak.clone());
            Line::of(task.title.clone(), rest)
        })
        .collect();
    let mark_done = lumenna_records::Action::new(
        lumenna_records::ActionKind::MarkDone,
        lumenna_records::Subject::Task,
        lumenna_records::actions::MARK_DONE,
        String::new(),
    );
    GlanceEntry {
        spoken: said.join(". ") + ".",
        moment,
        headline,
        headline_emoji,
        next,
        due,
        tasks,
        mark_done: mark_done.title,
        mark_done_sentence: mark_done.sentence,
        energy,
        check_in: lumenna_records::energy::CHECK_IN.to_owned(),
        check_in_sentence: lumenna_records::fields::sentence_case(lumenna_records::energy::CHECK_IN.to_owned()),
    }
}

/// The glance now of a day with nothing in it (`lumenna_records::glance::empty`), worded in
/// `clock`: what a widget shows before any glance was written for it.
#[must_use]
pub fn empty(clock: &dyn Clock) -> GlanceEntry {
    worded(lumenna_records::glance::empty(&jiff::Zoned::now()), clock)
}

/// A block as the day says one: "<start> to <end>, <title>".
fn block_line(block: &GlanceBlock, clock: &dyn Clock) -> Line {
    Line::of(format!("{} to {}", clock.time(&block.start), clock.time(&block.end)), vec![block.title.clone()])
}

/// The moment of a timeline (`App::glance`, a widget's file) to show at `now`: the last whose
/// moment has come, else the first. Every platform's widget picks it so.
#[must_use]
pub fn current(entries: &[GlanceEntry], now: jiff::Timestamp) -> Option<&GlanceEntry> {
    entries.iter().rev().find(|entry| entry.moment.at.parse::<jiff::Timestamp>().is_ok_and(|at| at <= now)).or(entries.first())
}

/// When a widget showing `entries` at `now` is next to be drawn again: the next moment of the
/// timeline, or sooner, the next whole minute of a timer the current moment shows running, for a
/// widget that cannot count one itself. None when nothing more changes.
#[must_use]
pub fn redraw_at(entries: &[GlanceEntry], now: jiff::Timestamp) -> Option<jiff::Timestamp> {
    let next = entries.iter().filter_map(|entry| entry.moment.at.parse::<jiff::Timestamp>().ok()).find(|at| *at > now);
    let timer = current(entries, now).and_then(|entry| entry.moment.timer.as_ref());
    let minute = timer.and_then(|timer| timer.counting_from.parse::<jiff::Timestamp>().ok()).and_then(|from| {
        let passed = now.duration_since(from).as_mins().max(0);
        from.checked_add(jiff::SignedDuration::from_mins(passed + 1)).ok()
    });
    // A countdown's minutes turn at whole minutes before it runs out, not since it started.
    let down = timer.and_then(|timer| timer.runs_out_at.as_deref()?.parse::<jiff::Timestamp>().ok()).and_then(|out| {
        let secs = out.duration_since(now).as_secs();
        let step = if secs > 0 { (secs - 1).rem_euclid(60) + 1 } else { 60 - (-secs).rem_euclid(60) };
        now.checked_add(jiff::SignedDuration::from_secs(step)).ok()
    });
    [next, minute, down].into_iter().flatten().min()
}

/// How much of a timer's planned length is left at `now`, said: "12 minutes left", "time is up",
/// "3 minutes over". None for a timer with no planned length, which only counts up (`logged`).
/// What a widget, a tray or a menu bar item, a Live Activity's or an ongoing notification's text
/// shows beside the task, drawn again at `redraw_at`.
#[must_use]
pub fn countdown(timer: &lumenna_records::glance::GlanceTimer, now: jiff::Timestamp) -> Option<String> {
    let planned = i64::from(timer.planned_mins?) * 60;
    let left = match timer.runs_out_at.as_deref().and_then(|at| at.parse::<jiff::Timestamp>().ok()) {
        Some(out) => out.duration_since(now).as_secs(),
        None => planned - now.duration_since(timer.counting_from.parse::<jiff::Timestamp>().ok()?).as_secs().max(0),
    };
    Some(words::left(left))
}

/// The timer's control as a menu's item reads it at `now`: what a press does, what it runs on or
/// would start, and, while it runs with a planned length, what is left ("Stop Timer, Essay, 12
/// minutes left"), else how long so far. `sentence` for a shell writing sentence case. What a
/// tray's or a menu bar's item says, drawn again at least each minute while a timer runs.
#[must_use]
pub fn control_line(control: &lumenna_records::controls::TimerControl, now: jiff::Timestamp, sentence: bool) -> String {
    let action = if sentence { &control.action_sentence } else { &control.action };
    joined([Some(action.clone()), Some(control_value(control, now))])
}

/// What a timer's control shows beneath its name at `now`, where what a press does is its hint
/// (Control Center's toggle): the task it runs on, then what is left of the planned length, else
/// how long so far ("Essay, 12 minutes left"); while none runs, what a press would start, or why
/// nothing would.
#[must_use]
pub fn control_value(control: &lumenna_records::controls::TimerControl, now: jiff::Timestamp) -> String {
    let count = control.timer.as_ref().and_then(|timer| countdown(timer, now).or_else(|| logged(&timer.counting_from, now)));
    joined([Some(control.detail.clone()), count])
}

/// The parts that say something, joined as a line's are.
fn joined<const N: usize>(parts: [Option<String>; N]) -> String {
    parts.into_iter().flatten().filter(|part| !part.trim().is_empty()).collect::<Vec<_>>().join(", ")
}

/// How long a timer counting from `counting_from` (RFC 3339) has run at `now`, in whole minutes,
/// said: "25 minutes". None for a moment that does not read.
#[must_use]
pub fn logged(counting_from: &str, now: jiff::Timestamp) -> Option<String> {
    let from: jiff::Timestamp = counting_from.parse().ok()?;
    let minutes = now.duration_since(from).as_mins().max(0);
    Some(words::duration(u32::try_from(minutes).unwrap_or(u32::MAX)))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TwentyFour;

    impl Clock for TwentyFour {
        fn time(&self, clock: &str) -> String {
            clock.to_owned()
        }

        fn day(&self, iso: &str) -> String {
            iso.to_owned()
        }
    }

    fn block(title: &str, start: &str, end: &str) -> GlanceBlock {
        GlanceBlock {
            id: format!("{title}@2026-10-25"),
            title: title.to_owned(),
            start: start.to_owned(),
            end: end.to_owned(),
            starts_at: String::new(),
            ends_at: String::new(),
            kind: "work".to_owned(),
            colour: None,
            // Every block here wears one, so every line below is seen to leave it out.
            emoji: Some("🧠".to_owned()),
            tasks: Vec::new(),
        }
    }

    fn moment() -> GlanceMoment {
        GlanceMoment {
            at: "2026-10-25T09:30:00Z".to_owned(),
            date: "2026-10-25".to_owned(),
            current: None,
            next: None,
            timer: None,
            tasks: Vec::new(),
            due: 0,
            overdue: 0,
            blocks_left: 0,
            energy: String::new(),
        }
    }

    #[test]
    fn a_block_under_way_says_until_when_then_what_comes_next_and_what_is_due() {
        let entry = worded(
            GlanceMoment { current: Some(block("Deep work", "09:00", "11:00")), next: Some(block("Lunch", "12:00", "13:00")), due: 3, overdue: 1, ..moment() },
            &TwentyFour,
        );
        assert_eq!(entry.headline.text, "Deep work, until 11:00");
        assert_eq!(entry.next.unwrap().text, "12:00 to 13:00, Lunch");
        assert_eq!(entry.due.text, "3 tasks due, 1 overdue");
        assert_eq!(entry.spoken, "Deep work, until 11:00. Next, 12:00 to 13:00, Lunch. 3 tasks due, 1 overdue.");
        assert_eq!(entry.headline_emoji.as_deref(), Some("🧠"), "drawn beside the headline, said nowhere");
    }

    #[test]
    fn each_task_due_reads_as_a_task_row_says_it_and_its_button_is_the_cores_mark_done() {
        let task = |title: &str, time: Option<&str>, overdue| lumenna_records::glance::GlanceTask {
            id: title.to_owned(),
            title: title.to_owned(),
            due: Some("due today".to_owned()),
            due_time: time.map(str::to_owned),
            overdue,
            streak: None,
            emoji: Some("📞".to_owned()),
        };
        let entry = worded(
            GlanceMoment { tasks: vec![task("Call the bank", Some("15:00"), false), task("Pay rent", None, true)], due: 2, overdue: 1, ..moment() },
            &TwentyFour,
        );
        let lines: Vec<&str> = entry.tasks.iter().map(|line| line.text.as_str()).collect();
        assert_eq!(lines, ["Call the bank, due today at 15:00", "Pay rent, due today, overdue"]);
        assert_eq!((entry.mark_done.as_str(), entry.mark_done_sentence.as_str()), ("Mark Done", "Mark done"));
    }

    #[test]
    fn a_repeating_tasks_streak_ends_its_glance_line() {
        let stretch = lumenna_records::glance::GlanceTask {
            id: "stretch".to_owned(),
            title: "Stretch".to_owned(),
            due: Some("due today".to_owned()),
            due_time: None,
            overdue: false,
            streak: Some("done 12 times running".to_owned()),
            emoji: None,
        };
        let entry = worded(GlanceMoment { tasks: vec![stretch], due: 1, ..moment() }, &TwentyFour);
        assert_eq!(entry.tasks[0].text, "Stretch, due today, done 12 times running");
    }

    #[test]
    fn a_widget_shows_the_last_moment_come_and_draws_again_at_the_next_or_a_timers_next_minute() {
        let at = |at: &str| GlanceMoment { at: at.to_owned(), ..moment() };
        let entries = vec![worded(at("2026-10-25T09:00:00Z"), &TwentyFour), worded(at("2026-10-25T10:00:00Z"), &TwentyFour)];
        let now = |t: &str| t.parse::<jiff::Timestamp>().unwrap();
        assert_eq!(current(&entries, now("2026-10-25T09:30:00Z")).unwrap().moment.at, "2026-10-25T09:00:00Z");
        assert_eq!(current(&entries, now("2026-10-25T08:00:00Z")).unwrap().moment.at, "2026-10-25T09:00:00Z", "before the first, the first");
        assert_eq!(redraw_at(&entries, now("2026-10-25T09:30:00Z")), Some(now("2026-10-25T10:00:00Z")));
        assert_eq!(redraw_at(&entries, now("2026-10-25T10:30:00Z")), None);
        assert_eq!(logged("2026-10-25T09:00:00Z", now("2026-10-25T09:25:30Z")).as_deref(), Some("25 minutes"));
    }

    #[test]
    fn a_timer_with_a_planned_length_counts_down_to_it_and_on_past_it_and_one_without_only_up() {
        let now = |t: &str| t.parse::<jiff::Timestamp>().unwrap();
        let mut timer = lumenna_records::glance::GlanceTimer {
            sitting: "s".to_owned(),
            task: "Essay".to_owned(),
            block: String::new(),
            counting_from: "2026-10-25T09:00:00Z".to_owned(),
            planned_mins: Some(25),
            runs_out_at: Some("2026-10-25T09:25:00Z".to_owned()),
            emoji: None,
        };
        assert_eq!(countdown(&timer, now("2026-10-25T09:12:30Z")).as_deref(), Some("13 minutes left"));
        assert_eq!(countdown(&timer, now("2026-10-25T09:25:20Z")).as_deref(), Some("time is up"));
        assert_eq!(countdown(&timer, now("2026-10-25T09:28:00Z")).as_deref(), Some("3 minutes over"));
        timer.runs_out_at = None;
        assert_eq!(countdown(&timer, now("2026-10-25T09:30:00Z")).as_deref(), Some("5 minutes over"), "over before it started");
        timer.planned_mins = None;
        assert_eq!(countdown(&timer, now("2026-10-25T09:30:00Z")), None);

        let entries = vec![worded(
            GlanceMoment {
                timer: Some(lumenna_records::glance::GlanceTimer {
                    planned_mins: Some(25),
                    runs_out_at: Some("2026-10-25T09:25:30Z".to_owned()),
                    emoji: None,
                    ..timer
                }),
                ..moment()
            },
            &TwentyFour,
        )];
        assert_eq!(redraw_at(&entries, now("2026-10-25T09:40:10Z")), Some(now("2026-10-25T09:40:30Z")), "the countdown's minute, before the count up's");
    }

    #[test]
    fn a_trays_timer_item_says_what_a_press_does_on_what_and_what_is_left() {
        let now = "2026-10-25T09:12:30Z".parse::<jiff::Timestamp>().unwrap();
        let mut control = lumenna_records::controls::TimerControl {
            running: true,
            detail: "Essay".to_owned(),
            action: "Stop Timer".to_owned(),
            action_sentence: "Stop timer".to_owned(),
            enabled: true,
            timer: Some(lumenna_records::glance::GlanceTimer {
                sitting: "s".to_owned(),
                task: "Essay".to_owned(),
                block: String::new(),
                counting_from: "2026-10-25T09:00:00Z".to_owned(),
                planned_mins: Some(25),
                runs_out_at: Some("2026-10-25T09:25:00Z".to_owned()),
                emoji: None,
            }),
        };
        assert_eq!(control_line(&control, now, false), "Stop Timer, Essay, 13 minutes left");
        control.timer.as_mut().unwrap().planned_mins = None;
        assert_eq!(control_line(&control, now, true), "Stop timer, Essay, 12 minutes", "no plan: how long so far");
        control.timer = None;
        assert_eq!(control_line(&control, now, false), "Stop Timer, Essay");
    }

    #[test]
    fn free_time_says_until_the_next_block_and_an_empty_day_says_so() {
        let free = worded(GlanceMoment { next: Some(block("Meeting", "14:00", "15:00")), ..moment() }, &TwentyFour);
        assert_eq!(free.headline.text, "Free, until 14:00, then Meeting");
        let empty = worded(moment(), &TwentyFour);
        assert_eq!(empty.spoken, "Nothing more planned today. Nothing due.");
        assert_eq!(super::empty(&TwentyFour).spoken, empty.spoken);
    }
}
