//! How paired devices and exports are named and described.

use lumenna_surface::{DeviceView, ExportFormat};

/// A paired device, as its line in the list reads: "Kitchen Mac, macos, last synced 5
/// minutes ago". Sync status in words rather than an icon (§9).
pub fn line(device: &DeviceView, now: jiff::Timestamp) -> String {
    let mut parts = vec![device.name.clone(), device.platform.clone()];
    if device.this_device {
        parts.push("this device".to_owned());
    } else if let Some(error) = &device.last_error {
        parts.push(format!("last attempt failed: {error}"));
        if let Some(success) = &device.last_success {
            parts.push(format!("last synced {}", ago(success, now)));
        }
    } else if let Some(success) = &device.last_success {
        parts.push(format!("last synced {}", ago(success, now)));
    } else {
        parts.push("not synced yet".to_owned());
    }
    parts.join(", ")
}

/// How long ago a timestamp was, as a person says it.
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

/// The exports Settings offers, with what each is for and the extension its file takes.
pub const EXPORTS: [(ExportFormat, &str, &str); 4] = [
    (ExportFormat::Json, "Export &JSON, Complete, Can Be Imported...", "json"),
    (ExportFormat::Markdown, "Export a &Markdown Checklist...", "md"),
    (ExportFormat::Org, "Export an &Org Outline...", "org"),
    (ExportFormat::Ics, "Export a &Calendar File of Your Blocks...", "ics"),
];

/// The file name an export is offered under, dated: "Lumenna 2026-10-04.json".
pub fn export_name(format: ExportFormat, today: jiff::civil::Date) -> String {
    let extension = EXPORTS.iter().find(|(f, _, _)| *f == format).map_or("txt", |(_, _, e)| e);
    format!("Lumenna {today}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(this: bool, success: Option<&str>, error: Option<&str>) -> DeviceView {
        DeviceView {
            name: "Kitchen Mac".to_owned(),
            platform: "macos".to_owned(),
            node_id: "ab12".to_owned(),
            this_device: this,
            paired_at: "2026-10-01T09:00:00Z".to_owned(),
            last_attempt: None,
            last_success: success.map(str::to_owned),
            last_error: error.map(str::to_owned),
        }
    }

    fn now() -> jiff::Timestamp {
        "2026-10-04T12:00:00Z".parse().unwrap()
    }

    #[test]
    fn a_device_says_when_it_last_synced() {
        let synced = device(false, Some("2026-10-04T11:55:00Z"), None);
        assert_eq!(line(&synced, now()), "Kitchen Mac, macos, last synced 5 minutes ago");
        assert_eq!(line(&device(false, None, None), now()), "Kitchen Mac, macos, not synced yet");
        assert_eq!(line(&device(true, None, None), now()), "Kitchen Mac, macos, this device");
    }

    #[test]
    fn a_failure_is_said_before_the_last_success() {
        let failed = device(false, Some("2026-10-03T12:00:00Z"), Some("timed out"));
        assert_eq!(line(&failed, now()), "Kitchen Mac, macos, last attempt failed: timed out, last synced 1 day ago");
    }

    #[test]
    fn time_ago_is_said_in_the_largest_whole_unit() {
        assert_eq!(ago("2026-10-04T11:59:30Z", now()), "just now");
        assert_eq!(ago("2026-10-04T11:59:00Z", now()), "1 minute ago");
        assert_eq!(ago("2026-10-04T09:00:00Z", now()), "3 hours ago");
        assert_eq!(ago("not a time", now()), "not a time");
    }

    #[test]
    fn an_export_is_named_by_its_day_and_format() {
        let today = "2026-10-04".parse().unwrap();
        assert_eq!(export_name(ExportFormat::Markdown, today), "Lumenna 2026-10-04.md");
        assert_eq!(export_name(ExportFormat::Ics, today), "Lumenna 2026-10-04.ics");
    }
}
