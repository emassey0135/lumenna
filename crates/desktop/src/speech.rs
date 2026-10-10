//! How each line reads, assembled from the components the core sends.
//!
//! A tree item has one text, and a screen reader reads it as the item's name, so the
//! components are joined here: the title first and verbatim — it is what a person scans for —
//! then the value and the states that mean something. What the tree reports from its own
//! structure — `SysTreeView32` on Windows, the tree items the GTK app annotates — is left out: the level, the position in the set and its size, whether a row
//! is expanded, and whether a checkbox is checked. Saying those in the text as well would say
//! them twice.

use lumenna_surface::words::duration;
use lumenna_surface::{Candidate, CancelledBlock, PlanAssignment, PlanBlock, RowView, TaskDetail};

/// How this device says times and days. The core sends `HH:MM` and ISO dates, which are
/// components; whether that is "2:30 PM" or "14:30" is the person's locale, so it is decided
/// by the platform.
pub trait Clock {
    /// `14:30` as this device says it.
    fn time(&self, clock: &str) -> String;
    /// An ISO date as a person says it: "Today", or "Monday 5 October".
    fn day(&self, iso: &str) -> String;
}

/// One thing a picker offers: a block as "Tomorrow, 9:00 AM to 11:00 AM, Deep work", in
/// this device's clock; anything else as its title, then what tells it apart (a task's
/// project). Depth is left to the list, which says a level as its platform does.
pub fn choice(choice: &lumenna_surface::Choice, clock: &dyn Clock) -> String {
    match (&choice.date, &choice.start, &choice.end) {
        (Some(date), Some(start), Some(end)) => {
            format!("{}, {} to {}, {}", clock.day(date), clock.time(start), clock.time(end), choice.title)
        }
        _ => match &choice.detail {
            Some(detail) => format!("{}, {detail}", choice.title),
            None => choice.title.clone(),
        },
    }
}

/// One row of a listing: the title, then when it is due, its value and notable states.
///
/// `ready` is true of almost every task, and saying it everywhere buries the states that
/// mean something. `completed` is left to the checkbox where there is one.
pub fn row(row: &RowView, checkbox: bool, clock: &dyn Clock) -> String {
    let mut parts = vec![row.title.clone()];
    parts.extend(due(row, clock));
    parts.extend(row.value.clone());
    parts.extend(
        row.trailing_states()
            .filter(|state| !(checkbox && *state == "completed"))
            .map(str::to_owned),
    );
    join(parts)
}

/// A row in the trash, where every row is `deleted` and saying so on each is noise.
pub fn trashed(row: &RowView, clock: &dyn Clock) -> String {
    let mut parts = vec![row.title.clone()];
    parts.extend(due(row, clock));
    parts.extend(row.value.clone());
    parts.extend(row.trailing_states().filter(|state| *state != "deleted").map(str::to_owned));
    join(parts)
}

/// When a row is due, with the time in this device's clock: "due tomorrow at 3:00 PM".
fn due(row: &RowView, clock: &dyn Clock) -> Option<String> {
    let due = row.due.clone()?;
    Some(match &row.due_time {
        Some(time) => format!("{due} at {}", clock.time(time)),
        None => due,
    })
}

/// A task's computed states for its details, and the rule it repeats by when the date
/// grammar cannot say it — which the Repeats field then shows empty and leaves alone.
pub fn task_state(task: &TaskDetail) -> String {
    let states: Vec<&str> = task.state.iter().map(String::as_str).filter(|s| *s != "ready").collect();
    let mut text = if states.is_empty() { "open".to_owned() } else { states.join(", ") };
    if task.repetition.is_none()
        && let Some(rule) = &task.recurrence
    {
        text.push_str(&format!(", repeats by the rule {rule}"));
    }
    text
}

/// A place in the sidebar, with what is in it: "Work, 3 tasks".
pub fn place(title: &str, detail: &str) -> String {
    lumenna_surface::places::line(title, detail)
}

/// A block on the day: "9:00 AM to 11:00 AM, Deep work, 2 hours, work block, now, 3 tasks
/// assigned" — the time and title, then the details the core words for every app.
pub fn block(block: &PlanBlock, clock: &dyn Clock) -> String {
    let mut parts = vec![format!("{} to {}", clock.time(&block.start), clock.time(&block.end)), block.title.clone()];
    parts.extend(block.details.iter().cloned());
    join(parts)
}

/// A sitting: a task in a block for one session — its title, then the details the
/// core words for every app, a capped timer among them, never presented as fact.
pub fn sitting(sitting: &PlanAssignment) -> String {
    let mut parts = vec![sitting.title.clone()];
    parts.extend(sitting.details.iter().cloned());
    join(parts)
}

/// Free time, which a timeline shows by empty space and a list has to say.
pub fn free(start: &str, end: &str, minutes: u32, clock: &dyn Clock) -> String {
    format!("Free, {}, {} to {}", duration(minutes), clock.time(start), clock.time(end))
}

/// Free time from its item, in the order every app says it: its title, its details, then
/// its span in this device's clock.
pub fn free_time(title: &str, details: &[String], start: &str, end: &str, clock: &dyn Clock) -> String {
    let mut parts = vec![title.to_owned()];
    parts.extend(details.iter().cloned());
    parts.push(format!("{} to {}", clock.time(start), clock.time(end)));
    join(parts)
}

/// Where the present falls: a position, not a highlight.
pub fn now(time: &str, clock: &dyn Clock) -> String {
    format!("Now, {}", clock.time(time))
}

/// A repeating block cancelled for this day alone.
pub fn cancelled(block: &CancelledBlock, clock: &dyn Clock) -> String {
    let mut parts = vec![clock.time(&block.start), block.title.clone()];
    parts.extend(block.details.iter().cloned());
    join(parts)
}

/// The day's first row: what a glance at a timeline gives a sighted user.
pub fn summary(date: &str, summary: &str, clock: &dyn Clock) -> String {
    let day = clock.day(date);
    if summary.is_empty() { day } else { format!("{day}. {summary}") }
}

/// A completion as a menu item: its name first, then what kind of thing it is — "Work,
/// project", not the core's "project Work".
///
/// The name is what a person scans for, and a menu jumps to an item by its first letter, so
/// leading with the kind would put every project under P and every label under L.
pub fn candidate(candidate: &Candidate) -> String {
    let name = candidate
        .label
        .strip_prefix(candidate.kind.as_str())
        .and_then(|rest| rest.strip_prefix(' '))
        .unwrap_or(&candidate.label);
    format!("{name}, {}", candidate.kind)
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

    /// Says 14:30 as "2:30 PM".
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
            due: None,
            due_time: None,
            value: value.map(str::to_owned),
            hint: None,
            actions: Vec::new(),
        }
    }

    fn sitting_with(status: &str, planned: Option<u32>, minutes: u32) -> PlanAssignment {
        let sitting = PlanAssignment {
            row: 1,
            id: "a".to_owned(),
            task: "t".to_owned(),
            title: "Write report".to_owned(),
            status: status.to_owned(),
            planned_mins: planned,
            minutes,
            capped: false,
            running: false,
            details: Vec::new(),
            actions: Vec::new(),
        };
        PlanAssignment { details: lumenna_surface::words::sitting_details(&sitting), ..sitting }
    }

    #[test]
    fn a_task_reads_title_first_then_its_date_and_the_states_that_mean_something() {
        let row = task("Buy milk", Some("priority 1"), &["overdue", "ready", "recurring"], false);
        let row = RowView { due: Some("due tomorrow".to_owned()), ..row };
        assert_eq!(super::row(&row, true, &TwelveHour), "Buy milk, due tomorrow, priority 1, overdue, recurring");
    }

    #[test]
    fn a_due_time_is_said_in_this_devices_clock() {
        let row = task("Call the bank", None, &[], false);
        let row = RowView { due: Some("due today".to_owned()), due_time: Some("15:00".to_owned()), ..row };
        assert_eq!(super::row(&row, true, &TwelveHour), "Call the bank, due today at 3:00 PM");
    }

    #[test]
    fn completed_is_left_to_the_checkbox_where_there_is_one() {
        let row = task("Buy milk", None, &["completed"], true);
        assert_eq!(super::row(&row, true, &TwelveHour), "Buy milk");
        assert_eq!(super::row(&row, false, &TwelveHour), "Buy milk, completed");
    }

    #[test]
    fn the_trash_does_not_say_deleted_on_every_row() {
        let row = task("Buy milk", None, &["deleted", "completed"], true);
        let row = RowView { due: Some("due Monday".to_owned()), ..row };
        assert_eq!(trashed(&row, &TwelveHour), "Buy milk, due Monday, completed");
    }

    #[test]
    fn a_title_is_never_abbreviated_or_rearranged() {
        let row = task("Call Sam, re: the 3:00 thing", None, &[], false);
        assert_eq!(super::row(&row, true, &TwelveHour), "Call Sam, re: the 3:00 thing");
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
            accepts_tasks: true,
            counts_capacity: true,
            anchored: false,
            colour: None,
            notes: String::new(),
            details: Vec::new(),
            actions: Vec::new(),
        };
        let block = PlanBlock { details: lumenna_surface::words::block_details(&block), ..block };
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
            accepts_tasks: false,
            counts_capacity: false,
            anchored: false,
            colour: None,
            notes: String::new(),
            details: Vec::new(),
            actions: Vec::new(),
        };
        let block = PlanBlock { details: lumenna_surface::words::block_details(&block), ..block };
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

    fn completion(kind: &str, label: &str) -> Candidate {
        Candidate { text: String::new(), kind: kind.to_owned(), label: label.to_owned() }
    }

    #[test]
    fn a_completion_leads_with_its_name_so_its_first_letter_finds_it() {
        assert_eq!(candidate(&completion("project", "project Home Office")), "Home Office, project");
        assert_eq!(candidate(&completion("label", "label calls")), "calls, label");
        assert_eq!(candidate(&completion("date", "date next friday")), "next friday, date");
    }

    #[test]
    fn a_completion_whose_label_does_not_start_with_its_kind_is_said_whole() {
        assert_eq!(candidate(&completion("keyword", "overdue")), "overdue, keyword");
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
