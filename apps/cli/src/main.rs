//! `lum` — Lumenna's command line.
//!
//! §15 calls this the first target and a permanent one. It is usable in weeks rather than
//! months, it is fully accessible by construction, and it forces the core API to be complete
//! before any GUI can paper over gaps in it.
//!
//! # Completeness, for two reasons
//!
//! **It is a first-class interface.** Some people simply prefer text — `edbrowse` exists
//! because that preference is real and long-standing, not because those users' GUIs are
//! broken. Add scripting, and the CLI is a product in its own right.
//!
//! **And it is a test.** If something is only possible in a GUI, that is business logic that
//! leaked out of the core, violating principle 2. CLI coverage is the cheapest checkable
//! proxy for core coverage there is — far easier to audit than reading eleven UI
//! implementations looking for logic that should not be there.
//!
//! Treating it only as a test would harm it, though. A test optimises for coverage; a
//! product optimises for use. So: short identifiers, because a UUID is thirty-six characters
//! and worse to dictate than to type; `--json` as a versioned contract rather than a
//! debugging convenience; and `--help` treated as an **accessibility surface**, since it is
//! the primary discovery mechanism for anyone who cannot skim a GUI.

mod api;
mod durability;
mod error;
mod profile;
mod render;
mod rpc;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use jiff::Zoned;
use jiff::civil;
use lumenna_core::edit::{self, MoveTo, ProjectDeletion};
use lumenna_core::filter::{Context, Expr, Predicate};
use lumenna_core::id::{AssignmentId, LabelId, SeriesId, TaskId};
use lumenna_core::model::{
    BlockKind, BlockRef, BlockSeries, Due, Label, Priority, Project, SavedFilter, Task,
};
use lumenna_core::order::OrderKey;
use lumenna_core::row::{Role, Row, RowId};
use lumenna_core::snapshot::Snapshot;
use lumenna_parse::quickadd::{Known, Severity, parse_quick_add};
use lumenna_parse::{parse_filter, words};

use api::{Outcome, Response};
use error::{CliError, Result};
use profile::Profile;
use render::Format;

/// A task manager and day planner that keeps them in one place.
#[derive(Debug, Parser)]
#[command(name = "lum", version, about, long_about = None)]
struct Cli {
    /// Use a specific profile directory instead of the default.
    #[arg(long, global = true, value_name = "DIR")]
    profile: Option<PathBuf>,

    /// Print machine-readable JSON instead of lines.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Work with tasks.
    #[command(subcommand)]
    Task(TaskCommand),

    /// Work with projects.
    #[command(subcommand)]
    Project(ProjectCommand),

    /// Work with labels.
    #[command(subcommand)]
    Label(LabelCommand),

    /// Work with saved filters.
    #[command(subcommand)]
    Filter(FilterCommand),

    /// Show a day's blocks and what is assigned to them.
    Plan {
        /// A date phrase. Today if left out.
        date: Vec<String>,
    },

    /// Work with time blocks.
    #[command(subcommand)]
    Block(BlockCommand),

    /// Put a task into a block.
    Assign {
        /// A row number from the last listing, or a task identifier.
        task: String,
        /// The block series to put it in.
        #[arg(long)]
        block: String,
        /// Which day, for a repeating block. Today if left out.
        #[arg(long)]
        date: Option<String>,
        /// How long this sitting is meant to take.
        #[arg(long)]
        minutes: Option<u32>,
    },

    /// Take a task back out of a block.
    Unassign {
        /// The assignment identifier.
        assignment: String,
    },

    /// Start the timer on an assignment.
    Start {
        /// The assignment identifier.
        assignment: String,
    },

    /// Stop the timer and log the minutes.
    ///
    /// With --minutes, record that figure as the whole of the sitting instead — how time is
    /// logged without a timer, and how a capped figure from a forgotten one is put right.
    Stop {
        /// The assignment identifier.
        assignment: String,
        /// The sitting's total, replacing whatever was logged.
        #[arg(long)]
        minutes: Option<u32>,
    },

    /// Read or change settings.
    #[command(subcommand)]
    Config(ConfigCommand),

    /// Undo the last change made on this device.
    ///
    /// The history is this device's alone and survives between commands, so `lum undo`
    /// reverses what the last command did — or what the BTSpeak app did. Anything changed
    /// since by something else is kept, and you are told what.
    Undo,

    /// Redo the change most recently undone.
    Redo,

    /// Back up the whole store now, history included.
    ///
    /// A backup holds every task ever created — including the ones you deleted — so that a
    /// store can be rebuilt from it. One is also taken automatically when the last is a day
    /// old; `lum config set backup-every` changes that, and `backup-dir` where they go.
    Backup {
        /// Write it here instead of the configured directory.
        #[arg(long, value_name = "DIR")]
        to: Option<PathBuf>,
    },

    /// Merge a backup into the store.
    ///
    /// Nothing in the store is lost: what the backup has that the store lacks comes in, and
    /// the rest is left alone. A task deleted after the backup was taken stays deleted.
    Restore {
        /// The backup file.
        file: PathBuf,
    },

    /// Write out the current state: no history, nothing from the trash.
    ///
    /// JSON is complete and is what `lum import` reads back. Markdown and org are task lists
    /// for reading; ics is your blocks, for any calendar.
    Export {
        /// json, markdown (md), org, or ics.
        #[arg(long, value_enum, default_value_t)]
        format: durability::ExportFormat,
        /// Write to this file instead of standard output.
        #[arg(long, short, value_name = "FILE")]
        output: Option<PathBuf>,
        /// Replace the file if it exists.
        #[arg(long)]
        force: bool,
    },

    /// Read a JSON export, or a backup, into the store.
    ///
    /// Records keep their identifiers, so importing the same file twice changes nothing the
    /// second time. Nothing missing from the file is removed.
    Import {
        /// The file.
        file: PathBuf,
    },

    /// Serve the command surface over JSON-RPC on stdin and stdout.
    ///
    /// For clients that cannot link Rust — Emacs, the BTSpeak app. One server per client:
    /// it opens the store as any other process does, and pushes a notification when another
    /// process writes to it. No syncing; that is the daemon's job.
    Rpc,

    /// Print a shell completion script.
    Completions {
        /// Which shell.
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TaskCommand {
    /// Add a task, written the way you would say it.
    ///
    /// Everything is optional except the title: a date phrase, `p1` to `p4`, `#project`,
    /// `@label`, and an estimate like `45m`. What was understood is read back before it is
    /// saved.
    #[command(visible_alias = "a")]
    Add {
        /// The task, in quick-add form.
        text: Vec<String>,
        /// Save it without printing what was understood.
        #[arg(long)]
        quiet: bool,
    },

    /// List tasks, optionally filtered.
    #[command(visible_alias = "ls")]
    List {
        /// A filter query, such as `#work & (p1 | overdue)`.
        query: Vec<String>,
    },

    /// Show everything about one task.
    Show {
        /// A row number from the last listing, or an identifier.
        id: String,
    },

    /// Change a task.
    Edit {
        /// A row number from the last listing, or an identifier.
        id: String,
        /// A new title.
        #[arg(long)]
        title: Option<String>,
        /// A date phrase, or `none` to clear it.
        #[arg(long)]
        due: Option<String>,
        /// 1 to 4, where 1 is highest.
        #[arg(long)]
        priority: Option<u8>,
        /// How long it should take, such as `45m`, or `none` to clear it.
        #[arg(long)]
        estimate: Option<String>,
        /// Replacement notes.
        #[arg(long)]
        notes: Option<String>,
        /// Move it to a project by name.
        #[arg(long)]
        project: Option<String>,
    },

    /// Mark a task done.
    #[command(visible_alias = "d")]
    Done {
        /// A row number from the last listing, or an identifier.
        id: String,
    },

    /// Undo the most recent completion of a task.
    Undone {
        /// A row number from the last listing, or an identifier.
        id: String,
    },

    /// Move a task to the trash. Recoverable with `lum task restore`.
    Rm {
        /// A row number from the last listing, or an identifier.
        id: String,
    },

    /// Take a task back out of the trash.
    Restore {
        /// A row number from the last listing, or an identifier.
        id: String,
    },

    /// Delete a task permanently, with its history. This cannot be undone.
    Erase {
        /// A row number from the last listing, or an identifier.
        id: String,
        /// Skip the confirmation.
        #[arg(long)]
        yes: bool,
    },

    /// Move a task to another project, parent, or the top level.
    Move {
        /// A row number from the last listing, or an identifier.
        id: String,
        /// Put it under this task.
        #[arg(long)]
        parent: Option<String>,
        /// Put it in this project, by name.
        #[arg(long)]
        project: Option<String>,
        /// Take it out from under its parent.
        #[arg(long)]
        top: bool,
    },

    /// Search titles and notes.
    Search {
        /// The text to look for.
        text: Vec<String>,
    },

    /// Say that one task cannot start until another is done.
    #[command(subcommand)]
    Depend(DependCommand),
}

#[derive(Debug, Subcommand)]
pub(crate) enum DependCommand {
    /// Add a dependency.
    Add {
        /// The task that has to wait.
        id: String,
        /// The task it waits for.
        #[arg(long)]
        on: String,
    },
    /// Remove a dependency.
    Rm {
        /// The task that was waiting.
        id: String,
        /// The task it was waiting for.
        #[arg(long)]
        on: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    /// Add a project.
    Add {
        /// Its name.
        name: String,
        /// Put it under this project.
        #[arg(long)]
        parent: Option<String>,
    },
    /// List projects.
    List,
    /// Rename a project.
    Rename {
        /// The project to rename.
        name: String,
        /// Its new name.
        to: String,
    },
    /// Archive a project, keeping its tasks out of active views.
    Archive {
        /// The project to archive.
        name: String,
    },
    /// Delete a project.
    Rm {
        /// The project to delete.
        name: String,
        /// Keep its tasks, moved to the Inbox, instead of trashing them.
        #[arg(long)]
        keep_tasks: bool,
    },
    /// Set a project's urgency multiplier, roughly 0.5 to 2.0.
    ///
    /// This is not a second priority. Task priority is how much one item matters; weight is
    /// how much a whole area matters right now.
    Weight {
        /// The project.
        name: String,
        /// The multiplier, or `inherit` to take the parent's again.
        value: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum LabelCommand {
    /// Add a label.
    Add {
        /// Its name, without the leading `@`.
        name: String,
    },
    /// List labels.
    List,
    /// Rename a label. Every task wearing it follows.
    Rename {
        /// The label to rename.
        name: String,
        /// Its new name.
        to: String,
    },
    /// Fold one label into another, for when a typo made a near-duplicate.
    Merge {
        /// The label to retire.
        from: String,
        /// The label to keep.
        into: String,
    },
    /// Delete a label. Tasks wearing it simply stop showing it.
    Rm {
        /// The label to delete.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum FilterCommand {
    /// Save a filter query under a name.
    Add {
        /// What to call it.
        name: String,
        /// The query.
        query: Vec<String>,
    },
    /// List saved filters.
    List,
    /// Delete a saved filter.
    Rm {
        /// Its name.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum BlockCommand {
    /// Add a block.
    Add {
        /// What to call it.
        title: String,
        /// When it starts, such as `9am`.
        #[arg(long)]
        at: String,
        /// How many minutes it lasts.
        #[arg(long)]
        minutes: u32,
        /// Which day it starts. Today if left out.
        #[arg(long)]
        date: Option<String>,
        /// work, break, or event.
        #[arg(long, default_value = "work")]
        kind: String,
        /// A repetition, such as `every weekday`.
        #[arg(long)]
        repeat: Option<String>,
    },
    /// List block series.
    List,
    /// Delete a block series.
    Rm {
        /// Its identifier.
        id: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    /// Print a setting, or all of them.
    Get {
        /// Which setting.
        key: Option<String>,
    },
    /// Change a setting.
    Set {
        /// Which setting.
        key: String,
        /// Its new value.
        value: String,
    },
}
fn main() -> ExitCode {
    let cli = Cli::parse();
    let format = Format::from_flag(cli.json);
    match run(&cli, format) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            anstream::eprintln!("lum: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli, format: Format) -> Result<()> {
    if let Command::Completions { shell } = &cli.command {
        let mut command = <Cli as clap::CommandFactory>::command();
        clap_complete::generate(*shell, &mut command, "lum", &mut std::io::stdout());
        return Ok(());
    }

    let mut profile = Profile::open(cli.profile.as_deref())?;
    ensure_inbox(&mut profile)?;
    if matches!(cli.command, Command::Rpc) {
        return rpc::serve(profile);
    }

    if !matches!(cli.command, Command::Backup { .. }) {
        durability::back_up_if_due(&mut profile);
    }

    let now = Zoned::now();
    let response = dispatch(&mut profile, &cli.command, &now)?;
    // Row numbers count against whatever was listed last, so the listing is recorded from
    // the response rather than by each command that produces one — one place that can be
    // wrong instead of six.
    if let Some(listing) = listing_of(&response.outcome) {
        profile.remember(&listing);
    }
    render::emit(&response, format);
    Ok(())
}

/// Runs one command and returns what it produced.
///
/// The one place a command becomes a [`Response`], so `lum rpc` reaches every operation the
/// command line does by building the same [`Command`] (§12).
pub(crate) fn dispatch(
    profile: &mut Profile,
    command: &Command,
    now: &Zoned,
) -> Result<Response> {
    match command {
        Command::Completions { .. } | Command::Rpc => unreachable!("handled above"),
        Command::Task(command) => task(profile, command, now),
        Command::Project(command) => project(profile, command),
        Command::Label(command) => label(profile, command),
        Command::Filter(command) => filter(profile, command),
        Command::Plan { date } => plan(profile, &date.join(" "), now),
        Command::Block(command) => block(profile, command, now),
        Command::Assign { task, block, date, minutes } => {
            assign(profile, task, block, date.as_deref(), *minutes, now)
        }
        Command::Unassign { assignment } => unassign(profile, assignment),
        Command::Start { assignment } => start(profile, assignment, now),
        Command::Stop { assignment, minutes } => stop(profile, assignment, *minutes, now),
        Command::Config(command) => config(profile, command),
        Command::Undo => step(profile, false),
        Command::Redo => step(profile, true),
        Command::Backup { to } => durability::backup(profile, to.as_deref()),
        Command::Restore { file } => durability::restore(profile, file),
        Command::Export { format, output, force } => {
            durability::export(profile, *format, output.as_deref(), *force, now)
        }
        Command::Import { file } => durability::import(profile, file),
    }
}

/// Task commands, gathered under `lum task` the way blocks are under `lum block`.
fn task(profile: &mut Profile, command: &TaskCommand, now: &Zoned) -> Result<Response> {
    match command {
        TaskCommand::Add { text, quiet } => add(profile, &text.join(" "), *quiet, now),
        TaskCommand::List { query } => list(profile, &query.join(" "), now),
        TaskCommand::Show { id } => show(profile, id, now),
        TaskCommand::Edit { id, title, due, priority, estimate, notes, project } => edit_task(
            profile,
            id,
            title.as_deref(),
            due.as_deref(),
            *priority,
            estimate.as_deref(),
            notes.as_deref(),
            project.as_deref(),
            now,
        ),
        TaskCommand::Done { id } => done(profile, id, now),
        TaskCommand::Undone { id } => undone(profile, id),
        TaskCommand::Rm { id } => trash(profile, id),
        TaskCommand::Restore { id } => restore(profile, id),
        TaskCommand::Erase { id, yes } => erase(profile, id, *yes),
        TaskCommand::Move { id, parent, project, top } => {
            move_task(profile, id, parent.as_deref(), project.as_deref(), *top)
        }
        TaskCommand::Search { text } => search(profile, &text.join(" "), now),
        TaskCommand::Depend(command) => depend(profile, command),
    }
}

/// What a response puts in the last listing, if anything.
///
/// Only listings do: a mutation must not overwrite the numbering the user is working
/// against, or `lum task done 1` twice in a row would mean two different tasks.
fn listing_of(outcome: &Outcome) -> Option<Vec<(&'static str, String)>> {
    match outcome {
        Outcome::Rows(rows) => {
            Some(rows.rows.iter().map(|row| (row.role, row.id.clone())).collect())
        }
        // Blocks and assignments are numbered within their own kinds but listed together,
        // which is what lets `lum start 1` work straight after `lum plan` (§15).
        Outcome::Plan(plan) => Some(
            plan.blocks
                .iter()
                .flat_map(|block| {
                    std::iter::once(("block", block.id.clone())).chain(
                        block.assignments.iter().map(|a| ("assignment", a.id.clone())),
                    )
                })
                .collect(),
        ),
        Outcome::Change(_)
        | Outcome::Task(_)
        | Outcome::Filters(_)
        | Outcome::Settings(_)
        | Outcome::Timer(_)
        | Outcome::Completions(_)
        | Outcome::Preview(_)
        | Outcome::Server(_)
        | Outcome::Backup(_)
        | Outcome::Restore(_)
        | Outcome::Export(_)
        | Outcome::Import(_) => None,
    }
}

/// Every store has exactly one Inbox, and it is a real project rather than a null
/// `project_id` (§3.4).
///
/// The store creates it, under the same identifier on every device, so there is nothing to
/// create here. What is left is a store from before that, which minted an Inbox of its own:
/// that one is folded into the shared one, once.
fn ensure_inbox(profile: &mut Profile) -> Result<()> {
    let edit = edit::adopt_inbox(&state(profile));
    if !edit.is_empty() {
        profile.store.apply(&edit)?;
    }
    Ok(())
}

/// The repaired snapshot every query runs against.
pub(crate) fn state(profile: &Profile) -> Snapshot {
    let (mut snapshot, _) = profile.store.snapshot();
    // §3.13: merge can produce cycles and dangling references that no single device ever
    // wrote. Repairing on read means every query below can assume a tree.
    snapshot.repair();
    snapshot
}

fn ids_of<T: ToString>(items: impl Iterator<Item = T>) -> Vec<String> {
    items.map(|id| id.to_string()).collect()
}

// ---------------------------------------------------------------------------------------
// Shared resolution
// ---------------------------------------------------------------------------------------

fn resolve_task(profile: &Profile, snapshot: &Snapshot, input: &str) -> Result<TaskId> {
    let candidates = ids_of(snapshot.tasks.keys());
    let resolved = profile.resolve(input, "task", &candidates)?;
    resolved.parse().map_err(|_| CliError::Message(format!("'{input}' is not a task")))
}

fn find_project<'a>(snapshot: &'a Snapshot, name: &str) -> Result<&'a Project> {
    snapshot.project_by_name(name).ok_or_else(|| {
        let names: Vec<&str> = snapshot.projects.values().map(|p| p.name.as_str()).collect();
        let hint = lumenna_core::suggest::nearest(name, names)
            .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
        CliError::Message(format!("no project called '{name}'{hint}"))
    })
}

fn find_label<'a>(snapshot: &'a Snapshot, name: &str) -> Result<&'a Label> {
    let name = name.trim_start_matches('@');
    snapshot.label_by_name(name).ok_or_else(|| {
        let names: Vec<&str> = snapshot.labels.values().map(|l| l.name.as_str()).collect();
        let hint = lumenna_core::suggest::nearest(name, names)
            .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
        CliError::Message(format!("no label called '{name}'{hint}"))
    })
}

/// A date phrase, resolved against now — `today`, `friday`, `2026-03-01`.
fn parse_date_phrase(text: &str, now: &Zoned) -> Result<civil::Date> {
    let tokens = words(text);
    lumenna_parse::date::parse_date(&tokens, 0)
        .and_then(|(spec, _)| spec.resolve(now))
        .ok_or_else(|| CliError::Message(format!("could not read a date from '{text}'")))
}

/// A due phrase, or `none` to clear it.
fn parse_due_phrase(text: &str, now: &Zoned) -> Result<Option<Due>> {
    if text.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let tokens = words(text);
    let when = lumenna_parse::date::parse_when(&tokens, 0)
        .ok_or_else(|| CliError::Message(format!("could not read a date from '{text}'")))?;
    Ok(when.spec.resolve(now)?)
}

fn parse_time_phrase(text: &str) -> Result<civil::Time> {
    let tokens = words(text);
    lumenna_parse::date::parse_time(&tokens, 0)
        .map(|(time, _)| time)
        .ok_or_else(|| CliError::Message(format!("could not read a time from '{text}'")))
}

fn parse_query(snapshot: &Snapshot, text: &str) -> Result<Expr> {
    let known = Known::from_snapshot(snapshot);
    parse_filter(text, &known).map_err(CliError::Parse)
}

/// A key that sorts after everything already there.
fn order_after(existing: impl Iterator<Item = OrderKey>) -> OrderKey {
    existing.max().map_or_else(OrderKey::middle, |last| OrderKey::after(&last))
}

// ---------------------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------------------

fn add(profile: &mut Profile, text: &str, quiet: bool, now: &Zoned) -> Result<Response> {
    if text.trim().is_empty() {
        return Err(CliError::Message("nothing to add".to_owned()));
    }
    let snapshot = state(profile);
    let parsed = parse_quick_add(text, &Known::from_snapshot(&snapshot));
    let preview = parsed.resolve(&snapshot, now);

    // An unknown project is an error; an unknown label is a new label (§3.4). Refusing on
    // the first and proceeding on the second is the asymmetry that whole section argues for.
    if preview.has_errors() {
        let messages: Vec<&str> =
            preview.diagnostics.iter().map(|d| d.message.as_str()).collect();
        return Err(CliError::Message(messages.join("; ")));
    }

    let inbox = snapshot
        .inbox()
        .ok_or_else(|| CliError::Message("this store has no Inbox".to_owned()))?;
    let project_id = preview.project.unwrap_or(inbox.id);

    let mut changes = Vec::new();
    let mut labels: std::collections::BTreeSet<LabelId> =
        preview.labels.iter().copied().collect();
    let mut order = order_after(snapshot.labels.values().map(|l| l.order.clone()));
    for name in &preview.new_labels {
        let label = Label::new(name, order.clone());
        order = OrderKey::after(&order);
        labels.insert(label.id);
        changes.extend(edit::create_label(label).changes);
    }

    let mut task = Task::new(
        project_id,
        &preview.title,
        order_after(
            snapshot
                .tasks
                .values()
                .filter(|t| t.project_id == project_id && t.parent_id.is_none())
                .map(|t| t.order.clone()),
        ),
    );
    task.due = preview.due.clone();
    task.priority = preview.priority;
    task.estimate_mins = preview.estimate_mins;
    task.labels = labels;
    let task_id = task.id;
    changes.extend(edit::create_task(task).changes);

    let change = edit::Edit { description: format!("Added {}", preview.title), changes };
    profile.store.apply_recorded(&change)?;

    // The announcement stands in for the inline highlighting a sighted user gets as they
    // type (§6.1) — and always names the resolved date, since "Friday" is the ambiguous part
    // and confirming the phrase back would confirm nothing.
    let mut response = Response::changed_as(preview.announcement(), &change);

    // §12's surface is `add_task(text) -> Task`, so the task comes back whole. Read after
    // applying: labels created alongside it are only nameable once they exist.
    let after = state(profile);
    if let Some(created) = after.tasks.get(&task_id) {
        response = response.with_task(api::TaskDetail::of(created, &after, now));
    }
    for diagnostic in &preview.diagnostics {
        if diagnostic.severity == Severity::Notice {
            response = response.note(diagnostic.message.clone());
        }
    }
    Ok(if quiet { response.quietly() } else { response })
}

fn list(profile: &mut Profile, query: &str, now: &Zoned) -> Result<Response> {
    let snapshot = state(profile);
    let expr = parse_query(&snapshot, query)?;
    let cx = Context::new(&snapshot, now);

    let unresolved: Vec<api::Unresolved> = expr
        .unresolved(&snapshot)
        .into_iter()
        .map(|name| api::Unresolved {
            kind: api::name_kind(name.kind),
            name: name.name,
            suggestion: name.suggestion,
        })
        .collect();
    let notices: Vec<String> = unresolved
        .iter()
        .map(|name| {
            let hint = name
                .suggestion
                .as_deref()
                .map_or_else(String::new, |near| format!(" — did you mean '{near}'?"));
            format!("no {} called '{}'{hint}", name.kind, name.name)
        })
        .collect();

    let rows = snapshot.task_rows(&expr, &cx);
    let listing = api::Rows::new(&rows, "task");
    let mut response = Response::new(
        render::count_line(listing.count, "task"),
        Outcome::Rows(if query.trim().is_empty() {
            listing
        } else {
            listing.with_query(api::Query {
                text: query.to_owned(),
                description: expr.describe(),
                unresolved,
            })
        }),
    );
    for notice in notices {
        response = response.note(notice);
    }
    Ok(response)
}

fn search(profile: &mut Profile, text: &str, now: &Zoned) -> Result<Response> {
    if text.trim().is_empty() {
        return Err(CliError::Message("nothing to search for".to_owned()));
    }
    let snapshot = state(profile);
    let expr = Expr::Predicate(Predicate::Search(text.to_owned()));
    let cx = Context::new(&snapshot, now);
    let rows = api::Rows::new(&snapshot.task_rows(&expr, &cx), "task");
    Ok(Response::new(render::count_line(rows.count, "task"), Outcome::Rows(rows)))
}

fn show(profile: &Profile, input: &str, now: &Zoned) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let task = snapshot
        .tasks
        .get(&id)
        .ok_or(CliError::Edit(lumenna_core::edit::EditError::NotFound { kind: "task" }))?;
    let detail = api::TaskDetail::of(task, &snapshot, now);
    Ok(Response::new(detail.title.clone(), Outcome::Task(Box::new(detail))))
}

#[expect(clippy::too_many_arguments, reason = "one parameter per editable field")]
fn edit_task(
    profile: &mut Profile,
    input: &str,
    title: Option<&str>,
    due: Option<&str>,
    priority: Option<u8>,
    estimate: Option<&str>,
    notes: Option<&str>,
    project: Option<&str>,
    now: &Zoned,
) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let before = snapshot
        .tasks
        .get(&id)
        .ok_or(CliError::Edit(lumenna_core::edit::EditError::NotFound { kind: "task" }))?
        .clone();
    let mut after = before.clone();

    if let Some(title) = title {
        after.title = title.to_owned();
    }
    if let Some(due) = due {
        after.due = parse_due_phrase(due, now)?;
    }
    if let Some(priority) = priority {
        after.priority = Priority::from_u8(priority);
    }
    if let Some(estimate) = estimate {
        after.estimate_mins = if estimate.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(parse_minutes(estimate)?)
        };
    }
    if let Some(notes) = notes {
        after.notes = notes.to_owned();
    }
    // A new project goes through the move, so subtasks follow and a parent left behind is
    // let go of — the same rules as `lum task move --project`. The other fields are laid
    // over the task's half of that move.
    let moved = match project {
        Some(project) => {
            let project = find_project(&snapshot, project)?.id;
            Some(edit::move_task(&snapshot, id, MoveTo::Project(project))?)
        }
        None => None,
    };
    let change = match moved.filter(|m| !m.is_empty()) {
        Some(mut moved) => {
            for change in &mut moved.changes {
                if let edit::Change::Task(transition) = change
                    && let Some(task) = transition.after.as_mut()
                    && task.id == id
                {
                    let (project, parent) = (task.project_id, task.parent_id);
                    *task = after.clone();
                    task.project_id = project;
                    task.parent_id = parent;
                }
            }
            moved.description = format!("Edited {}", after.title);
            moved
        }
        None => edit::update_task(before, after),
    };
    if change.is_empty() {
        return Ok(Response::unchanged("nothing changed"));
    }
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

/// `45m`, `2h`, `1h30m`, or a bare number of minutes.
fn parse_minutes(text: &str) -> Result<u32> {
    if let Ok(plain) = text.parse::<u32>() {
        return Ok(plain);
    }
    let parsed = parse_quick_add(&format!("x {text}"), &Known::default());
    parsed
        .estimate_mins
        .map(|spanned| spanned.value)
        .ok_or_else(|| CliError::Message(format!("could not read a duration from '{text}'")))
}

fn done(profile: &mut Profile, input: &str, now: &Zoned) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let change = edit::complete_task(&snapshot, id, now)?;
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

fn undone(profile: &mut Profile, input: &str) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let change = edit::uncomplete_task(&snapshot, id)?;
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

fn trash(profile: &mut Profile, input: &str) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let change = edit::trash_task(&snapshot, id)?;
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed_as(
        format!("{} (recover it with `lum task restore`)", change.description),
        &change,
    ))
}

fn restore(profile: &mut Profile, input: &str) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let change = edit::restore_task(&snapshot, id)?;
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

fn erase(profile: &mut Profile, input: &str, yes: bool) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;
    let title = snapshot.tasks.get(&id).map_or("that task", |t| t.title.as_str()).to_owned();

    if !yes {
        anstream::eprint!("Permanently delete '{title}' and its history? Type yes to confirm: ");
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if answer.trim() != "yes" {
            return Ok(Response::unchanged("nothing was deleted"));
        }
    }
    let change = edit::purge_task(&snapshot, id)?;
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

fn move_task(
    profile: &mut Profile,
    input: &str,
    parent: Option<&str>,
    project: Option<&str>,
    top: bool,
) -> Result<Response> {
    let snapshot = state(profile);
    let id = resolve_task(profile, &snapshot, input)?;

    let destination = match (parent, project, top) {
        (Some(parent), _, _) => MoveTo::Parent(Some(resolve_task(profile, &snapshot, parent)?)),
        (_, Some(project), _) => MoveTo::Project(find_project(&snapshot, project)?.id),
        (_, _, true) => MoveTo::Parent(None),
        _ => {
            return Err(CliError::Message(
                "say where: --parent, --project, or --top".to_owned(),
            ));
        }
    };
    let change = edit::move_task(&snapshot, id, destination)?;
    if change.is_empty() {
        return Ok(Response::unchanged("it is already there"));
    }
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

fn depend(profile: &mut Profile, command: &DependCommand) -> Result<Response> {
    let snapshot = state(profile);
    let (input, on, adding) = match command {
        DependCommand::Add { id, on } => (id, on, true),
        DependCommand::Rm { id, on } => (id, on, false),
    };
    let id = resolve_task(profile, &snapshot, input)?;
    let dependency = resolve_task(profile, &snapshot, on)?;
    if adding && id == dependency {
        return Err(CliError::Message("a task cannot wait for itself".to_owned()));
    }
    // A local edge that closes a cycle of any length is a mistake this device can see, even
    // though merge can still produce one and `repair` exists for that (§3.13). Core refuses
    // it, so every client does.
    let change = if adding {
        edit::add_dependency(&snapshot, id, dependency)?
    } else {
        edit::remove_dependency(&snapshot, id, dependency)?
    };
    if change.is_empty() {
        return Ok(Response::unchanged("nothing changed"));
    }
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

// ---------------------------------------------------------------------------------------
// Projects, labels, filters
// ---------------------------------------------------------------------------------------

fn project(profile: &mut Profile, command: &ProjectCommand) -> Result<Response> {
    let snapshot = state(profile);
    match command {
        ProjectCommand::Add { name, parent } => {
            if snapshot.project_by_name(name).is_some() {
                return Err(CliError::Message(format!("there is already a project '{name}'")));
            }
            let mut project = Project::new(
                name,
                order_after(snapshot.projects.values().map(|p| p.order.clone())),
            );
            if let Some(parent) = parent {
                project.parent_id = Some(find_project(&snapshot, parent)?.id);
            }
            let change = edit::create_project(project);
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
        ProjectCommand::List => {
            let mut rows = Vec::new();
            let mut live: Vec<&Project> =
                snapshot.projects.values().filter(|p| p.deleted_at.is_none()).collect();
            live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            let count = u32::try_from(live.len()).unwrap_or(u32::MAX);
            for (index, project) in live.iter().enumerate() {
                let tasks = snapshot
                    .tasks
                    .values()
                    .filter(|t| t.project_id == project.id && !t.is_deleted())
                    .count();
                let mut value = render::count_line(tasks, "task");
                let weight = snapshot.effective_weight(project.id);
                if (weight - Project::NEUTRAL_WEIGHT).abs() > f32::EPSILON {
                    value.push_str(&format!(", weight {weight}"));
                }
                if project.archived {
                    value.push_str(", archived");
                }
                rows.push(Row {
                    id: RowId::Project(project.id),
                    role: Role::Project,
                    depth: depth_of(&snapshot, project),
                    index: u32::try_from(index).unwrap_or(u32::MAX) + 1,
                    count,
                    expanded: None,
                    checked: None,
                    title: project.name.clone(),
                    state: Vec::new(),
                    value: Some(value),
                    hint: None,
                });
            }
            let listing = api::Rows::new(&rows, "project");
            Ok(Response::new(
                render::count_line(listing.count, "project"),
                Outcome::Rows(listing),
            ))
        }
        ProjectCommand::Rename { name, to } => {
            let before = find_project(&snapshot, name)?.clone();
            if snapshot.project_by_name(to).is_some_and(|other| other.id != before.id) {
                return Err(CliError::Message(format!("there is already a project '{to}'")));
            }
            let mut after = before.clone();
            after.name = to.clone();
            let change = edit::update_project(before, after);
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed_as(format!("Renamed {name} to {to}"), &change))
        }
        ProjectCommand::Archive { name } => {
            let before = find_project(&snapshot, name)?.clone();
            let mut after = before.clone();
            after.archived = !before.archived;
            let archived = after.archived;
            let change = edit::update_project(before, after);
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed_as(
                if archived { format!("Archived {name}") } else { format!("Unarchived {name}") },
                &change,
            ))
        }
        ProjectCommand::Rm { name, keep_tasks } => {
            let target = find_project(&snapshot, name)?;
            if target.is_inbox {
                return Err(CliError::Message("the Inbox cannot be deleted".to_owned()));
            }
            let disposition = if *keep_tasks {
                ProjectDeletion::MoveTasksToInbox
            } else {
                ProjectDeletion::TrashTasks
            };
            let change = edit::trash_project(&snapshot, target.id, disposition)?;
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
        ProjectCommand::Weight { name, value } => {
            let before = find_project(&snapshot, name)?.clone();
            let mut after = before.clone();
            if value.eq_ignore_ascii_case("inherit") {
                let project_id = before.id;
                after.weight = None;
                let change = edit::update_project(before, after);
                if change.is_empty() {
                    return Ok(Response::unchanged("it already inherits its weight"));
                }
                profile.store.apply_recorded(&change)?;
                let inherited = state(profile).effective_weight(project_id);
                return Ok(Response::changed_as(
                    format!("{name} now inherits its weight, which is {inherited}"),
                    &change,
                ));
            }
            let value: f32 = value.parse().ok().filter(|v: &f32| v.is_finite() && *v > 0.0).ok_or_else(
                || CliError::Message("a weight has to be a positive number, or `inherit`".to_owned()),
            )?;
            after.weight = Some(value);
            let change = edit::update_project(before, after);
            profile.store.apply_recorded(&change)?;

            let response = Response::changed_as(format!("{name} now weighs {value}"), &change);
            let (low, high) = Project::WEIGHT_RANGE;
            Ok(if value < low || value > high {
                response.note(format!(
                    "{value} is outside the usual range of {low} to {high}; a wider range \
                     lets one project dominate every ranking"
                ))
            } else {
                response
            })
        }
    }
}

fn depth_of(snapshot: &Snapshot, project: &Project) -> u32 {
    let mut depth = 0;
    let mut current = project.parent_id;
    let mut seen = std::collections::BTreeSet::from([project.id]);
    while let Some(id) = current {
        if !seen.insert(id) {
            break;
        }
        depth += 1;
        current = snapshot.projects.get(&id).and_then(|p| p.parent_id);
    }
    depth
}

fn label(profile: &mut Profile, command: &LabelCommand) -> Result<Response> {
    let snapshot = state(profile);
    match command {
        LabelCommand::Add { name } => {
            let name = name.trim_start_matches('@');
            if snapshot.label_by_name(name).is_some() {
                return Err(CliError::Message(format!("there is already a label '{name}'")));
            }
            let change = edit::create_label(Label::new(
                name,
                order_after(snapshot.labels.values().map(|l| l.order.clone())),
            ));
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
        LabelCommand::List => {
            let mut live: Vec<&Label> =
                snapshot.labels.values().filter(|l| l.deleted_at.is_none()).collect();
            live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            let count = u32::try_from(live.len()).unwrap_or(u32::MAX);
            let rows: Vec<Row> = live
                .iter()
                .enumerate()
                .map(|(index, label)| {
                    let used = snapshot
                        .tasks
                        .values()
                        .filter(|t| t.labels.contains(&label.id) && !t.is_deleted())
                        .count();
                    Row {
                        id: RowId::Label(label.id),
                        role: Role::Label,
                        depth: 0,
                        index: u32::try_from(index).unwrap_or(u32::MAX) + 1,
                        count,
                        expanded: None,
                        checked: None,
                        title: label.name.clone(),
                        state: Vec::new(),
                        value: Some(render::count_line(used, "task")),
                        hint: None,
                    }
                })
                .collect();
            let listing = api::Rows::new(&rows, "label");
            Ok(Response::new(
                render::count_line(listing.count, "label"),
                Outcome::Rows(listing),
            ))
        }
        LabelCommand::Rename { name, to } => {
            let before = find_label(&snapshot, name)?.clone();
            let to_name = to.trim_start_matches('@');
            if snapshot.label_by_name(to_name).is_some_and(|other| other.id != before.id) {
                return Err(CliError::Message(format!(
                    "there is already a label '{to_name}'; `lum label merge` folds one into the other"
                )));
            }
            let mut after = before.clone();
            after.name = to.trim_start_matches('@').to_owned();
            let change = edit::update_label(before, after);
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
        LabelCommand::Merge { from, into } => {
            let loser = find_label(&snapshot, from)?.id;
            let winner = find_label(&snapshot, into)?.id;
            let change = edit::merge_labels(&snapshot, loser, winner)?;
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
        LabelCommand::Rm { name } => {
            let target = find_label(&snapshot, name)?.id;
            let change = edit::trash_label(&snapshot, target)?;
            profile.store.apply_recorded(&change)?;
            // Worth stating, because it surprises people: the tasks are untouched.
            Ok(Response::changed_as(
                format!("{}; tasks that wore it are unchanged", change.description),
                &change,
            ))
        }
    }
}

fn filter(profile: &mut Profile, command: &FilterCommand) -> Result<Response> {
    let snapshot = state(profile);
    match command {
        FilterCommand::Add { name, query } => {
            let text = query.join(" ");
            // Checked now so a broken query is caught here rather than the first time the
            // filter is used, but stored as text: a saved filter containing `today` has to
            // mean today at evaluation time (§6.2).
            let expr = parse_query(&snapshot, &text)?;
            let change = edit::create_filter(SavedFilter {
                id: lumenna_core::id::FilterId::new(),
                name: name.clone(),
                query: text,
                order: order_after(snapshot.saved_filters.values().map(|f| f.order.clone())),
                color: None,
                deleted_at: None,
            });
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed_as(format!("Saved {name}: {}", expr.describe()), &change))
        }
        FilterCommand::List => {
            let mut live: Vec<&SavedFilter> =
                snapshot.saved_filters.values().filter(|f| f.deleted_at.is_none()).collect();
            live.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));
            let filters = api::Filters {
                count: live.len(),
                filters: live
                    .iter()
                    .enumerate()
                    .map(|(index, saved)| api::FilterView {
                        row: index + 1,
                        id: saved.id.to_string(),
                        name: saved.name.clone(),
                        query: saved.query.clone(),
                    })
                    .collect(),
            };
            Ok(Response::new(
                render::count_line(filters.count, "filter"),
                Outcome::Filters(filters),
            ))
        }
        FilterCommand::Rm { name } => {
            let target = snapshot
                .saved_filters
                .values()
                .find(|f| f.deleted_at.is_none() && f.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| CliError::Message(format!("no filter called '{name}'")))?;
            let change = edit::trash_filter(&snapshot, target.id)?;
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
    }
}

// ---------------------------------------------------------------------------------------
// Planning: blocks, assignments, timers
// ---------------------------------------------------------------------------------------

fn plan(profile: &mut Profile, date: &str, now: &Zoned) -> Result<Response> {
    let day = if date.trim().is_empty() {
        now.date()
    } else {
        parse_date_phrase(date, now)?
    };
    // Year documents load only when a year is viewed, which is what keeps the watch viable
    // (§8). This is that moment.
    profile.store.load_year(day.year())?;

    let snapshot = state(profile);
    let occurrences = snapshot.day(day)?;

    // Both kinds are numbered within their own kind, so `lum start 1` works straight after
    // `lum plan` without a second command in between.
    let mut assignment_row = 0;
    let mut blocks = Vec::new();
    for (index, occurrence) in occurrences.iter().enumerate() {
        let series = snapshot.series.get(&occurrence.series_id);
        let block_ref = series.map(|s| occurrence.block_ref(s));
        let mut assigned: Vec<&lumenna_core::model::BlockAssignment> = snapshot
            .assignments
            .values()
            .filter(|a| Some(a.block_ref) == block_ref)
            .collect();
        assigned.sort_by(|a, b| a.order.cmp_with(&a.id, &b.order, &b.id));

        let assignments = assigned
            .iter()
            .map(|assignment| {
                let elapsed = assignment.elapsed(now.timestamp(), Some(occurrence.duration_mins));
                assignment_row += 1;
                api::PlanAssignment {
                    row: assignment_row,
                    id: assignment.id.to_string(),
                    task: assignment.task_id.to_string(),
                    title: snapshot
                        .tasks
                        .get(&assignment.task_id)
                        .map_or_else(|| "(not loaded)".to_owned(), |t| t.title.clone()),
                    status: assignment.status.speech(),
                    planned_mins: assignment.planned_mins,
                    minutes: elapsed.mins,
                    capped: elapsed.capped,
                }
            })
            .collect();

        blocks.push(api::PlanBlock {
            row: index + 1,
            id: format!("{}@{}", occurrence.series_id, occurrence.date),
            series: occurrence.series_id.to_string(),
            title: occurrence.title.clone(),
            start: render::time_text(occurrence.start_time),
            end: render::time_text(occurrence.end_time()),
            duration_mins: occurrence.duration_mins,
            assignments,
        });
    }

    let plan = api::Plan { date: day.to_string(), count: blocks.len(), blocks };
    Ok(Response::new(
        format!("{day}, {}", render::count_line(plan.count, "block")),
        Outcome::Plan(plan),
    ))
}

fn block(profile: &mut Profile, command: &BlockCommand, now: &Zoned) -> Result<Response> {
    match command {
        BlockCommand::Add { title, at, minutes, date, kind, repeat } => {
            let day = match date {
                Some(text) => parse_date_phrase(text, now)?,
                None => now.date(),
            };
            let start = parse_time_phrase(at)?;
            let kind = match kind.to_lowercase().as_str() {
                "work" => BlockKind::Work,
                "break" => BlockKind::Break,
                "event" => BlockKind::Event,
                other => {
                    return Err(CliError::Message(format!(
                        "'{other}' is not a block kind; use work, break, or event"
                    )));
                }
            };
            let mut series = BlockSeries::one_off(title, kind, day, start, *minutes)
                .map_err(|e| CliError::Message(e.to_string()))?;

            if let Some(repeat) = repeat {
                let tokens = words(repeat);
                let (spec, _, _) = lumenna_parse::date::parse_recurrence(&tokens, 0)
                    .ok_or_else(|| {
                        CliError::Message(format!("could not read a repetition from '{repeat}'"))
                    })?;
                let rrule = spec.to_rrule();
                // A series anchored on a day it never occurs is a trap (§5), so the start
                // date moves to the rule's first real occurrence.
                let rule = lumenna_core::recur::Rule::parse(&rrule)?;
                if let Some(first) = rule.first_from(day)? {
                    series.start_date = first;
                }
                series.rrule = Some(rrule);
                series.end_date = None;
            }

            profile.store.load_year(series.start_date.year())?;
            let (start, first) = (series.start_time, series.start_date);
            let id = series.id;
            profile.store.apply_recorded(&edit::create_series(series))?;
            Ok(Response::touched(
                format!(
                    "Added block {title} at {} on {}",
                    render::time_text(start),
                    first
                ),
                api::Affected { blocks: vec![id.to_string()], ..api::Affected::default() },
            ))
        }
        BlockCommand::List => {
            profile.store.load_all_years()?;
            let snapshot = state(profile);
            let mut live: Vec<&BlockSeries> =
                snapshot.series.values().filter(|s| s.deleted_at.is_none()).collect();
            live.sort_by_key(|s| (s.start_date, s.start_time, s.id));
            let count = u32::try_from(live.len()).unwrap_or(u32::MAX);
            let rows: Vec<Row> = live
                .iter()
                .enumerate()
                .map(|(index, series)| {
                    let mut value = format!(
                        "{} for {} minutes from {}",
                        render::time_text(series.start_time),
                        series.duration_mins,
                        series.start_date
                    );
                    if let Some(rrule) = &series.rrule {
                        value.push_str(&format!(", repeats: {rrule}"));
                    }
                    Row {
                        id: RowId::Occurrence(series.id, series.start_date),
                        role: Role::Block,
                        depth: 0,
                        index: u32::try_from(index).unwrap_or(u32::MAX) + 1,
                        count,
                        expanded: None,
                        checked: None,
                        title: series.title.clone(),
                        state: Vec::new(),
                        value: Some(value),
                        hint: None,
                    }
                })
                .collect();
            let listing = api::Rows::new(&rows, "block");
            Ok(Response::new(
                render::count_line(listing.count, "block"),
                Outcome::Rows(listing),
            ))
        }
        BlockCommand::Rm { id } => {
            profile.store.load_all_years()?;
            let snapshot = state(profile);
            let series_id = resolve_series(profile, &snapshot, id)?;
            let change = edit::trash_series(&snapshot, series_id)?;
            if change.is_empty() {
                return Ok(Response::unchanged("that block is already in the trash"));
            }
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed(&change))
        }
    }
}

/// Block rows are remembered as `<series>@<date>`, so a row number resolves to either.
fn resolve_series(profile: &Profile, snapshot: &Snapshot, input: &str) -> Result<SeriesId> {
    let candidates = ids_of(snapshot.series.keys());
    let resolved = profile.resolve(input, "block", &candidates)?;
    let bare = resolved.split('@').next().unwrap_or(&resolved);
    bare.parse()
        .map_err(|_| CliError::Message(format!("'{input}' is not a block")))
}

fn assign(
    profile: &mut Profile,
    task: &str,
    block: &str,
    date: Option<&str>,
    minutes: Option<u32>,
    now: &Zoned,
) -> Result<Response> {
    let day = match date {
        Some(text) => parse_date_phrase(text, now)?,
        None => now.date(),
    };
    profile.store.load_year(day.year())?;
    profile.store.load_all_years()?;
    let snapshot = state(profile);

    let task_id = resolve_task(profile, &snapshot, task)?;
    let series_id = resolve_series(profile, &snapshot, block)?;
    let series = snapshot
        .series
        .get(&series_id)
        .ok_or(CliError::Edit(lumenna_core::edit::EditError::NotFound { kind: "block" }))?;

    // A one-off block's assignments name only the series, so that moving the block carries
    // them with it (§3.7). A repeating block's name the day.
    let block_ref = if series.is_recurring() {
        BlockRef::Occurrence(series_id, day)
    } else {
        BlockRef::OneOff(series_id)
    };
    // A repeating block's assignment is sharded by the day it is for; a one-off's follows
    // its series, since the reference carries no date (§3.7).
    let year = if series.is_recurring() { day.year() } else { series.start_date.year() };

    let mut change = edit::assign_task(&snapshot, task_id, block_ref, year)?;
    if let Some(minutes) = minutes
        && let Some(edit::Change::Assignment { transition, .. }) = change.changes.first_mut()
        && let Some(assignment) = transition.after.as_mut()
    {
        assignment.planned_mins = Some(minutes);
    }
    let announcement = format!("{} into {} on {day}", change.description, series.title);
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed_as(announcement, &change))
}

fn resolve_assignment(
    profile: &Profile,
    snapshot: &Snapshot,
    input: &str,
) -> Result<AssignmentId> {
    let candidates = ids_of(snapshot.assignments.keys());
    let resolved = profile.resolve(input, "assignment", &candidates)?;
    resolved
        .parse()
        .map_err(|_| CliError::Message(format!("'{input}' is not an assignment")))
}

/// Which `blocks-<year>` document an assignment lives in.
///
/// A one-off block's assignment names no date, so its year is its series' — and a series
/// this device has not loaded gives no answer. Guessing would write the edit into a document
/// for the wrong year, holding a fragment of the record, so it is an error instead.
fn assignment_year(snapshot: &Snapshot, id: AssignmentId) -> Result<i16> {
    snapshot
        .assignments
        .get(&id)
        .and_then(|assignment| {
            lumenna_store::doc::assignment_year(
                assignment,
                snapshot.series.get(&assignment.block_ref.series_id()),
            )
        })
        .ok_or_else(|| {
            CliError::Message(
                "that assignment's block is not in this store, so there is no telling which \
                 year it belongs to"
                    .to_owned(),
            )
        })
}

fn unassign(profile: &mut Profile, input: &str) -> Result<Response> {
    profile.store.load_all_years()?;
    let snapshot = state(profile);
    let id = resolve_assignment(profile, &snapshot, input)?;
    let change = edit::unassign(&snapshot, id, assignment_year(&snapshot, id)?)?;
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed(&change))
}

fn start(profile: &mut Profile, input: &str, now: &Zoned) -> Result<Response> {
    profile.store.load_all_years()?;
    let snapshot = state(profile);
    let id = resolve_assignment(profile, &snapshot, input)?;
    let change = edit::start_timer(&snapshot, id, assignment_year(&snapshot, id)?, now)?;
    if change.is_empty() {
        return Ok(Response::unchanged("that timer is already running"));
    }
    profile.store.apply_recorded(&change)?;
    Ok(Response::changed_as("Started timer", &change))
}

fn stop(
    profile: &mut Profile,
    input: &str,
    minutes: Option<u32>,
    now: &Zoned,
) -> Result<Response> {
    profile.store.load_all_years()?;
    let snapshot = state(profile);
    let id = resolve_assignment(profile, &snapshot, input)?;
    let year = assignment_year(&snapshot, id)?;

    if let Some(minutes) = minutes {
        let change = edit::log_minutes(&snapshot, id, year, minutes)?;
        if change.is_empty() {
            return Ok(Response::unchanged(format!("{minutes} minutes were already logged")));
        }
        profile.store.apply_recorded(&change)?;
        return Ok(Response::changed(&change));
    }

    // The cap is the occurrence's own length — an exception may have shortened or
    // lengthened this day's block — falling back to the series' when the occurrence cannot
    // be found, since a timer is still worth stopping then.
    let assignment = &snapshot.assignments[&id];
    let cap = edit::occurrence_of(&snapshot, assignment.block_ref)
        .map(|occurrence| occurrence.duration_mins)
        .ok()
        .or_else(|| {
            snapshot.series.get(&assignment.block_ref.series_id()).map(|s| s.duration_mins)
        });

    let (change, elapsed) = edit::stop_timer(&snapshot, id, year, cap, now)?;
    if change.is_empty() {
        return Ok(Response::unchanged(format!(
            "that timer is not running; {} logged",
            render::count_line(elapsed.mins as usize, "minute")
        )));
    }
    profile.store.apply_recorded(&change)?;

    let response = Response::new(
        format!("Logged {} minutes", elapsed.mins),
        Outcome::Timer(api::Timer {
            assignment: id.to_string(),
            minutes: elapsed.mins,
            capped: elapsed.capped,
        }),
    );
    Ok(if elapsed.capped {
        // A truncated figure is not a fact (§3.7). It is recorded so the sitting is not lost,
        // and the user is told how to replace it with the real one.
        response.note(format!(
            "that timer ran past the end of its block, so it was capped at {} minutes; if \
             that is wrong, record the real figure with `lum stop {id} --minutes <n>`",
            elapsed.mins
        ))
    } else {
        response
    })
}

// ---------------------------------------------------------------------------------------
// Undo (§9)
// ---------------------------------------------------------------------------------------

/// `lum undo` and `lum redo`.
///
/// Always announced, never a silent state change (§9): without a visual channel a
/// mis-keystroke can go unnoticed for minutes, and so can an undo that did less than asked.
fn step(profile: &mut Profile, redo: bool) -> Result<Response> {
    use lumenna_store::undo::Step;
    let taken = if redo { profile.store.redo()? } else { profile.store.undo()? };
    Ok(match taken {
        Step::Nothing => {
            Response::unchanged(if redo { "nothing to redo" } else { "nothing to undo" })
        }
        Step::Unreadable => Response::unchanged(
            "the last change was saved by a different version of Lumenna and cannot be \
             reversed by this one; it has been set aside, and the one before it is next",
        ),
        Step::Done(reverted) => {
            let verb = if redo { "Redid" } else { "Undid" };
            let mut response =
                Response::changed_as(format!("{verb}: {}", reverted.description), &reverted.applied);
            if reverted.applied.is_empty() && reverted.kept.is_empty() {
                response = response.note("it was already that way");
            }
            for kept in reverted.kept {
                response = response.note(kept);
            }
            response
        }
    })
}

// ---------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------

fn config(profile: &mut Profile, command: &ConfigCommand) -> Result<Response> {
    let snapshot = state(profile);
    let settings = snapshot.settings.clone();

    match command {
        ConfigCommand::Get { key } => {
            let all: Vec<api::Setting> = [
                ("cascade-complete-subtasks", settings.cascade_complete_subtasks.to_string()),
                (
                    "verbosity",
                    match settings.verbosity {
                        lumenna_core::model::Verbosity::Terse => "terse".to_owned(),
                        lumenna_core::model::Verbosity::Full => "full".to_owned(),
                    },
                ),
                ("all-day-reminder-hour", settings.all_day_reminder_hour.to_string()),
                ("day-start", settings.day_window.0.to_string()),
                ("day-end", settings.day_window.1.to_string()),
                ("week-start", format!("{:?}", settings.week_start).to_lowercase()),
            ]
            .into_iter()
            .map(|(key, value)| api::Setting { key: key.to_owned(), value })
            .chain(durability::device_settings(profile)?)
            .collect();

            match key {
                Some(key) => {
                    let found = all
                        .into_iter()
                        .find(|setting| setting.key == *key)
                        .ok_or_else(|| CliError::Message(format!("no setting called '{key}'")))?;
                    Ok(Response::new(
                        found.value.clone(),
                        Outcome::Settings(api::SettingList { settings: vec![found] }),
                    ))
                }
                None => Ok(Response::new(
                    render::count_line(all.len(), "setting"),
                    Outcome::Settings(api::SettingList { settings: all }),
                )),
            }
        }
        ConfigCommand::Set { key, value } if durability::DEVICE_KEYS.contains(&key.as_str()) => {
            durability::set_device_setting(profile, key, value)
        }
        ConfigCommand::Set { key, value } => {
            let mut after = settings.clone();
            match key.as_str() {
                "cascade-complete-subtasks" => {
                    after.cascade_complete_subtasks = parse_bool(value)?;
                }
                "verbosity" => {
                    after.verbosity = match value.to_lowercase().as_str() {
                        "terse" => lumenna_core::model::Verbosity::Terse,
                        "full" => lumenna_core::model::Verbosity::Full,
                        _ => {
                            return Err(CliError::Message(
                                "verbosity is terse or full".to_owned(),
                            ));
                        }
                    };
                }
                "all-day-reminder-hour" => after.all_day_reminder_hour = parse_time_phrase(value)?,
                "day-start" => after.day_window.0 = parse_time_phrase(value)?,
                "day-end" => after.day_window.1 = parse_time_phrase(value)?,
                "week-start" => {
                    after.week_start = lumenna_parse::date::weekday_of(&value.to_lowercase())
                        .ok_or_else(|| {
                            CliError::Message(format!("'{value}' is not a day of the week"))
                        })?;
                }
                other => return Err(CliError::Message(format!("no setting called '{other}'"))),
            }
            let change = edit::update_settings(settings, after);
            if change.is_empty() {
                return Ok(Response::unchanged("nothing changed"));
            }
            profile.store.apply_recorded(&change)?;
            Ok(Response::changed_as(format!("{key} is now {value}"), &change))
        }
    }
}

fn parse_bool(text: &str) -> Result<bool> {
    match text.to_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Ok(true),
        "false" | "no" | "off" | "0" => Ok(false),
        other => Err(CliError::Message(format!("'{other}' is not yes or no"))),
    }
}
