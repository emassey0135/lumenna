//! A time report as a listing (`lumenna_core::report`): what every sitting logged over the last
//! weeks, by week, by project and by label, each with the estimates of the tasks worked beside
//! it. Rows like any listing's, so it reads as every list does: a heading for each way it is
//! summed, the sums beneath, each "<what>, <logged>, <estimated>".

use jiff::Zoned;
use lumenna_core::report::{REPORT_WEEKS, Total, report as summed};
use lumenna_core::snapshot::Snapshot;

use crate::error::Result;
use crate::types::{RowView, Rows, Swatch};
use crate::words::{count_line, duration, to_u32};

/// What a sum's row is called: what `role` says of it.
pub const TOTAL: &str = "total";

/// What a total says after its name: the time logged, then the estimates of the tasks worked and
/// how many had none: "3 hours logged, 4 hours estimated, 1 task without an estimate".
#[must_use]
pub fn total_words(total: &Total) -> String {
    let mins = u32::try_from(total.logged_secs / 60).unwrap_or(u32::MAX);
    if total.logged_secs == 0 {
        return "nothing logged".to_owned();
    }
    let mut parts = vec![format!("{} logged", if mins == 0 { "under a minute".to_owned() } else { duration(mins) })];
    if total.estimate_mins > 0 {
        parts.push(format!("{} estimated", duration(total.estimate_mins)));
    }
    if total.unestimated > 0 {
        parts.push(format!("{} without an estimate", count_line(total.unestimated as usize, "task")));
    }
    parts.join(", ")
}

/// A week, as the report names it: "This week", "Last week", "3 weeks ago".
#[must_use]
pub fn week_name(back: usize) -> String {
    match back {
        0 => "This week".to_owned(),
        1 => "Last week".to_owned(),
        n => format!("{n} weeks ago"),
    }
}

/// The report of the last [`REPORT_WEEKS`] weeks at `now`, as a listing: By Week, By Project and,
/// when a task worked has one, By Label, each a heading over its sums. Nothing logged, it lists
/// nothing and says so.
///
/// # Errors
///
/// None yet; a listing's signature, so it is asked for as every place's is.
pub fn report(snapshot: &Snapshot, now: &Zoned) -> Result<Rows> {
    let summed = summed(snapshot, now.date(), REPORT_WEEKS, now.timestamp());
    let span = format!("in the last {}", count_line(REPORT_WEEKS as usize, "week"));
    let mut rows: Vec<RowView> = Vec::new();
    if summed.total.logged_secs > 0 {
        let mut group = |id: &str, title: &str, total: Option<&Total>, items: Vec<(String, String, String, Option<Swatch>)>| {
            rows.push(RowView {
                id: format!("report:{id}"),
                role: "group".to_owned(),
                expanded: Some(true),
                title: title.to_owned(),
                value: total.map(total_words),
                ..RowView::default()
            });
            let count = to_u32(items.len());
            for (index, (id, title, value, swatch)) in items.into_iter().enumerate() {
                rows.push(RowView {
                    id,
                    role: TOTAL.to_owned(),
                    depth: 1,
                    index: to_u32(index) + 1,
                    count,
                    title,
                    value: Some(value),
                    swatch,
                    ..RowView::default()
                });
            }
        };
        let weeks = summed
            .weeks
            .iter()
            .enumerate()
            .map(|(back, (start, total))| (format!("week:{start}"), week_name(back), total_words(total), None))
            .collect();
        group("weeks", "By Week", Some(&summed.total), weeks);
        let projects = summed
            .projects
            .iter()
            .map(|(id, total)| {
                let project = snapshot.projects.get(id);
                let name = project.map_or_else(|| "(not loaded)".to_owned(), |p| p.name.clone());
                (format!("project:{id}"), name, total_words(total), Swatch::of(project.and_then(|p| p.color.as_deref())))
            })
            .collect();
        group("projects", "By Project", None, projects);
        if !summed.labels.is_empty() {
            let labels = summed
                .labels
                .iter()
                .map(|(id, total)| {
                    let label = snapshot.labels.get(id);
                    let name = label.map_or_else(|| "(not loaded)".to_owned(), |l| l.name.clone());
                    (format!("label:{id}"), name, total_words(total), Swatch::of(label.and_then(|l| l.color.as_deref())))
                })
                .collect();
            group("labels", "By Label", None, labels);
        }
    }
    // The headings' places among themselves, as every listing numbers its siblings.
    let headings = to_u32(rows.iter().filter(|r| r.depth == 0).count());
    let mut heading = 0;
    for (at, row) in rows.iter_mut().enumerate() {
        row.row = to_u32(at + 1);
        if row.depth == 0 {
            heading += 1;
            row.index = heading;
            row.count = headings;
        }
    }
    let logged = u32::try_from(summed.total.logged_secs / 60).unwrap_or(u32::MAX);
    Ok(Rows {
        announcement: if summed.total.logged_secs == 0 {
            format!("Nothing logged {span}")
        } else {
            format!("{} logged {span}", crate::words::capitalised(&duration(logged)))
        },
        notices: Vec::new(),
        noun: TOTAL.to_owned(),
        count: to_u32(rows.len()),
        query: None,
        rows,
        empty: format!("Nothing logged {span}."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_total_says_what_was_logged_then_the_estimates_beside_it() {
        let total = Total { logged_secs: 200 * 60, estimate_mins: 240, tasks: 3, unestimated: 1, sittings: 4 };
        assert_eq!(total_words(&total), "3 hours 20 minutes logged, 4 hours estimated, 1 task without an estimate");
        assert_eq!(total_words(&Total::default()), "nothing logged");
        assert_eq!(total_words(&Total { logged_secs: 30, tasks: 1, unestimated: 1, sittings: 1, ..Total::default() }), "under a minute logged, 1 task without an estimate");
    }

    #[test]
    fn a_report_of_nothing_logged_lists_nothing_and_says_so() {
        let now: Zoned = "2026-11-27T09:00:00[Europe/London]".parse().unwrap();
        let rows = report(&Snapshot::default(), &now).unwrap();
        assert!(rows.rows.is_empty());
        assert_eq!((rows.announcement.as_str(), rows.empty.as_str()), ("Nothing logged in the last 4 weeks", "Nothing logged in the last 4 weeks."));
    }
}
