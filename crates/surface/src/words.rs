//! Small pieces of wording every client composes the same way.

/// "no tasks", "1 task", "17 tasks".
#[must_use]
pub fn count_line(count: usize, noun: &str) -> String {
    match count {
        0 => format!("no {noun}s"),
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

/// A time of day as `HH:MM`, without the seconds nobody schedules by.
#[must_use]
pub fn time_text(time: jiff::civil::Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}
