//! The shell's completion of names from the store: projects, labels and saved filters, for the
//! arguments that take one (`COMPLETE=bash lum`, or `lum completions bash`, registers it). A store
//! that cannot be opened offers nothing rather than failing the shell.

use std::ffi::OsStr;
use std::path::PathBuf;

use clap_complete::CompletionCandidate;

use crate::profile::Profile;

/// The names of `kind` that start with what has been typed, ignoring case, as `lum complete`
/// gives them, from the profile the command line being completed names.
fn matching(current: &OsStr, kind: lumenna_host::Completing) -> Vec<CompletionCandidate> {
    let typed = current.to_string_lossy();
    let Ok(profile) = Profile::open(profile_given().as_deref()) else { return Vec::new() };
    let Ok(found) = profile.complete_typed(kind, &typed) else { return Vec::new() };
    found.candidates.into_iter().map(|candidate| CompletionCandidate::new(candidate.text)).collect()
}

/// The `--profile` of the command line being completed, which the shell hands `lum` whole
/// after `--`, so a name is offered from the store the command will open.
fn profile_given() -> Option<PathBuf> {
    let mut words = std::env::args_os().skip_while(|word| word != "--");
    while let Some(word) = words.next() {
        if word == "--profile" {
            return words.next().map(PathBuf::from);
        }
        if let Some(value) = word.to_str().and_then(|word| word.strip_prefix("--profile=")) {
            return Some(PathBuf::from(value));
        }
    }
    None
}

/// Project names.
pub(crate) fn projects(current: &OsStr) -> Vec<CompletionCandidate> {
    matching(current, lumenna_host::Completing::Project)
}

/// Label names.
pub(crate) fn labels(current: &OsStr) -> Vec<CompletionCandidate> {
    matching(current, lumenna_host::Completing::Label)
}

/// Saved filters' names.
pub(crate) fn filters(current: &OsStr) -> Vec<CompletionCandidate> {
    matching(current, lumenna_host::Completing::SavedFilter)
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
    /// A saved filter's name.
    SavedFilter,
}

impl From<Typed> for lumenna_host::Completing {
    fn from(kind: Typed) -> Self {
        match kind {
            Typed::Line => Self::Line,
            Typed::Filter => Self::Filter,
            Typed::Project => Self::Project,
            Typed::Label => Self::Label,
            Typed::SavedFilter => Self::SavedFilter,
        }
    }
}
