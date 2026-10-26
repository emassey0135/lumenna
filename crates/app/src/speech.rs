//! How each line reads, assembled from the components the core sends.
//!
//! A tree item has one text, and a screen reader reads it as the item's name, so the
//! components are joined here: the title first and verbatim — it is what a person scans for —
//! then the value and the states that mean something. What the tree reports from its own
//! structure — `SysTreeView32` on Windows, the tree items the GTK app annotates — is left out: the level, the position in the set and its size, whether a row
//! is expanded, and whether a checkbox is checked. Saying those in the text as well would say
//! them twice.

use lumenna_records::{Candidate, CancelledBlock, PlanAssignment, PlanBlock, RowView, TaskDetail};

/// How this device says times and days. The core sends `HH:MM` and ISO dates, which are
/// components; whether that is "2:30 PM" or "14:30" is the person's locale, so it is decided
/// by the platform.
pub trait Clock {
    /// `14:30` as this device says it.
    fn time(&self, clock: &str) -> String;
    /// An ISO date as a person says it: "Today", or "Monday 5 October".
    fn day(&self, iso: &str) -> String;
}

/// What a platform formats in its own locale: a time of day, and a date. The rest of saying a
/// day — "Today", "Tomorrow", the year only when it is not this one — is [`Spoken`]'s, once.
pub trait Formats {
    /// A time of day as this device writes one: "2:30 PM", or "14:30".
    fn time(&self, hour: i8, minute: i8) -> String;
    /// A date with its weekday and month named, the year too when `with_year`: "Monday 5 October".
    fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String;
}

/// A [`Clock`] over a platform's [`Formats`], counting days from the device's today.
pub struct Spoken<F>(pub F);

impl<F: Formats> Clock for Spoken<F> {
    fn time(&self, clock: &str) -> String {
        match clock.parse::<jiff::civil::Time>() {
            Ok(time) => self.0.time(time.hour(), time.minute()),
            Err(_) => clock.to_owned(),
        }
    }

    fn day(&self, iso: &str) -> String {
        let Ok(date) = iso.parse::<jiff::civil::Date>() else { return iso.to_owned() };
        let today = jiff::Zoned::now().date();
        if date == today {
            "Today".to_owned()
        } else if today.tomorrow().is_ok_and(|d| d == date) {
            "Tomorrow".to_owned()
        } else if today.yesterday().is_ok_and(|d| d == date) {
            "Yesterday".to_owned()
        } else {
            // The year only when it is not this one, as a person would say it.
            self.0.date(date.year(), date.month(), date.day(), date.year() != today.year())
        }
    }
}

/// A line as read: what comes first — the name, or a block's times and name — then the rest.
/// A list that shows two lines shows them so; one that reads one line reads [`Line::text`].
#[boltffi::data]
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Line {
    /// What comes first.
    pub title: String,
    /// The rest, joined: empty for nothing more.
    pub detail: String,
    /// The whole line: the title, then the rest.
    pub text: String,
    /// The line for a braille display, where a device shows one apart from speech (BTSpeak's
    /// `braille_title`): a list row's states as their short forms ("ovd rec"), as BTSpeak's
    /// own mail list shows its statuses; else the line as said.
    pub braille: String,
}

impl Line {
    /// A line of `title`, then each of `rest` that says something.
    #[must_use]
    pub fn of(title: String, rest: Vec<String>) -> Self {
        let detail = join(rest);
        let text = join(vec![title.clone(), detail.clone()]);
        Self { braille: text.clone(), text, title, detail }
    }

    /// The whole line: the title, then the rest.
    #[must_use]
    pub fn text(&self) -> String {
        self.text.clone()
    }
}


/// [`choice`], as its first part and the rest: a block's whole line is its first.
pub fn choice_line(choice: &lumenna_records::Choice, clock: &dyn Clock) -> Line {
    match (&choice.date, &choice.start, &choice.end) {
        (Some(date), Some(start), Some(end)) => Line::of(
            format!("{}, {} to {}, {}", clock.day(date), clock.time(start), clock.time(end), choice.title),
            Vec::new(),
        ),
        _ => Line::of(choice.title.clone(), choice.detail.iter().cloned().collect()),
    }
}

/// One row of a listing: the title, then when it is due, its value and notable states.
///
/// `ready` is true of almost every task, and saying it everywhere buries the states that
/// mean something. `completed` is left to the checkbox where there is one.
pub fn row(row: &RowView, checkbox: bool, clock: &dyn Clock) -> String {
    row_line(row, checkbox, clock).text()
}

/// [`row`], as its title and the rest.
pub fn row_line(row: &RowView, checkbox: bool, clock: &dyn Clock) -> Line {
    let states: Vec<&str> = row.trailing_states().filter(|state| !(checkbox && *state == "completed")).collect();
    stated(row, clock, &states)
}

/// A row's line: the title, when it is due, its value and `states`, said as words and shown in
/// braille as their short forms.
fn stated(row: &RowView, clock: &dyn Clock, states: &[&str]) -> Line {
    let mut parts = Vec::new();
    parts.extend(due(row, clock));
    parts.extend(row.value.clone());
    let brailled = join(vec![row.title.clone(), join(parts.clone()), short_forms(states)]);
    parts.extend(states.iter().map(|state| (*state).to_owned()));
    Line { braille: brailled, ..Line::of(row.title.clone(), parts) }
}

/// States as a braille display shows them, the core's short forms with a space between
/// ("ovd rec"); a state this build does not know is shown as its word.
fn short_forms(states: &[&str]) -> String {
    states
        .iter()
        .map(|state| lumenna_core::state::State::from_keyword(state).map_or((*state).to_owned(), |s| s.abbreviation().to_owned()))
        .collect::<Vec<_>>()
        .join(" ")
}


/// [`trashed`], as its title and the rest.
pub fn trashed_line(row: &RowView, clock: &dyn Clock) -> Line {
    let states: Vec<&str> = row.trailing_states().filter(|state| *state != "deleted").collect();
    stated(row, clock, &states)
}

/// When a row is due, with the time in this device's clock: "due tomorrow at 3:00 PM".
fn due(row: &RowView, clock: &dyn Clock) -> Option<String> {
    due_words(row.due.as_deref(), row.due_time.as_deref(), clock)
}

/// When something is due, as every line says it: the core's words, then its time in this
/// device's clock ("due today at 3:00 PM").
pub fn due_words(due: Option<&str>, time: Option<&str>, clock: &dyn Clock) -> Option<String> {
    let due = due?.to_owned();
    Some(match time {
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



/// [`block`], as its times and title, then its details.
pub fn block_line(block: &PlanBlock, clock: &dyn Clock) -> Line {
    let first = format!("{} to {}, {}", clock.time(&block.start), clock.time(&block.end), block.title);
    let braille = join(vec![first.clone(), join(lumenna_records::words::block_braille(block))]);
    Line { braille, ..Line::of(first, block.details.clone()) }
}

/// A sitting: a task in a block for one session — its title, then the details the
/// core words for every app, a capped timer among them, never presented as fact.
pub fn sitting(sitting: &PlanAssignment) -> String {
    sitting_line(sitting).text()
}

/// [`sitting`], as its title and its details.
pub fn sitting_line(sitting: &PlanAssignment) -> Line {
    let braille = join(vec![sitting.title.clone(), join(lumenna_records::words::sitting_braille(sitting))]);
    Line { braille, ..Line::of(sitting.title.clone(), sitting.details.clone()) }
}



/// [`free_time`], as its title, then its details and span.
pub fn free_line(title: &str, details: &[String], start: &str, end: &str, clock: &dyn Clock) -> Line {
    let mut parts = details.to_vec();
    parts.push(format!("{} to {}", clock.time(start), clock.time(end)));
    Line::of(title.to_owned(), parts)
}

/// [`free_line`] with its length as a braille display shows it, `minutes` as "1:00".
pub fn free_line_brailled(free: &crate::day::FreeTime, clock: &dyn Clock) -> Line {
    let span = format!("{} to {}", clock.time(&free.start), clock.time(&free.end));
    let braille = join(vec![free.title.clone(), lumenna_records::words::duration_braille(free.minutes), span]);
    Line { braille, ..free_line(&free.title, &free.details, &free.start, &free.end, clock) }
}



/// [`cancelled`], as its time and title, then its details.
pub fn cancelled_line(block: &CancelledBlock, clock: &dyn Clock) -> Line {
    let first = format!("{}, {}", clock.time(&block.start), block.title);
    let braille = join(vec![first.clone(), join(lumenna_records::words::cancelled_braille())]);
    Line { braille, ..Line::of(first, block.details.clone()) }
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

/// A result's announcement, then each notice, as the core joins them (`words::spoken`).
pub fn announcement(announcement: &str, notices: &[String]) -> String {
    lumenna_records::words::spoken(announcement, notices)
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
            priority: None,
            swatch: None,
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
        PlanAssignment { details: lumenna_records::words::sitting_details(&sitting), ..sitting }
    }

    #[test]
    fn a_rows_braille_shows_its_states_short_and_says_them_whole() {
        let row = RowView { due: Some("due yesterday".to_owned()), ..task("Pay rent", Some("priority 1"), &["overdue", "recurring", "ready"], false) };
        let line = row_line(&row, true, &TwelveHour);
        assert_eq!(line.text, "Pay rent, due yesterday, priority 1, overdue, recurring");
        assert_eq!(line.braille, "Pay rent, due yesterday, priority 1, ovd rec");
        let plain = row_line(&task("Water the plants", None, &["ready"], false), true, &TwelveHour);
        assert_eq!(plain.braille, plain.text, "nothing to shorten");
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
        assert_eq!(trashed_line(&row, &TwelveHour).text, "Buy milk, due Monday, completed");
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
            swatch: None,
            timezone: None,
            notes: String::new(),
            details: Vec::new(),
            actions: Vec::new(),
        };
        let block = PlanBlock { details: lumenna_records::words::block_details(&block), ..block };
        assert_eq!(
            block_line(&block, &TwelveHour).text,
            "9:00 AM to 11:00 AM, Deep work, 2 hours, work block, now, 1 task assigned"
        );
        let line = block_line(&block, &TwelveHour);
        assert_eq!(line.title, "9:00 AM to 11:00 AM, Deep work");
        assert_eq!(line.detail, "2 hours, work block, now, 1 task assigned");
        assert_eq!(line.text, block_line(&block, &TwelveHour).text);
        // Braille: the length in hours and minutes, the kind short, the tasks counted short.
        assert_eq!(line.braille, "9:00 AM to 11:00 AM, Deep work, 2:00, wrk, now, 1 tsk");
        let sitting = sitting_line(&sitting_with("done", Some(75), 15));
        assert_eq!(sitting.braille, "Write report, done, plan 1:15, 0:15 logged");
    }

    #[test]
    fn a_length_in_braille_is_hours_and_minutes() {
        use lumenna_records::words::duration_braille;
        assert_eq!([duration_braille(75), duration_braille(60), duration_braille(15)], ["1:15", "1:00", "0:15"]);
    }

    /// Writes a date as its parts, so a test sees which parts the core asked for.
    struct Parts;

    impl Formats for Parts {
        fn time(&self, hour: i8, minute: i8) -> String {
            format!("{hour}h{minute:02}")
        }

        fn date(&self, year: i16, month: i8, day: i8, with_year: bool) -> String {
            if with_year { format!("{day}/{month}/{year}") } else { format!("{day}/{month}") }
        }
    }

    #[test]
    fn a_day_near_today_is_said_by_name_and_another_by_the_platform_with_its_year_only_when_another_year() {
        let today = jiff::Zoned::now().date();
        let clock = Spoken(Parts);
        assert_eq!(clock.day(&today.to_string()), "Today");
        assert_eq!(clock.day(&today.tomorrow().unwrap().to_string()), "Tomorrow");
        assert_eq!(clock.day(&today.yesterday().unwrap().to_string()), "Yesterday");
        let later = today.checked_add(jiff::Span::new().days(10)).unwrap();
        let expected = if later.year() == today.year() {
            format!("{}/{}", later.day(), later.month())
        } else {
            format!("{}/{}/{}", later.day(), later.month(), later.year())
        };
        assert_eq!(clock.day(&later.to_string()), expected);
        let next_year = jiff::civil::date(today.year() + 1, 3, 4);
        assert_eq!(clock.day(&next_year.to_string()), format!("4/3/{}", today.year() + 1));
        assert_eq!(clock.time("09:05"), "9h05");
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
            swatch: None,
            timezone: None,
            notes: String::new(),
            details: Vec::new(),
            actions: Vec::new(),
        };
        let block = PlanBlock { details: lumenna_records::words::block_details(&block), ..block };
        assert_eq!(block_line(&block, &TwelveHour).text, "12:30 PM to 1:15 PM, Lunch, 45 minutes, break block");
    }

    #[test]
    fn a_planned_length_never_reads_as_planned_twice() {
        assert_eq!(sitting(&sitting_with("planned", Some(45), 0)), "Write report, planned for 45 minutes");
        assert_eq!(
            sitting(&sitting_with("worked", Some(45), 50)),
            "Write report, worked, 45 minutes planned, 50 minutes logged"
        );
    }

    #[test]
    fn free_time_says_its_title_its_length_then_its_span() {
        assert_eq!(
            free_line("Free", &["45 minutes".to_owned()], "10:15", "11:00", &TwelveHour).text,
            "Free, 45 minutes, 10:15 AM to 11:00 AM"
        );
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
    fn an_announcement_is_followed_by_each_notice_as_the_core_joins_them() {
        assert_eq!(
            announcement("Added Buy milk.", &["New label shop.".to_owned()]),
            "Added Buy milk. New label shop."
        );
        assert_eq!(announcement("", &[]), "");
    }

    #[test]
    fn a_sentence_starts_with_a_capital() {
        assert_eq!(sentence("no task matches 'x'"), "No task matches 'x'");
        assert_eq!(sentence(""), "");
    }
}
