//! Turning a [`Response`] into output.
//!
//! Two renderings of one value: `--json` serialises the response as it stands, and text mode
//! writes the prose a terminal wants. Neither computes anything the other does not have —
//! everything either of them says comes off the typed surface, which
//! is what keeps `--json` a contract rather than a second implementation (§15).

use anstream::{eprintln, print, println};
use serde::Serialize;

use lumenna_surface::words::count_line;
use lumenna_surface::{Plan, Rows, TaskDetail};

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
        | Outcome::Server(_)
        | Outcome::Backup(_)
        | Outcome::Restore(_)
        | Outcome::Import(_) => println!("{}", response.announcement()),
        // An export to standard output is the payload itself, exactly, so it can be piped
        // or redirected into a file that is nothing but the export.
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
        Outcome::Export(exported) => match &exported.content {
            Some(content) => print!("{content}"),
            None => println!("{}", response.announcement()),
        },
        Outcome::Rows(rows) => list(rows, response.announcement()),
        Outcome::Task(shown) => detail(&shown.task),
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
        // results are invisible (§6.3).
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
        if let Some(recurrence) = &task.recurrence {
            text.push_str(&format!(" (repeats: {recurrence})"));
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
    println!("{}", plan.announcement);
    for block in &plan.blocks {
        println!("{}  {} to {}  {}", block.row, block.start, block.end, block.title);
        for assignment in &block.assignments {
            let mut detail = vec![assignment.status.to_owned()];
            if assignment.minutes > 0 {
                detail.push(format!("{} minutes logged", assignment.minutes));
            }
            if assignment.capped {
                detail.push("timer looks orphaned".to_owned());
            }
            println!("     {}  {}  {}", assignment.row, assignment.title, detail.join(", "));
        }
    }
}

/// One device as a sentence: what it is, and how syncing with it last went. Words rather
/// than a symbol, because §9 is explicit that a glyph communicates nothing.
fn device_line(device: &lumenna_surface::DeviceView) -> String {
    let mut line = format!("{}, {}", device.name, device.platform);
    if device.this_device {
        line.push_str(", this device");
        return line;
    }
    let ago = |t: &str| t.parse::<jiff::Timestamp>().map_or_else(|_| t.to_owned(), relative);
    match (&device.last_success, &device.last_error, &device.last_attempt) {
        (_, Some(error), Some(attempt)) => {
            line.push_str(&format!(", last attempt {} failed: {error}", ago(attempt)));
            if let Some(success) = &device.last_success {
                line.push_str(&format!("; last synced {}", ago(success)));
            }
        }
        (Some(success), _, _) => line.push_str(&format!(", last synced {}", ago(success))),
        _ => line.push_str(", not synced yet"),
    }
    line
}

/// "just now", "5 minutes ago", "3 hours ago", "2 days ago".
fn relative(then: jiff::Timestamp) -> String {
    let seconds = jiff::Timestamp::now().duration_since(then).as_secs().max(0);
    match seconds {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{} ago", count_line((seconds / 60) as usize, "minute")),
        3600..86_400 => format!("{} ago", count_line((seconds / 3600) as usize, "hour")),
        _ => format!("{} ago", count_line((seconds / 86_400) as usize, "day")),
    }
}
