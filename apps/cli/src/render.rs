//! Turning a [`Response`] into output.
//!
//! Two renderings of one value: `--json` serialises the response as it stands, and text mode
//! writes the prose a terminal wants. Neither computes anything the other does not have —
//! everything either of them says comes off the typed surface, which is what keeps `--json` a
//! contract rather than a second implementation.

use anstream::{eprintln, print, println};
use serde::Serialize;

use lumenna_surface::{Plan, PlanItem, Rows, TaskDetail};

use crate::api::{Outcome, Response};

/// How output is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Lines, for a person at a terminal.
    Text,
    /// The typed surface, for a script.
    Json,
}

impl Format {
    /// Reads the `--json` flag.
    #[must_use]
    pub const fn from_flag(json: bool) -> Self {
        if json { Self::Json } else { Self::Text }
    }
}

/// Writes a response.
pub fn emit(response: &Response, format: Format) {
    match format {
        Format::Json => print_json(response),
        Format::Text => text(response),
    }
}

fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(error) => eprintln!("could not render JSON: {error}"),
    }
}

fn text(response: &Response) {
    if response.silent {
        return;
    }
    // Notices go to stderr so a pipeline keeps its payload, and first so that they read as
    // context for what follows rather than an afterthought.
    for notice in response.notices() {
        eprintln!("lum: {notice}");
    }
    match &response.outcome {
        // Completions, previews and server information are reachable over `lum rpc` only,
        // where nothing renders text. Announcing them is the honest fallback rather than a
        // branch that cannot be reached.
        Outcome::Change(_)
        | Outcome::Timer(_)
        | Outcome::Completions(_)
        | Outcome::Preview(_)
        | Outcome::WorkBlocks(_)
        | Outcome::Server(_)
        | Outcome::Backup(_)
        | Outcome::Restore(_)
        | Outcome::Import(_) => println!("{}", response.announcement()),
        Outcome::Paired(_) => println!("{}", response.announcement()),
        Outcome::Synced(report) => {
            println!("{}", response.announcement());
            for peer in &report.peers {
                match &peer.error {
                    None if peer.changed.is_empty() => {
                        println!("{}: synced, nothing new from it", peer.name);
                    }
                    None => println!("{}: brought in {}", peer.name, peer.changed.join(", ")),
                    Some(error) => println!("{}: not synced, {error}", peer.name),
                }
            }
        }
        Outcome::SyncStatus(status) => {
            println!("{}", response.announcement());
            for device in &status.devices {
                println!("{}", device_line(device));
            }
        }
        Outcome::Devices(list) => {
            println!("{}", response.announcement());
            for device in &list.devices {
                println!("{}", device_line(device));
            }
        }
        // An export to standard output is the payload itself, exactly, so it can be piped
        // or redirected into a file that is nothing but the export.
        Outcome::Export(exported) => match &exported.content {
            Some(content) => print!("{content}"),
            None => println!("{}", response.announcement()),
        },
        Outcome::Rows(rows) => list(rows, response.announcement()),
        Outcome::Task(shown) => detail(&shown.task),
        Outcome::Block(block) => {
            let mut fields = vec![
                ("title", block.title.clone()),
                ("id", block.id.clone()),
                ("starts", format!("{} on {}", block.start, block.start_date)),
                ("lasts", lumenna_surface::words::duration(block.minutes)),
                ("kind", block.kind.clone()),
            ];
            match (&block.repetition, &block.rrule) {
                (Some(phrase), _) => fields.push(("repeats", phrase.clone())),
                (None, Some(rule)) => fields.push(("repeats", format!("by the rule {rule}"))),
                (None, None) => {}
            }
            let yes = |value: bool| if value { "yes" } else { "no" }.to_owned();
            fields.push(("takes tasks", yes(block.accepts_tasks)));
            fields.push(("counts capacity", yes(block.counts_capacity)));
            fields.push(("anchored", yes(block.anchored)));
            if let Some(minutes) = block.min_minutes {
                fields.push(("shortest", lumenna_surface::words::duration(minutes)));
            }
            if let Some(until) = &block.until {
                fields.push(("until", until.clone()));
            }
            if let Some(filter) = &block.task_filter {
                fields.push(("tasks from", filter.clone()));
            }
            if let Some(colour) = &block.colour {
                fields.push(("colour", colour.clone()));
            }
            if !block.notes.is_empty() {
                fields.push(("notes", block.notes.clone()));
            }
            let width = fields.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
            for (key, value) in fields {
                println!("{key:>width$}: {value}", width = width);
            }
        }
        Outcome::Plan(plan) => day(plan),
        Outcome::Filters(filters) => {
            println!("{}", response.announcement());
            for filter in &filters.filters {
                println!("{}  {}  {}", filter.row, filter.name, filter.query);
            }
        }
        Outcome::Settings(settings) => match settings.settings.as_slice() {
            // One setting was asked for by name, so the name is not news.
            [only] => println!("{}", only.value),
            all => {
                let width = all.iter().map(|s| s.key.len()).max().unwrap_or(0);
                for setting in all {
                    println!("{:>width$}: {}", setting.key, setting.value, width = width);
                }
            }
        },
    }
}

fn list(rows: &Rows, announcement: &str) {
    if let Some(query) = &rows.query {
        // The readback: a mis-parsed filter shows wrong results *silently*, and wrong
        // results are invisible.
        println!("{}", query.description);
    }
    println!("{announcement}");
    let width = rows.rows.len().to_string().len();
    for row in &rows.rows {
        let indent = "  ".repeat(row.depth as usize);
        let mut line = format!("{:>width$}  {indent}{}", row.row, row.title, width = width);
        let mut trailing: Vec<String> = Vec::new();
        if row.checked == Some(true) {
            trailing.push("done".to_owned());
        }
        if let Some(value) = &row.value {
            trailing.push(value.clone());
        }
        trailing.extend(row.trailing_states().map(ToOwned::to_owned));
        if !trailing.is_empty() {
            line.push_str("  ");
            line.push_str(&trailing.join(", "));
        }
        println!("{line}");
    }
}

fn detail(task: &TaskDetail) {
    let mut fields: Vec<(&str, String)> = vec![("title", task.title.clone())];
    fields.push(("id", task.id.clone()));
    if let Some(project) = &task.project {
        fields.push(("project", project.clone()));
    }
    if task.has_priority() {
        fields.push(("priority", format!("p{}", task.priority)));
    }
    if let Some(due) = &task.due {
        let mut text = due.clone();
        if let Some(time) = &task.due_time {
            text.push_str(&format!(" at {time}"));
        }
        match (&task.repetition, &task.recurrence) {
            (Some(phrase), _) => text.push_str(&format!(", {phrase}")),
            (None, Some(rule)) => text.push_str(&format!(", repeats by the rule {rule}")),
            (None, None) => {}
        }
        fields.push(("due", text));
    }
    if let Some(minutes) = task.estimate_mins {
        fields.push(("estimate", format!("{minutes} minutes")));
    }
    if !task.labels.is_empty() {
        fields.push(("labels", task.labels.join(", ")));
    }
    if !task.depends.is_empty() {
        let titles: Vec<&str> = task.depends.iter().map(|d| d.title.as_str()).collect();
        fields.push(("depends on", titles.join(", ")));
    }
    if !task.state.is_empty() {
        fields.push(("state", task.state.join(", ")));
    }
    if !task.notes.is_empty() {
        fields.push(("notes", task.notes.clone()));
    }

    let width = fields.iter().map(|(key, _)| key.len()).max().unwrap_or(0);
    for (key, value) in fields {
        println!("{key:>width$}: {value}", width = width);
    }
}

fn day(plan: &Plan) {
    // The summary carries the count the announcement does, so it replaces it rather than
    // following it.
    if plan.summary.is_empty() {
        println!("{}", plan.announcement);
    } else {
        println!("{}. {}", plan.date, plan.summary);
    }
    // The core words the details for every app; the terminal adds the row numbers.
    let block_line = |block: &lumenna_surface::PlanBlock| {
        println!(
            "{}  {} to {}  {}  {}",
            block.row, block.start, block.end, block.title, block.details.join(", ")
        );
        for assignment in &block.assignments {
            println!("     {}  {}  {}", assignment.row, assignment.title, assignment.details.join(", "));
        }
    };
    // The timeline is the day as lived — free time and now as rows. An older reader's
    // plan has none, and gets the blocks alone.
    if plan.timeline.is_empty() {
        plan.blocks.iter().for_each(block_line);
    }
    for item in &plan.timeline {
        match item {
            PlanItem::Block { row } => {
                if let Some(block) = plan.blocks.get(*row as usize - 1) {
                    block_line(block);
                }
            }
            PlanItem::Free { start, end, minutes } => {
                println!("   free, {} from {start} to {end}", lumenna_surface::words::duration(*minutes));
            }
            PlanItem::Now { time } => println!("   now, {time}"),
        }
    }
    // What is not happening today but could be put back, with the command that does it.
    for block in &plan.cancelled {
        let short = &block.series[..block.series.len().min(8)];
        println!(
            "   cancelled for this day: {} at {}  (lum block restore {short} --date {})",
            block.title, block.start, plan.date
        );
    }
}

/// One device as a sentence: what it is, and how syncing with it last went. Words rather
/// than a symbol, because a glyph communicates nothing to a screen reader.
fn device_line(device: &lumenna_surface::DeviceView) -> String {
    let mut parts = vec![device.name.clone(), device.platform.clone()];
    parts.extend(device.status.iter().cloned());
    parts.join(", ")
}

