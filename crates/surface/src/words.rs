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

/// How long ago a timestamp was, as a person says it: "just now", "5 minutes ago",
/// "2 hours ago", "3 days ago". An unreadable timestamp is returned as it came.
#[must_use]
pub fn ago(timestamp: &str, now: jiff::Timestamp) -> String {
    let Ok(then) = timestamp.parse::<jiff::Timestamp>() else { return timestamp.to_owned() };
    let seconds = now.as_second() - then.as_second();
    let plural = |n: i64, unit: &str| if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") };
    match seconds {
        ..60 => "just now".to_owned(),
        60..3_600 => plural(seconds / 60, "minute"),
        3_600..86_400 => plural(seconds / 3_600, "hour"),
        _ => plural(seconds / 86_400, "day"),
    }
}

/// What a line about a block says after its time and title (§13): its length, its kind,
/// where it falls today, whether this day was changed, what about it differs from its kind,
/// and — for a block that takes tasks — how many are in it.
///
/// The kind is said because it says which actions exist; a flag is said only where it
/// differs from the kind's own, so a work block is not "work block, takes tasks, movable".
#[must_use]
pub fn block_details(block: &crate::types::PlanBlock) -> Vec<String> {
    let mut parts = vec![duration(block.duration_mins), format!("{} block", block.kind)];
    if !block.when.is_empty() {
        parts.push(block.when.clone());
    }
    if block.changed_for_this_day {
        parts.push("changed for this day".to_owned());
    }
    let (takes, anchored) = match block.kind.as_str() {
        "work" => (true, false),
        "break" => (false, false),
        _ => (false, true),
    };
    if block.accepts_tasks != takes {
        parts.push(if block.accepts_tasks { "takes tasks" } else { "takes no tasks" }.to_owned());
    }
    if block.anchored != anchored {
        parts.push(if block.anchored { "anchored" } else { "movable" }.to_owned());
    }
    if block.accepts_tasks {
        parts.push(match block.assignments.len() {
            0 => "nothing assigned".to_owned(),
            1 => "1 task assigned".to_owned(),
            n => format!("{n} tasks assigned"),
        });
    }
    parts
}

/// What a line about a sitting says after its title (§13): its status with its planned
/// length, the time logged, and a timer that looks forgotten.
#[must_use]
pub fn sitting_details(sitting: &crate::types::PlanAssignment) -> Vec<String> {
    let mut parts = crate::form::sitting_status(sitting.clone());
    if sitting.minutes > 0 {
        parts.push(format!("{} logged", duration(sitting.minutes)));
    }
    if sitting.capped {
        parts.push("capped, the timer looks forgotten".to_owned());
    }
    parts
}

/// How syncing with a device is going, in words rather than an icon (§9): "this device";
/// "last synced 5 minutes ago"; or, when the last attempt failed, when and why, then when
/// it last worked.
#[must_use]
pub fn device_status(device: &crate::types::DeviceView, now: jiff::Timestamp) -> Vec<String> {
    if device.this_device {
        return vec!["this device".to_owned()];
    }
    let mut parts = Vec::new();
    if let Some(error) = &device.last_error {
        parts.push(match &device.last_attempt {
            Some(attempt) => format!("last attempt {} failed: {error}", ago(attempt, now)),
            None => format!("last attempt failed: {error}"),
        });
    }
    match &device.last_success {
        Some(success) => parts.push(format!("last synced {}", ago(success, now))),
        None if parts.is_empty() => parts.push("not synced yet".to_owned()),
        None => {}
    }
    parts
}
