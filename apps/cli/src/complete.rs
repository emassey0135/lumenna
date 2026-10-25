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

/// The assistant's operations, for `lum assist`.
pub(crate) fn operations() -> Vec<CompletionCandidate> {
    lumenna_records::assistant::operations()
        .into_iter()
        .map(|op| CompletionCandidate::new(op.id).help(Some(op.sentence.into())))
        .collect()
}

/// What `lum complete` is completing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Typed {
    /// A quick-add line.
    Line,
    /// A filter, in the filter language.
    Filter,
    /// A project's name.
    Project,
    /// A label's name.
    Label,
}

/// What could come next at the end of `text`: a line's or a filter's from the parser's own
/// completion, which every app's quick-add field offers; a project's or a label's name from the
/// store, starting with what was typed, ignoring case, as the shell's completion offers it.
pub(crate) fn typed(profile: &Profile, kind: Typed, text: &str) -> lumenna_host::Result<lumenna_host::Completions> {
    let cursor = u32::try_from(text.len()).unwrap_or(u32::MAX);
    let (noun, rows) = match kind {
        Typed::Line => return profile.complete_text(text, cursor, lumenna_host::Syntax::QuickAdd),
        Typed::Filter => return profile.complete_text(text, cursor, lumenna_host::Syntax::Filter),
        Typed::Project => ("project", profile.list_projects()?.rows),
        Typed::Label => ("label", profile.list_labels()?.rows),
    };
    let typed = text.to_lowercase();
    let candidates: Vec<lumenna_host::Candidate> = rows
        .into_iter()
        .filter(|row| row.title.to_lowercase().starts_with(&typed))
        .map(|row| lumenna_host::Candidate { label: format!("{noun} {}", row.title), text: row.title, kind: noun.to_owned() })
        .collect();
    // Worded as the parser words its own.
    let announcement = match candidates.as_slice() {
        [only] => format!("1 completion, {}", only.label),
        all => lumenna_host::words::count_line(all.len(), "completion"),
    };
    Ok(lumenna_host::Completions {
        announcement,
        notices: Vec::new(),
        start: 0,
        end: cursor,
        candidates,
    })
}
