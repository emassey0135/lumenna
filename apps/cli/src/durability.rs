//! `lum backup`, `restore`, `export` and `import`.
//!
//! The work is in the surface — every client backs up, not just this one — and what is here
//! is what a terminal adds: which file format a word on the command line means, `--force`, and
//! saying so when the automatic backup fails.

use std::path::Path;

use crate::api::Response;
use crate::error::{CliError, Result};
use crate::profile::Profile;

/// What `lum export` writes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ExportFormat {
    /// Everything current, structured — the one `lum import` reads back.
    #[default]
    Json,
    /// Tasks as a checklist, for reading.
    #[value(alias = "md")]
    Markdown,
    /// Tasks as an org outline, for Emacs.
    Org,
    /// Blocks as an iCalendar file, for any calendar.
    #[value(alias = "ical", alias = "icalendar")]
    Ics,
}

impl From<ExportFormat> for lumenna_host::ExportFormat {
    fn from(format: ExportFormat) -> Self {
        match format {
            ExportFormat::Json => Self::Json,
            ExportFormat::Markdown => Self::Markdown,
            ExportFormat::Org => Self::Org,
            ExportFormat::Ics => Self::Ics,
        }
    }
}

impl From<lumenna_host::ExportFormat> for ExportFormat {
    fn from(format: lumenna_host::ExportFormat) -> Self {
        match format {
            lumenna_host::ExportFormat::Json => Self::Json,
            lumenna_host::ExportFormat::Markdown => Self::Markdown,
            lumenna_host::ExportFormat::Org => Self::Org,
            lumenna_host::ExportFormat::Ics => Self::Ics,
        }
    }
}

/// `lum export`: the present state, to standard output or a file.
pub(crate) fn export(
    profile: &Profile,
    format: lumenna_host::ExportFormat,
    output: Option<&Path>,
    force: bool,
) -> Result<Response> {
    let path = output.map(|p| p.display().to_string());
    match profile.export(format, path, force) {
        Ok(exported) => Ok(Response::new(exported)),
        // The surface refuses; the terminal says how it is asked.
        Err(refused) if refused.message().ends_with(lumenna_host::NOT_ASKED_TO_REPLACE) => {
            Err(CliError::Message(format!("{}; pass --force to replace it", refused.message())))
        }
        Err(refused) => Err(refused.into()),
    }
}

/// Takes a backup at once if one is due, then looks again hourly, for as long as the process
/// runs: what a resident `lum` (an MCP server, `lum notify`) does, as `lum rpc` does.
pub(crate) fn back_up_hourly(lumenna: std::sync::Arc<lumenna_host::Lumenna>) {
    std::thread::spawn(move || {
        loop {
            if let Err(error) = lumenna.back_up_if_due() {
                anstream::eprintln!("lum: the automatic backup failed: {error}");
            }
            std::thread::sleep(std::time::Duration::from_secs(60 * 60));
        }
    });
}

/// Takes a backup if one is due, before a command runs.
///
/// Before the command rather than after, so that a command about to do damage is preceded by
/// the backup that undoes it. Quiet when it works — a daily backup announced on every first
/// command of the day is noise — and said once when it does not.
pub(crate) fn back_up_if_due(profile: &Profile) {
    if let Err(error) = profile.back_up_if_due() {
        anstream::eprintln!("lum: the automatic backup failed: {error}");
    }
}
