//! The shell's completion of names from the store: projects, labels and saved filters, for the
//! arguments that take one (`COMPLETE=bash lum`, or `lum completions bash`, registers it). A store
//! that cannot be opened offers nothing rather than failing the shell.

use std::ffi::OsStr;

use clap_complete::CompletionCandidate;

use crate::profile::Profile;

/// The names `list` gives that start with what has been typed, ignoring case.
fn matching(current: &OsStr, list: impl FnOnce(&Profile) -> Option<Vec<String>>) -> Vec<CompletionCandidate> {
    let typed = current.to_string_lossy().to_lowercase();
    let Some(names) = Profile::open(None).ok().and_then(|profile| list(&profile)) else { return Vec::new() };
    names.into_iter().filter(|name| name.to_lowercase().starts_with(&typed)).map(CompletionCandidate::new).collect()
}

/// Project names.
pub(crate) fn projects(current: &OsStr) -> Vec<CompletionCandidate> {
    matching(current, |profile| Some(profile.list_projects().ok()?.rows.into_iter().map(|row| row.title).collect()))
}

/// Label names.
pub(crate) fn labels(current: &OsStr) -> Vec<CompletionCandidate> {
    matching(current, |profile| Some(profile.list_labels().ok()?.rows.into_iter().map(|row| row.title).collect()))
}

/// Saved filters' names.
pub(crate) fn filters(current: &OsStr) -> Vec<CompletionCandidate> {
    matching(current, |profile| Some(profile.list_filters().ok()?.filters.into_iter().map(|filter| filter.name).collect()))
}
