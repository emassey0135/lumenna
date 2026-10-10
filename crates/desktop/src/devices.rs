//! How paired devices and exports are named and described.

use lumenna_surface::{DeviceView, ExportFormat};

/// A paired device, as its line in the list reads: "Kitchen Mac, macos, last synced 5
/// minutes ago" — its name and platform, then the status the core words for every app.
/// `now` is kept for callers; the status was worded when the devices were listed.
pub fn line(device: &DeviceView, _now: jiff::Timestamp) -> String {
    let mut parts = vec![device.name.clone(), device.platform.clone()];
    parts.extend(device.status.iter().cloned());
    parts.join(", ")
}

/// How long ago a timestamp was, as a person says it — the core's wording.
pub fn ago(timestamp: &str, now: jiff::Timestamp) -> String {
    lumenna_surface::words::ago(timestamp, now)
}

/// The exports Settings offers, with what each is for and the extension its file takes.
#[derive(Debug, Clone, Copy)]
pub struct Export {
    /// What it writes.
    pub format: ExportFormat,
    /// Its button's text, plain: each toolkit marks the mnemonic its own way (`marked`).
    pub label: &'static str,
    /// The letter that is its mnemonic.
    pub key: char,
    /// The extension its file takes.
    pub extension: &'static str,
}

/// The exports Settings offers, with what each is for.
pub const EXPORTS: [Export; 4] = [
    Export {
        format: ExportFormat::Json,
        label: "Export JSON, Complete, Can Be Imported...",
        key: 'J',
        extension: "json",
    },
    Export { format: ExportFormat::Markdown, label: "Export a Markdown Checklist...", key: 'M', extension: "md" },
    Export { format: ExportFormat::Org, label: "Export an Org Outline...", key: 'O', extension: "org" },
    Export { format: ExportFormat::Ics, label: "Export a Calendar File of Your Blocks...", key: 'C', extension: "ics" },
];

/// `text` with its mnemonic marked the way a toolkit marks one: `&` for Win32, `_` for GTK.
/// The marker goes before the first `key`; a marker already in the text is doubled, so it
/// shows as itself.
pub fn marked(text: &str, key: char, marker: char) -> String {
    let mut out = String::with_capacity(text.len() + 1);
    let mut placed = false;
    for c in text.chars() {
        if c == marker {
            out.push(marker);
        } else if c == key && !placed {
            out.push(marker);
            placed = true;
        }
        out.push(c);
    }
    out
}

/// The file name an export is offered under, dated: "Lumenna 2026-10-04.json".
pub fn export_name(format: ExportFormat, today: jiff::civil::Date) -> String {
    let extension = EXPORTS.iter().find(|e| e.format == format).map_or("txt", |e| e.extension);
    format!("Lumenna {today}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(this: bool, success: Option<&str>, error: Option<&str>) -> DeviceView {
        let mut view = DeviceView {
            name: "Kitchen Mac".to_owned(),
            platform: "macos".to_owned(),
            node_id: "ab12".to_owned(),
            this_device: this,
            paired_at: "2026-10-01T09:00:00Z".to_owned(),
            last_attempt: None,
            last_success: success.map(str::to_owned),
            last_error: error.map(str::to_owned),
            schema_version: 1,
            status: Vec::new(),
        };
        view.status = lumenna_surface::words::device_status(&view, now(), true);
        view
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

    #[test]
    fn each_toolkit_marks_the_mnemonic_its_own_way() {
        assert_eq!(marked("Export a Markdown Checklist...", 'M', '&'), "Export a &Markdown Checklist...");
        assert_eq!(marked("Export a Markdown Checklist...", 'M', '_'), "Export a _Markdown Checklist...");
        assert_eq!(marked("Save_as & Close", 'C', '&'), "Save_as && &Close", "a marker in the text shows as itself");
    }

    #[test]
    fn every_export_has_its_mnemonic_letter_in_its_label() {
        for export in EXPORTS {
            assert!(export.label.contains(export.key), "{}", export.label);
        }
    }
}
