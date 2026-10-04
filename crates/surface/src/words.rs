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

/// "45 minutes", "1 hour", "2 hours 30 minutes".
#[must_use]
pub fn duration(minutes: u32) -> String {
    let (hours, rest) = (minutes / 60, minutes % 60);
    match (hours, rest) {
        (0, m) => count_line(m as usize, "minute"),
        (h, 0) => count_line(h as usize, "hour"),
        (h, m) => format!("{} {}", count_line(h as usize, "hour"), count_line(m as usize, "minute")),
    }
}
