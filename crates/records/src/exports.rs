//! The exports a settings screen offers, each with what it is for and what its file is called.

use crate::types::ExportFormat;

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

/// The file name an export is offered under, dated: "Lumenna 2026-10-04.json".
pub fn export_name(format: ExportFormat, today: jiff::civil::Date) -> String {
    let extension = EXPORTS.iter().find(|e| e.format == format).map_or("txt", |e| e.extension);
    format!("Lumenna {today}.{extension}")
}
