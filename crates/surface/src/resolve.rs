//! Turning what a client sent into what core takes: identifiers, names, and phrases.

use jiff::{Zoned, civil};
use lumenna_core::filter::Expr;
use lumenna_core::id::{AssignmentId, SeriesId, TaskId};
use lumenna_core::model::{Due, Label, Project};
use lumenna_core::order::OrderKey;
use lumenna_core::snapshot::Snapshot;
use lumenna_parse::quickadd::{Known, parse_quick_add};
use lumenna_parse::{parse_filter, words};

use crate::error::{LumennaError, Result};

/// A whole identifier, or a prefix of one, resolved against `candidates`.
///
/// An ambiguous prefix is an error rather than a guess. Picking one would be a silent wrong
/// answer, and the whole point of short identifiers is that they are typed quickly and
/// therefore checked less.
///
/// A bare number is refused: it is a row number, which only the terminal has (§15), and
/// treating it as a prefix would quietly act on whatever happened to start with those digits.
/// A block occurrence's `<series>@<date>` resolves to its series.
fn resolve(input: &str, candidates: impl Iterator<Item = String>) -> Result<String> {
    let input = input.trim();
    if input.parse::<usize>().is_ok() {
        return Err(LumennaError::new(format!(
            "'{input}' is a row number, and row numbers address the terminal's last listing; \
             pass an identifier"
        )));
    }
    let bare = input.split('@').next().unwrap_or(input).to_lowercase();
    let matches: Vec<String> = candidates.filter(|id| id.starts_with(&bare)).collect();
    match matches.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(LumennaError::new(format!("nothing here matches '{input}'"))),
        many => Err(LumennaError::new(format!(
            "'{input}' matches {} items; type more of it",
            many.len()
        ))),
    }
}

pub(crate) fn task_id(snapshot: &Snapshot, input: &str) -> Result<TaskId> {
    resolve(input, snapshot.tasks.keys().map(ToString::to_string))?
        .parse()
        .map_err(|_| LumennaError::new(format!("'{input}' is not a task")))
}

pub(crate) fn series_id(snapshot: &Snapshot, input: &str) -> Result<SeriesId> {
    resolve(input, snapshot.series.keys().map(ToString::to_string))?
        .parse()
        .map_err(|_| LumennaError::new(format!("'{input}' is not a block")))
}

pub(crate) fn assignment_id(snapshot: &Snapshot, input: &str) -> Result<AssignmentId> {
    resolve(input, snapshot.assignments.keys().map(ToString::to_string))?
        .parse()
        .map_err(|_| LumennaError::new(format!("'{input}' is not an assignment")))
}

/// A " — did you mean …?" for a name that matched nothing, or nothing.
fn hint<'a>(name: &str, names: impl IntoIterator<Item = &'a str>) -> String {
    lumenna_core::suggest::nearest(name, names)
        .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"))
}

pub(crate) fn project<'a>(snapshot: &'a Snapshot, name: &str) -> Result<&'a Project> {
    snapshot.project_by_name(name).ok_or_else(|| {
        let names = snapshot.projects.values().map(|p| p.name.as_str());
        LumennaError::new(format!("no project called '{name}'{}", hint(name, names)))
    })
}

pub(crate) fn label<'a>(snapshot: &'a Snapshot, name: &str) -> Result<&'a Label> {
    let name = name.trim_start_matches('@');
    snapshot.label_by_name(name).ok_or_else(|| {
        let names = snapshot.labels.values().map(|l| l.name.as_str());
        LumennaError::new(format!("no label called '{name}'{}", hint(name, names)))
    })
}

/// A date phrase, resolved against now — `today`, `friday`, `2026-03-01` — or today when
/// there is none.
pub(crate) fn date(text: Option<&str>, now: &Zoned) -> Result<civil::Date> {
    let Some(text) = text.filter(|t| !t.trim().is_empty()) else {
        return Ok(now.date());
    };
    lumenna_parse::date::parse_date(&words(text), 0)
        .and_then(|(spec, _)| spec.resolve(now))
        .ok_or_else(|| LumennaError::new(format!("could not read a date from '{text}'")))
}

/// A due phrase, or `none` to clear it.
pub(crate) fn due(text: &str, now: &Zoned) -> Result<Option<Due>> {
    if text.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let when = lumenna_parse::date::parse_when(&words(text), 0)
        .ok_or_else(|| LumennaError::new(format!("could not read a date from '{text}'")))?;
    Ok(when.spec.resolve(now)?)
}

pub(crate) fn time(text: &str) -> Result<civil::Time> {
    lumenna_parse::date::parse_time(&words(text), 0)
        .map(|(time, _)| time)
        .ok_or_else(|| LumennaError::new(format!("could not read a time from '{text}'")))
}

pub(crate) fn query(snapshot: &Snapshot, text: &str) -> Result<Expr> {
    Ok(parse_filter(text, &Known::from_snapshot(snapshot))?)
}

/// `45m`, `2h`, `1h30m`, or a bare number of minutes.
pub(crate) fn minutes(text: &str) -> Result<u32> {
    if let Ok(plain) = text.trim().parse::<u32>() {
        return Ok(plain);
    }
    parse_quick_add(&format!("x {text}"), &Known::default())
        .estimate_mins
        .map(|spanned| spanned.value)
        .ok_or_else(|| LumennaError::new(format!("could not read a duration from '{text}'")))
}

pub(crate) fn yes_or_no(text: &str) -> Result<bool> {
    match text.to_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        other => Err(LumennaError::new(format!("'{other}' is not yes or no"))),
    }
}

/// A key that sorts after everything already there.
pub(crate) fn order_after(existing: impl Iterator<Item = OrderKey>) -> OrderKey {
    existing.max().map_or_else(OrderKey::middle, |last| OrderKey::after(&last))
}
