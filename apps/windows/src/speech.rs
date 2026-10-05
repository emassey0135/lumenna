//! How each line reads, assembled from the components the core sends (§13).
//!
//! A tree item has one text, and a screen reader reads it as the item's name, so the
//! components are joined here: the title first and verbatim — it is what a person scans for —
//! then the value and the states that mean something. What `SysTreeView32` reports from its
//! own structure is left out: the level, the position in the set and its size, whether a row
//! is expanded, and whether a checkbox is checked. Saying those in the text as well would say
//! them twice.

use lumenna_surface::words::duration;
use lumenna_surface::{CancelledBlock, PlanAssignment, PlanBlock, RowView, sitting_status};

/// How this device says times and days. The core sends `HH:MM` and ISO dates, which are
/// components; whether that is "2:30 PM" or "14:30" is the person's locale, so it is decided
/// by the platform.
pub trait Clock {
    /// `14:30` as this device says it.
    fn time(&self, clock: &str) -> String;
    /// An ISO date as a person says it: "Today", or "Monday 5 October".
    fn day(&self, iso: &str) -> String;
}

/// One row of a listing: the title, then its value and notable states.
///
/// `ready` is true of almost every task, and saying it everywhere buries the states that
/// mean something. `completed` is left to the checkbox where there is one.
pub fn row(row: &RowView, checkbox: bool) -> String {
    let mut parts = vec![row.title.clone()];
    parts.extend(row.value.clone());
    parts.extend(
        row.trailing_states()
            .filter(|state| !(checkbox && *state == "completed"))
            .map(str::to_owned),
    );
    join(parts)
}

/// A row in the trash, where every row is `deleted` and saying so on each is noise.
pub fn trashed(row: &RowView) -> String {
    let mut parts = vec![row.title.clone()];
    parts.extend(row.value.clone());
    parts.extend(row.trailing_states().filter(|state| *state != "deleted").map(str::to_owned));
    join(parts)
}

/// A place in the sidebar, with what is in it: "Work, 3 tasks".
pub fn place(title: &str, detail: &str) -> String {
    if detail.is_empty() { title.to_owned() } else { format!("{title}, {detail}") }
}

/// A block on the day: "9:00 AM to 11:00 AM, Deep work, 2 hours, work block, now, 3 tasks
/// assigned". The kind is said because it says which actions exist (§13): only work blocks
/// take tasks.
pub fn block(block: &PlanBlock, clock: &dyn Clock) -> String {
    let mut parts = vec![
        format!("{} to {}", clock.time(&block.start), clock.time(&block.end)),
        block.title.clone(),
        duration(block.duration_mins),
        format!("{} block", block.kind),
    ];
    if !block.when.is_empty() {
        parts.push(block.when.clone());
    }
    if block.changed_for_this_day {
        parts.push("changed for this day".to_owned());
    }
    if block.kind == "work" {
        parts.push(match block.assignments.len() {
            0 => "nothing assigned".to_owned(),
            1 => "1 task assigned".to_owned(),
            n => format!("{n} tasks assigned"),
        });
    }
    join(parts)
}

/// A sitting: a task in a block for one session (§3.7).
pub fn sitting(sitting: &PlanAssignment) -> String {
    let mut parts = vec![sitting.title.clone()];
    parts.extend(sitting_status(sitting.clone()));
    if sitting.minutes > 0 {
        parts.push(format!("{} logged", duration(sitting.minutes)));
    }
    if sitting.capped {
        // Never presented as fact (§3.7).
        parts.push("capped, the timer looks forgotten".to_owned());
    }
    join(parts)
}

/// Free time, which a timeline shows by empty space and a list has to say (§13).
pub fn free(start: &str, end: &str, minutes: u32, clock: &dyn Clock) -> String {
    format!("Free, {}, {} to {}", duration(minutes), clock.time(start), clock.time(end))
}

/// Where the present falls: a position, not a highlight (§13).
pub fn now(time: &str, clock: &dyn Clock) -> String {
    format!("Now, {}", clock.time(time))
}

/// A repeating block cancelled for this day alone.
pub fn cancelled(block: &CancelledBlock, clock: &dyn Clock) -> String {
    format!("{}, {}, cancelled for this day", clock.time(&block.start), block.title)
}

/// The day's first row: what a glance at a timeline gives a sighted user (§13).
pub fn summary(date: &str, summary: &str, clock: &dyn Clock) -> String {
    let day = clock.day(date);
    if summary.is_empty() { day } else { format!("{day}. {summary}") }
}

/// A result's announcement, then each notice.
pub fn announcement(announcement: &str, notices: &[String]) -> String {
    std::iter::once(announcement)
        .chain(notices.iter().map(String::as_str))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(". ")
}

/// The first letter capitalised, as the core's sentences begin lower-case for embedding.
pub fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

fn join(parts: Vec<String>) -> String {
    parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
pub mod testing {
    //! A clock for tests: twelve-hour times, and one fixed day called Today.

    use super::Clock;

    pub struct TwelveHour;

    impl Clock for TwelveHour {
        fn time(&self, clock: &str) -> String {
            let (hours, minutes) = clock.split_once(':').unwrap();
            let hours: u32 = hours.parse().unwrap();
            let half = if hours < 12 { "AM" } else { "PM" };
            format!("{}:{minutes} {half}", (hours + 11) % 12 + 1)
        }

        fn day(&self, iso: &str) -> String {
            if iso == "2026-10-04" { "Today".to_owned() } else { iso.to_owned() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::TwelveHour;
    use super::*;

    fn task(title: &str, value: Option<&str>, state: &[&str], checked: bool) -> RowView {
        RowView {
            row: 1,
            id: "0199".to_owned(),
            role: "task".to_owned(),
            depth: 0,
            index: 1,
            count: 1,
            checked: Some(checked),
            expanded: None,
            title: title.to_owned(),
            state: state.iter().map(|s| (*s).to_owned()).collect(),
            value: value.map(str::to_owned),
            hint: None,
        }
    }

    fn sitting_with(status: &str, planned: Option<u32>, minutes: u32) -> PlanAssignment {
        PlanAssignment {
            row: 1,
            id: "a".to_owned(),
            task: "t".to_owned(),
            title: "Write report".to_owned(),
            status: status.to_owned(),
            planned_mins: planned,
            minutes,
            capped: false,
        }
    }

    #[test]
    fn a_task_reads_title_first_then_its_date_and_the_states_that_mean_something() {
        let row = task("Buy milk", Some("tomorrow"), &["overdue", "ready", "recurring"], false);
        assert_eq!(super::row(&row, true), "Buy milk, tomorrow, overdue, recurring");
    }

    #[test]
    fn completed_is_left_to_the_checkbox_where_there_is_one() {
        let row = task("Buy milk", None, &["completed"], true);
        assert_eq!(super::row(&row, true), "Buy milk");
        assert_eq!(super::row(&row, false), "Buy milk, completed");
    }

    #[test]
    fn the_trash_does_not_say_deleted_on_every_row() {
        let row = task("Buy milk", Some("due 2026-10-05"), &["deleted", "completed"], true);
        assert_eq!(trashed(&row), "Buy milk, due 2026-10-05, completed");
    }

    #[test]
    fn a_title_is_never_abbreviated_or_rearranged() {
        let row = task("Call Sam, re: the 3:00 thing", None, &[], false);
        assert_eq!(super::row(&row, true), "Call Sam, re: the 3:00 thing");
    }

    #[test]
    fn a_work_block_says_its_times_kind_and_how_much_is_assigned() {
        let block = PlanBlock {
            row: 1,
            id: "s@2026-10-04".to_owned(),
            series: "s".to_owned(),
            title: "Deep work".to_owned(),
            start: "09:00".to_owned(),
            end: "11:00".to_owned(),
            duration_mins: 120,
            kind: "work".to_owned(),
            when: "now".to_owned(),
            repeats: true,
            changed_for_this_day: false,
            assignments: vec![sitting_with("planned", None, 0)],
        };
        assert_eq!(
            super::block(&block, &TwelveHour),
            "9:00 AM to 11:00 AM, Deep work, 2 hours, work block, now, 1 task assigned"
        );
    }

    #[test]
    fn a_break_says_nothing_about_assignments() {
        let block = PlanBlock {
            row: 1,
            id: "s@d".to_owned(),
            series: "s".to_owned(),
            title: "Lunch".to_owned(),
            start: "12:30".to_owned(),
            end: "13:15".to_owned(),
            duration_mins: 45,
            kind: "break".to_owned(),
            when: String::new(),
            repeats: false,
            changed_for_this_day: false,
            assignments: Vec::new(),
        };
        assert_eq!(super::block(&block, &TwelveHour), "12:30 PM to 1:15 PM, Lunch, 45 minutes, break block");
    }

    #[test]
    fn a_planned_length_never_reads_as_planned_twice() {
        assert_eq!(super::sitting(&sitting_with("planned", Some(45), 0)), "Write report, planned for 45 minutes");
        assert_eq!(
            super::sitting(&sitting_with("worked", Some(45), 50)),
            "Write report, worked, 45 minutes planned, 50 minutes logged"
        );
    }

    #[test]
    fn free_time_and_now_are_rows_of_their_own() {
        assert_eq!(free("10:15", "11:00", 45, &TwelveHour), "Free, 45 minutes, 10:15 AM to 11:00 AM");
        assert_eq!(now("19:32", &TwelveHour), "Now, 7:32 PM");
    }

    #[test]
    fn the_summary_names_the_day_first() {
        assert_eq!(
            summary("2026-10-04", "Two blocks, three hours of work.", &TwelveHour),
            "Today. Two blocks, three hours of work."
        );
    }

    #[test]
    fn an_announcement_is_followed_by_each_notice() {
        assert_eq!(
            announcement("Added Buy milk", &["new label 'shop'".to_owned()]),
            "Added Buy milk. new label 'shop'"
        );
        assert_eq!(announcement("", &[]), "");
    }

    #[test]
    fn a_sentence_starts_with_a_capital() {
        assert_eq!(sentence("no task matches 'x'"), "No task matches 'x'");
        assert_eq!(sentence(""), "");
    }
}
