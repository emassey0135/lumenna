//! `lum` — Lumenna's command line.
//!
//! The first target, and a permanent one. It is fully accessible by construction, and it
//! forces the core API to be complete before any GUI can paper over gaps in it.
//!
//! # Completeness, for two reasons
//!
//! **It is a first-class interface.** Some people simply prefer text — `edbrowse` exists
//! because that preference is real and long-standing, not because those users' GUIs are
//! broken. Add scripting, and the CLI is a product in its own right.
//!
//! **And it is a test.** If something is only possible in a GUI, that is business logic that
//! leaked out of the core. CLI coverage is the cheapest checkable proxy for core coverage
//! there is — far easier to audit than reading eleven UI implementations looking for logic
//! that should not be there.
//!
//! Treating it only as a test would harm it, though. A test optimises for coverage; a
//! product optimises for use. So: short identifiers, because a UUID is thirty-six characters
//! and worse to dictate than to type; `--json` as a versioned contract rather than a
//! debugging convenience; and `--help` treated as an **accessibility surface**, since it is
//! the primary discovery mechanism for anyone who cannot skim a GUI.

mod assist;
mod complete;
mod durability;
mod error;
mod mcp;
mod network;
mod profile;
mod render;
mod service;

use lumenna_host::ActionKind;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
// The envelope `--json` writes is the one every RPC server writes.
pub(crate) use lumenna_host::rpc::api;
use lumenna_host::{BlockEdit, BlockScope, Direction, MoveTarget, NewBlock, TaskEdit};

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

    /// Work with reminders: a task's or a block's, and what comes up.
    #[command(subcommand)]
    Reminder(ReminderCommand),

    /// Say reminders in this terminal as they come due, until stopped: for a console
    /// without a desktop to show them. Written to the terminal, which speakup and BRLTTY
    /// read; with --speak, through speech-dispatcher as well.
    Notify {
        /// Also say each one with spd-say.
        #[arg(long)]
        speak: bool,
    },

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

    /// Skip a sitting not yet started, or plan a skipped one again with `--again`.
    ///
    /// Skipped, it stays on the record, so the task's history says it was put here and left.
    Skip {
        /// The assignment identifier.
        assignment: String,
        /// Plan it again instead.
        #[arg(long)]
        again: bool,
    },

    /// Move a sitting one place up or down among its block's.
    Reorder {
        /// The assignment identifier.
        assignment: String,
        /// Which way: `up` or `down`.
        #[arg(value_parser = ["up", "down"])]
        direction: String,
    },

    /// Set how long a sitting is meant to take, or clear it with `none`.
    ///
    /// The plan, not the record: what was logged stays as it is. `lum stop --minutes` is how
    /// the time actually spent is put right.
    Length {
        /// A row number from `lum plan`, or the assignment identifier.
        assignment: String,
        /// Minutes, or `none`.
        minutes: String,
    },

    /// End the block under way now, on this day alone.
    End {
        /// A row number from `lum plan`, or the block's identifier.
        block: String,
    },
    /// Give the block under way longer, on this day alone.
    Extend {
        /// A row number from `lum plan`, or the block's identifier.
        block: String,
        /// How much longer, such as 15m.
        #[arg(long, default_value = "15m")]
        by: String,
    },
    /// End the block under way and clear the rest of today: repeating blocks are cancelled
    /// for today, ones that happen once go to the trash. One undo brings it all back.
    EndDay,
    /// Clear the rest of today, leaving the block under way.
    ClearDay,
    /// Work out the rest of today again from now: says what would change, and changes it
    /// only with --apply.
    Replan {
        /// Leave this block where it is (a row number from `lum plan`, or its identifier).
        #[arg(long)]
        keep: Vec<String>,
        /// Skip this block today.
        #[arg(long)]
        skip: Vec<String>,
        /// Take this other option, by its number, before anything else.
        #[arg(long)]
        option: Option<usize>,
        /// Make the changes proposed: all of them, or those numbered, such as 1,3.
        #[arg(long, num_args = 0..=1, default_missing_value = "all")]
        apply: Option<String>,
    },
    /// What to work on now: the block under way's own sittings, then what is suggested.
    /// With --start, starts on one of them.
    Next {
        /// Start on this one, by its number in the list.
        #[arg(long)]
        start: Option<usize>,
    },
    /// Plan a whole day: each work block still to come given its most urgent tasks until
    /// full. Says the plan, and makes it only with --apply.
    PlanDay {
        /// The day, such as tomorrow; today when left out.
        date: Option<String>,
        /// Leave this task out (a row number, or its identifier).
        #[arg(long)]
        leave: Vec<String>,
        /// Make the sittings proposed: all of them, or those numbered, such as 1,3.
        #[arg(long, num_args = 0..=1, default_missing_value = "all")]
        apply: Option<String>,
    },
    /// Carry the rest of everything a block left short, each to the next block that takes it.
    CarryRest {
        /// A row number from `lum plan`, or `<block>@<date>`.
        block: String,
    },
    /// Put the rest of a sitting's planned time in another block.
    Carry {
        /// A row number from `lum plan`, or the assignment identifier.
        assignment: String,
        /// The block to carry it to, `<series>@<date>` or a row number from `lum plan`.
        #[arg(long)]
        to: String,
    },

    /// Start the timer on an assignment, or resume a paused one.
    Start {
        /// The assignment identifier.
        assignment: String,
    },

    /// Pause the timer: the time so far is kept and the sitting stays in progress, to be
    /// resumed with `lum start` or ended with `lum stop`.
    Pause {
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

    /// Pair this device with another of yours.
    ///
    /// Run it on both devices. On one network they find each other; otherwise run it on one
    /// and give the other the code it prints. Both show the same three words — check they
    /// match, and say yes on both.
    Pair {
        /// The other device's pairing code, when it is not on this network.
        code: Option<String>,
        /// Use the local network only: no relay, no lookup service.
        #[arg(long)]
        local_only: bool,
    },

    /// Sync with your other devices now, or say how syncing is going.
    Sync {
        #[command(subcommand)]
        what: Option<SyncCommand>,
        /// Use the local network only: no relay, no lookup service.
        #[arg(long)]
        local_only: bool,
    },

    /// Stay running and keep this device in sync.
    ///
    /// Also serves the command surface on a socket in the profile, which the BTSpeak app
    /// uses when it is there. Stop it with Control-C.
    SyncDaemon {
        /// Use the local network only: no relay, no lookup service.
        #[arg(long)]
        local_only: bool,
    },

    /// Your paired devices.
    #[command(subcommand)]
    Device(DeviceCommand),

    /// Run the sync daemon as a service, so this device stays in sync on its own.
    #[command(subcommand)]
    Daemon(DaemonCommand),

    /// What was done over the last two weeks, newest first.
    History,

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

    /// Erase everything deleted permanently: the trash, and the history of everything ever
    /// deleted, on every device.
    ///
    /// The store begins its history again without them, every device takes that in as it next
    /// syncs, and each deletes its automatic backups and takes a new one. Nothing can undo it,
    /// and a backup saved anywhere else still holds what was erased.
    EraseDeleted {
        /// Skip the confirmation.
        #[arg(long)]
        yes: bool,
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

    /// Serve the store to an AI assistant, over the Model Context Protocol on stdin and stdout.
    ///
    /// Add it to an assistant's MCP servers as the command `lum mcp`. Its tools read tasks, the
    /// day and its blocks, and what to work on, and propose replans; with --writes, they also
    /// add, edit, complete, plan and time tasks, each change one you can undo, and erase
    /// permanently, which nothing undoes, only once you have said yes to it. Records are
    /// named by identifier. A model in the cloud sends whatever it reads here off this device;
    /// a local one keeps it here: choose the assistant with that in mind.
    Mcp {
        #[command(subcommand)]
        command: Option<McpCommand>,
        /// Offer the tools that change the store, as well as those that read it.
        #[arg(long)]
        writes: bool,
        /// Serve over HTTP instead, at http://ADDR/mcp (127.0.0.1:7341 when no address is
        /// given), for a client that cannot start `lum` itself. Off this machine's loopback, a
        /// client must send `Authorization: Bearer` with the token `lum mcp token` prints. There
        /// is no TLS: off the machine, put Tailscale or a reverse proxy in front.
        #[arg(long, value_name = "ADDR", num_args = 0..=1, default_missing_value = "127.0.0.1:7341")]
        listen: Option<String>,
        /// Ask for the token on loopback too: for a tunnel or reverse proxy in front, which
        /// reaches this from elsewhere through loopback.
        #[arg(long, requires = "listen")]
        require_token: bool,
    },

    /// Run one of the operations an assistant can do, as `lum mcp`, Siri and Gemini run it.
    ///
    /// Give its parameters as name=value (`lum assist complete_task id=0193a`), or all of them
    /// as one JSON object with --args. Records are named by identifier, never by row number.
    /// --list says every operation and what it takes. One that erases permanently runs only
    /// with confirm=true, since nothing can undo it. With --json, both answer as every other
    /// command does.
    Assist {
        /// The operation, such as list_tasks or add_task.
        #[arg(required_unless_present = "list", add = clap_complete::ArgValueCandidates::new(complete::operations))]
        operation: Option<String>,
        /// Its parameters, each as name=value.
        #[arg(value_name = "NAME=VALUE", allow_hyphen_values = true)]
        given: Vec<String>,
        /// Its parameters as one JSON object, or `-` to read that from standard input.
        #[arg(long, value_name = "JSON", conflicts_with = "list")]
        args: Option<String>,
        /// Say every operation, with what it takes, instead of running one.
        #[arg(long, conflicts_with_all = ["operation", "given"])]
        list: bool,
    },

    /// Say what could come next in a quick-add line, a filter, or a project's or label's name,
    /// as typed so far: for a shell or an editor completing one.
    Complete {
        /// What is being typed.
        #[arg(value_enum)]
        kind: complete::Typed,
        /// What has been typed so far, completed at its end.
        #[arg(default_value = "", allow_hyphen_values = true)]
        text: String,
    },

    /// Print a shell completion script, which completes project, label and filter names from
    /// the store as one types.
    Completions {
        /// Which shell.
        shell: clap_complete::Shell,
    },
}

/// What `lum mcp` does besides serving.
#[derive(Subcommand, Debug)]
pub(crate) enum McpCommand {
    /// Print the token an HTTP client sends (`Authorization: Bearer <token>`), making one the
    /// first time. It is this device's, in the profile, and never syncs.
    Token {
        /// Replace it, so a client given the old one is refused.
        #[arg(long)]
        new: bool,
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
        /// A date phrase, or `none` to clear it. A date without a repetition keeps the one
        /// the task has.
        #[arg(long)]
        due: Option<String>,
        /// A repetition, such as `every monday` or `every! 2 weeks`, or `none` to stop it
        /// repeating.
        #[arg(long)]
        repeat: Option<String>,
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
        #[arg(long, add = clap_complete::ArgValueCompleter::new(complete::projects))]
        project: Option<String>,
        /// The labels it should wear, comma-separated, replacing its own. An empty string
        /// takes them all off. New names become labels.
        #[arg(long)]
        labels: Option<String>,
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
    /// Hold a task's due time to one place's time zone, or let it float with this device's
    /// (`none`).
    Zone {
        /// A row number from the last listing, or an identifier.
        id: String,
        /// A place's zone, such as `America/New_York` or `Europe/London`, or `none`.
        zone: String,
    },

    /// What has happened to a task, and what makes up its urgency.
    History {
        /// A row number from the last listing, or an identifier.
        id: String,
    },
    /// Delete a task from the trash. Undo can bring it back, and it stays in the history every
    /// device keeps and in backups.
    ///
    /// With --permanently it is erased from that history too, on every device: the store
    /// begins its history again without it, and without the history of everything deleted
    /// before. Nothing can undo that.
    Erase {
        /// A row number from the last listing, or an identifier.
        id: String,
        /// Erase it permanently, from the history every device keeps.
        #[arg(long)]
        permanently: bool,
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
        #[arg(long, add = clap_complete::ArgValueCompleter::new(complete::projects))]
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
        #[arg(long, add = clap_complete::ArgValueCompleter::new(complete::projects))]
        parent: Option<String>,
    },
    /// List projects.
    List,
    /// Rename a project.
    Rename {
        /// The project to rename.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
        /// Its new name.
        to: String,
    },
    /// Archive a project, keeping its tasks out of active views — or unarchive one that is.
    Archive {
        /// The project to archive or unarchive.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
    },
    /// Delete a project.
    Rm {
        /// The project to delete.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
        /// Keep its tasks, moved to the Inbox, instead of trashing them.
        #[arg(long)]
        keep_tasks: bool,
    },
    /// Move a project under another, or to the top level.
    Move {
        /// The project to move.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
        /// Put it under this project.
        #[arg(long, conflicts_with = "top")]
        parent: Option<String>,
        /// Put it at the top level.
        #[arg(long)]
        top: bool,
    },
    /// Move a project one place up or down among its siblings.
    Order {
        /// The project.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
        /// up or down.
        direction: Way,
    },
    /// Set a project's urgency multiplier, roughly 0.5 to 2.0.
    ///
    /// This is not a second priority. Task priority is how much one item matters; weight is
    /// how much a whole area matters right now.
    Weight {
        /// The project.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
        /// The multiplier, or `inherit` to take the parent's again.
        value: String,
    },
    /// Give a project a colour, or `none`. The name always shows too.
    Colour {
        /// The project.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::projects))]
        name: String,
        /// A colour name, such as `red`, or `none`.
        colour: String,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ReminderCommand {
    /// List a task's or block's reminders: its own, or the defaults it takes.
    List {
        /// A row number from the last listing, or a task or block identifier.
        target: String,
    },
    /// Give a task or block a reminder of its own, in place of the defaults.
    Add {
        /// A row number from the last listing, or a task or block identifier.
        target: String,
        /// What it counts from: due (a task), start or end (a block); or a moment of its own,
        /// such as "tomorrow 9am".
        #[arg(long)]
        at: String,
        /// How long before, such as 5m or 1h.
        #[arg(long, conflicts_with = "after")]
        before: Option<String>,
        /// How long after, such as 5m.
        #[arg(long)]
        after: Option<String>,
    },
    /// Remove one of a task's or block's own reminders.
    Rm {
        /// Its identifier, from `lum reminder list`.
        id: String,
    },
    /// Choose which devices one of a task's or block's own reminders fires on.
    Deliver {
        /// Its identifier, from `lum reminder list`.
        id: String,
        /// `all`, `only:<device>` or `except:<device>`, a device by its identifier from
        /// `lum device list`.
        on: String,
    },
    /// What comes up over the next hours.
    Upcoming {
        /// How many hours ahead.
        #[arg(long, default_value_t = 24)]
        hours: u32,
    },
    /// Dismiss one firing, on every device.
    Dismiss {
        /// The key a notification gives.
        key: String,
        /// The occurrence it names, as an ISO date.
        occurrence: String,
    },
    /// Snooze one firing, on every device.
    Snooze {
        /// The key a notification gives.
        key: String,
        /// The occurrence it names, as an ISO date.
        occurrence: String,
        /// How long, such as 10m.
        #[arg(long)]
        r#for: Option<String>,
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
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::labels))]
        name: String,
        /// Its new name.
        to: String,
    },
    /// Fold one label into another, for when a typo made a near-duplicate.
    Merge {
        /// The label to retire.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::labels))]
        from: String,
        /// The label to keep.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::labels))]
        into: String,
    },
    /// Move a label one place up or down.
    Order {
        /// The label.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::labels))]
        name: String,
        /// up or down.
        direction: Way,
    },
    /// Give a label a colour, or `none`. The name always shows too.
    Colour {
        /// The label.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::labels))]
        name: String,
        /// A colour name, such as `red`, or `none`.
        colour: String,
    },
    /// Delete a label. Tasks wearing it simply stop showing it.
    Rm {
        /// The label to delete.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::labels))]
        name: String,
        /// Skip the confirmation.
        #[arg(long)]
        yes: bool,
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
    /// Rename a saved filter or change its query.
    Edit {
        /// Its name.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::filters))]
        name: String,
        /// A new name.
        #[arg(long)]
        rename: Option<String>,
        /// A new query.
        #[arg(long)]
        query: Option<String>,
    },
    /// Move a saved filter one place up or down.
    Order {
        /// Its name.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::filters))]
        name: String,
        /// up or down.
        direction: Way,
    },
    /// Give a saved filter a colour, or `none`. The name always shows too.
    Colour {
        /// Its name.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::filters))]
        name: String,
        /// A colour name, such as `red`, or `none`.
        colour: String,
    },
    /// Delete a saved filter.
    Rm {
        /// Its name.
        #[arg(add = clap_complete::ArgValueCompleter::new(complete::filters))]
        name: String,
        /// Skip the confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// What a block can be given beyond its time, length, kind and repetition.
#[derive(clap::Args, Debug, Clone, Default)]
pub(crate) struct BlockExtras {
    /// Notes about the block; empty clears them.
    #[arg(long)]
    notes: Option<String>,
    /// Whether tasks can be put in it: yes or no. The kind decides when left out.
    #[arg(long, value_parser = clap::builder::BoolishValueParser::new())]
    takes_tasks: Option<bool>,
    /// Whether it counts toward the hours available for work: yes or no.
    #[arg(long, value_parser = clap::builder::BoolishValueParser::new())]
    counts_capacity: Option<bool>,
    /// Whether it is fixed in time, never moved when the day slips: yes or no.
    #[arg(long, value_parser = clap::builder::BoolishValueParser::new())]
    anchored: Option<bool>,
    /// How short re-flow may make it, in minutes; 0 returns it to the kind's default.
    #[arg(long)]
    min_minutes: Option<u32>,
    /// A filter scoping which tasks are offered for it, such as `#Work`; empty clears it.
    #[arg(long)]
    task_filter: Option<String>,
    /// The last day a repeating block happens, or `none` to repeat for good.
    #[arg(long)]
    until: Option<String>,
    /// A colour, by name; empty clears it.
    #[arg(long)]
    colour: Option<String>,
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
        #[command(flatten)]
        extras: BlockExtras,
    },
    /// List block series.
    List,
    /// Show one block series: when, how long, what kind, and how it repeats.
    Show {
        /// A row number from `lum block list` or `lum plan`, or an identifier.
        id: String,
    },
    /// Change a block: every occurrence with --all, or one day with --date.
    ///
    /// A repeating block always needs one or the other — which occurrences a change means is
    /// never guessed. A block that happens once needs neither.
    Edit {
        /// A row number from `lum block list` or `lum plan`, or an identifier.
        id: String,
        /// A new name.
        #[arg(long)]
        title: Option<String>,
        /// A new start time, such as `9am`.
        #[arg(long)]
        at: Option<String>,
        /// A new length in minutes.
        #[arg(long)]
        minutes: Option<u32>,
        /// work, break, or event.
        #[arg(long)]
        kind: Option<String>,
        /// A new repetition, such as `every weekday`, or `none`. Every occurrence only.
        #[arg(long)]
        repeat: Option<String>,
        /// Change only the occurrence on this day.
        #[arg(long, conflicts_with = "all")]
        date: Option<String>,
        /// Change every occurrence.
        #[arg(long)]
        all: bool,
        #[command(flatten)]
        extras: BlockExtras,
    },
    /// Cancel one day of a repeating block, leaving the rest.
    Cancel {
        /// A row number or identifier.
        id: String,
        /// Which day.
        #[arg(long)]
        date: String,
    },
    /// Put one day of a repeating block back as the series has it.
    Restore {
        /// A row number or identifier.
        id: String,
        /// Which day.
        #[arg(long)]
        date: String,
    },
    /// Hold a block to one place's time zone, or let it float with this device's (`none`).
    Zone {
        /// A row number or identifier.
        id: String,
        /// A place's zone, such as `America/New_York` or `Europe/London`, or `none`.
        zone: String,
    },
    /// Delete a block series.
    Rm {
        /// Its identifier.
        id: String,
        /// Skip the confirmation.
        #[arg(long)]
        yes: bool,
    },
}

/// Which way to move something in its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Way {
    /// One place earlier.
    Up,
    /// One place later.
    Down,
}

impl From<Way> for Direction {
    fn from(way: Way) -> Self {
        match way {
            Way::Up => Self::Up,
            Way::Down => Self::Down,
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum DaemonCommand {
    /// Install `lum sync-daemon` as a service that starts at login and keeps running.
    ///
    /// A user service on Linux, which keeps running after you log out; a system service on a
    /// BTSpeak, which asks for your password through sudo; a LaunchAgent on macOS; a task at
    /// logon on Windows. It syncs this profile, whatever its environment says.
    Install {
        /// Use the local network only: no relay, no lookup service.
        #[arg(long)]
        local_only: bool,
    },
    /// Stop the service and remove it.
    Uninstall,
    /// Start the installed service.
    Start,
    /// Stop the installed service until the next login, or `lum daemon start`.
    Stop,
    /// Whether the service is installed, and whether it is running.
    Status,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SyncCommand {
    /// How syncing is going, device by device.
    Status,
}

#[derive(Debug, Subcommand)]
pub(crate) enum DeviceCommand {
    /// List your paired devices.
    List,
    /// Rename a device.
    Rename {
        /// The device, by name or the start of its identifier.
        device: String,
        /// Its new name.
        name: String,
    },
    /// Stop syncing with a device.
    ///
    /// It keeps what it already has: this is for a device you replaced, not one that was
    /// stolen.
    Unpair {
        /// The device, by name or the start of its identifier.
        device: String,
        /// Skip the confirmation.
        #[arg(long)]
        yes: bool,
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
    // Completing for the shell (`COMPLETE=bash lum ...`) answers and exits before anything else.
    clap_complete::CompleteEnv::with_factory(<Cli as clap::CommandFactory>::command).bin("lum").complete();
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
        // The dynamic registration, which asks `lum` itself as one types, so projects, labels and
        // filters come from the store; a shell without one gets the static script.
        let name = shell.to_string();
        match clap_complete::env::Shells::builtins().completer(&name) {
            Some(completer) => completer
                .write_registration("COMPLETE", "lum", "lum", "lum", &mut std::io::stdout())
                .map_err(|e| CliError::Message(e.to_string()))?,
            None => {
                let mut command = <Cli as clap::CommandFactory>::command();
                clap_complete::generate(*shell, &mut command, "lum", &mut std::io::stdout());
            }
        }
        return Ok(());
    }

    let profile = Profile::open(cli.profile.as_deref())?;
    if matches!(cli.command, Command::Rpc) {
        return network::serve_rpc(&profile);
    }
    if let Command::Mcp { command, writes, listen, require_token } = &cli.command {
        return match command {
            Some(McpCommand::Token { new }) => {
                println!("{}", mcp::token(&profile, *new)?);
                Ok(())
            }
            None => {
                let listen = listen.clone().map(|address| mcp::Listen { address, require_token: *require_token });
                mcp::serve(&profile, *writes, listen)
            }
        };
    }
    if let Command::SyncDaemon { local_only } = cli.command {
        durability::back_up_if_due(&profile);
        return network::daemon(&profile, local_only);
    }

    // Every client backs up opportunistically, because a schedule only exists where
    // something stays running.
    if !matches!(cli.command, Command::Backup { .. }) {
        durability::back_up_if_due(&profile);
    }

    let response = dispatch(&profile, &cli.command)?;
    // Row numbers count against whatever was listed last, so the listing is recorded from
    // the response rather than by each command that produces one — one place that can be
    // wrong instead of six.
    if let Some(listing) = listing_of(&response.outcome) {
        profile.remember(&listing);
    }
    let clock = if format == Format::Text { clock(&profile) } else { render::Clock::default() };
    render::emit(&response, format, clock);
    Ok(())
}

/// This device's `clock` setting, read after the command so `lum config set clock` takes
/// effect at once. A store that cannot answer prints the 24-hour clock rather than nothing.
fn clock(profile: &Profile) -> render::Clock {
    profile
        .settings(Some("clock".to_owned()))
        .ok()
        .and_then(|list| list.settings.into_iter().next())
        .map_or_else(render::Clock::default, |setting| render::Clock::from_setting(&setting.value))
}

/// Runs one command and returns what it produced.
///
/// The one place a command becomes a [`Response`], so `lum rpc` reaches every operation the
/// command line does by building the same [`Command`]. Each arm resolves row numbers —
/// the terminal's own affordance — and calls the surface, which does the rest.
pub(crate) fn dispatch(profile: &Profile, command: &Command) -> Result<Response> {
    Ok(match command {
        Command::Completions { .. } | Command::Rpc | Command::Mcp { .. } | Command::SyncDaemon { .. } => {
            unreachable!("handled before dispatch")
        }
        Command::Task(command) => return task(profile, command),
        Command::Project(command) => return project(profile, command),
        Command::Label(command) => Response::new(match command {
            LabelCommand::Add { name } => profile.add_label(name)?,
            LabelCommand::List => return Ok(Response::new(profile.list_labels()?)),
            LabelCommand::Rename { name, to } => profile.rename_label(name, to)?,
            LabelCommand::Merge { from, into } => profile.merge_labels(from, into)?,
            LabelCommand::Order { name, direction } => {
                profile.reorder_label(name, (*direction).into())?
            }
            LabelCommand::Colour { name, colour } => profile.recolour_label(name, colour_given(colour))?,
            LabelCommand::Rm { name, yes } => {
                let asked = profile.list_labels()?.rows.into_iter().find(|r| r.title.eq_ignore_ascii_case(name.trim_start_matches('@')));
                if !confirmed(asked.map(|r| r.actions), ActionKind::Delete, *yes, false)? {
                    return Ok(Response::unchanged("nothing was deleted"));
                }
                profile.delete_label(name)?
            }
        }),
        Command::Filter(command) => match command {
            FilterCommand::Add { name, query } => {
                Response::new(profile.add_filter(name, &query.join(" "))?)
            }
            FilterCommand::List => Response::new(profile.list_filters()?),
            FilterCommand::Edit { name, rename, query } => {
                Response::new(profile.edit_filter(name, rename.clone(), query.clone())?)
            }
            FilterCommand::Order { name, direction } => {
                Response::new(profile.reorder_filter(name, (*direction).into())?)
            }
            FilterCommand::Colour { name, colour } => Response::new(profile.recolour_filter(name, colour_given(colour))?),
            FilterCommand::Rm { name, yes } => {
                let asked = profile.list_filters()?.filters.into_iter().find(|f| f.name.eq_ignore_ascii_case(name));
                if !confirmed(asked.map(|f| f.actions), ActionKind::Delete, *yes, false)? {
                    return Ok(Response::unchanged("nothing was deleted"));
                }
                Response::new(profile.delete_filter(name)?)
            }
        },
        Command::Plan { date } => {
            let date = date.join(" ");
            Response::new(profile.plan(Some(date).filter(|d| !d.trim().is_empty()))?)
        }
        Command::Block(command) => match command {
            BlockCommand::Add { title, at, minutes, date, kind, repeat, extras } => {
                Response::new(profile.add_block(NewBlock {
                    title: title.clone(),
                    at: at.clone(),
                    minutes: *minutes,
                    date: date.clone(),
                    kind: kind.clone(),
                    repeat: repeat.clone(),
                    notes: extras.notes.clone(),
                    accepts_tasks: extras.takes_tasks,
                    counts_capacity: extras.counts_capacity,
                    anchored: extras.anchored,
                    min_minutes: extras.min_minutes,
                    task_filter: extras.task_filter.clone(),
                    until: extras.until.clone(),
                    colour: extras.colour.clone(),
                })?)
            }
            BlockCommand::List => Response::new(profile.list_blocks()?),
            BlockCommand::Show { id } => Response::new(profile.show_block(&profile.row(id, "block")?)?),
            BlockCommand::Edit { id, title, at, minutes, kind, repeat, date, all, extras } => {
                let id = profile.row(id, "block")?;
                let scope = match date {
                    Some(date) => BlockScope::Occurrence { date: date.clone() },
                    None => BlockScope::Series,
                };
                let edit = BlockEdit {
                    title: title.clone(),
                    at: at.clone(),
                    minutes: *minutes,
                    kind: kind.clone(),
                    repeat: repeat.clone(),
                    notes: extras.notes.clone(),
                    accepts_tasks: extras.takes_tasks,
                    counts_capacity: extras.counts_capacity,
                    anchored: extras.anchored,
                    min_minutes: extras.min_minutes,
                    task_filter: extras.task_filter.clone(),
                    until: extras.until.clone(),
                    colour: extras.colour.clone(),
                };
                if scope == BlockScope::Series && !all && profile.show_block(&id)?.repeats {
                    return Err(CliError::Message(
                        "that block repeats; say --date <day> to change one day, or --all to \
                         change every one"
                            .to_owned(),
                    ));
                }
                Response::new(profile.edit_block(&id, edit, scope)?)
            }
            BlockCommand::Cancel { id, date } => {
                Response::new(profile.cancel_occurrence(&profile.row(id, "block")?, date)?)
            }
            BlockCommand::Restore { id, date } => {
                Response::new(profile.restore_occurrence(&profile.row(id, "block")?, date)?)
            }
            BlockCommand::Zone { id, zone } => Response::new(profile.set_block_zone(&profile.row(id, "block")?, Some(zone.clone()))?),
            BlockCommand::Rm { id, yes } => {
                let id = profile.row(id, "block")?;
                let series = id.split('@').next().unwrap_or(&id).to_owned();
                let asked = profile.list_blocks()?.rows.into_iter().find(|r| r.id.starts_with(&series));
                if !confirmed(asked.map(|r| r.actions), ActionKind::Delete, *yes, false)? {
                    return Ok(Response::unchanged("nothing was deleted"));
                }
                Response::new(profile.delete_block(&id)?)
            }
        },
        Command::Assign { task, block, date, minutes } => Response::new(profile.assign(
            &profile.row(task, "task")?,
            &profile.row(block, "block")?,
            date.clone(),
            *minutes,
        )?),
        Command::Unassign { assignment } => {
            Response::new(profile.unassign(&profile.row(assignment, "assignment")?)?)
        }
        Command::Skip { assignment, again } => {
            Response::new(profile.skip_sitting(&profile.row(assignment, "assignment")?, !*again)?)
        }
        Command::Reorder { assignment, direction } => {
            let direction = if direction == "up" { lumenna_host::Direction::Up } else { lumenna_host::Direction::Down };
            Response::new(profile.reorder_sitting(&profile.row(assignment, "assignment")?, direction)?)
        }
        Command::Length { assignment, minutes } => {
            let minutes = if minutes.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(minutes.trim().parse::<u32>().map_err(|_| {
                    CliError::Message(format!("'{minutes}' is not a number of minutes, or none"))
                })?)
            };
            Response::new(profile.plan_minutes(&profile.row(assignment, "assignment")?, minutes)?)
        }
        Command::End { block } => Response::new(profile.end_block_now(&profile.row(block, "block")?)?),
        Command::Extend { block, by } => {
            let minutes = lumenna_host::parse_minutes(by.clone())?;
            Response::new(profile.extend_block(&profile.row(block, "block")?, minutes)?)
        }
        Command::EndDay => Response::new(profile.end_day(true)?),
        Command::ClearDay => Response::new(profile.end_day(false)?),
        Command::Next { start } => {
            let offered = profile.what_should_i_work_on(None)?;
            match start {
                None => Response::new(offered),
                Some(n) => {
                    let choice = offered.choices.get(n.wrapping_sub(1)).ok_or_else(|| {
                        CliError::Message(format!("there is no {n}: {}", offered.announcement))
                    })?;
                    Response::new(profile.start_on(&choice.id, None)?)
                }
            }
        }
        Command::PlanDay { date, leave, apply } => {
            let exclude = leave.iter().map(|row| profile.row(row, "task")).collect::<std::result::Result<Vec<_>, _>>()?;
            let pins = lumenna_host::DayPins { exclude, declined: Vec::new() };
            let proposal = profile.propose_day_plan(date.clone(), time_style(profile), pins, None)?;
            match apply.as_deref() {
                None => Response::new(proposal),
                Some(numbers) => {
                    let accepted = numbered(numbers, proposal.sittings.clone())?;
                    Response::new(profile.apply_day_plan(proposal, accepted, None)?)
                }
            }
        }
        Command::Replan { keep, skip, option, apply } => {
            let series = |rows: &[String]| -> Result<Vec<String>> {
                rows.iter().map(|row| Ok(profile.row(row, "block")?.split('@').next().unwrap_or_default().to_owned())).collect()
            };
            let pins = lumenna_host::ReplanPins { keep: series(keep)?, skip: series(skip)?, ..Default::default() };
            let mut proposal = profile.replan(time_style(profile), pins, None)?;
            if let Some(n) = option {
                let taken = proposal.alternatives.get(n.wrapping_sub(1)).ok_or_else(|| {
                    CliError::Message(format!("there is no option {n}: {} offered", proposal.alternatives.len()))
                })?;
                proposal = profile.replan(time_style(profile), taken.pins.clone(), None)?;
            }
            match apply.as_deref() {
                None => Response::new(proposal),
                Some(numbers) => {
                    let accepted = numbered(numbers, proposal.changes.clone())?;
                    Response::new(profile.apply_replan(proposal, accepted, None)?)
                }
            }
        }
        Command::CarryRest { block } => Response::new(profile.carry_rest_forward(&profile.row(block, "block")?)?),
        Command::Carry { assignment, to } => {
            Response::new(profile.carry_forward(&profile.row(assignment, "assignment")?, &profile.row(to, "block")?)?)
        }
        Command::Start { assignment } => {
            Response::new(profile.start_timer(&profile.row(assignment, "assignment")?)?)
        }
        Command::Pause { assignment } => {
            Response::new(profile.pause_timer(&profile.row(assignment, "assignment")?)?)
        }
        Command::Stop { assignment, minutes } => {
            let timer = profile.stop_timer(&profile.row(assignment, "assignment")?, *minutes)?;
            let capped = timer.changed && timer.capped && profile.at_terminal();
            let id = timer.assignment.clone();
            let response = Response::new(timer);
            if capped {
                response.note(format!("`lum stop {id} --minutes <n>` records the real figure"))
            } else {
                response
            }
        }
        Command::Config(ConfigCommand::Get { key }) => Response::new(profile.settings(key.clone())?),
        Command::Config(ConfigCommand::Set { key, value }) => {
            Response::new(profile.set_setting(key, value)?)
        }
        Command::Pair { code, local_only } => {
            return network::pair(profile, code.as_deref(), *local_only);
        }
        Command::Sync { what: None, local_only } => return network::sync_once(profile, *local_only),
        Command::Sync { what: Some(SyncCommand::Status), .. } => return network::status(profile),
        Command::Daemon(DaemonCommand::Install { local_only }) => {
            return service::install(profile, *local_only);
        }
        Command::Daemon(DaemonCommand::Uninstall) => return service::uninstall(profile),
        Command::Daemon(DaemonCommand::Start) => return service::start_or_stop(profile, true),
        Command::Daemon(DaemonCommand::Stop) => return service::start_or_stop(profile, false),
        Command::Daemon(DaemonCommand::Status) => return service::status(profile),
        Command::Device(DeviceCommand::List) => return network::list_devices(profile),
        Command::Device(DeviceCommand::Rename { device, name }) => {
            return network::rename_device(profile, device, name);
        }
        Command::Device(DeviceCommand::Unpair { device, yes }) => {
            let asked = profile.devices()?.devices.into_iter().find(|d| d.name.eq_ignore_ascii_case(device) || d.node_id.starts_with(device.as_str()));
            if !confirmed(asked.map(|d| d.actions), ActionKind::Unpair, *yes, false)? {
                return Ok(Response::unchanged("nothing was unpaired"));
            }
            return network::unpair_device(profile, device);
        }
        Command::Reminder(command) => match command {
            ReminderCommand::List { target } => Response::new(profile.reminders_of(reminder_target(profile, target)?)?),
            ReminderCommand::Add { target, at, before, after } => {
                let offset = match (before, after) {
                    (Some(span), _) => -i32::try_from(lumenna_host::parse_minutes(span.clone())?).unwrap_or(i32::MAX),
                    (_, Some(span)) => i32::try_from(lumenna_host::parse_minutes(span.clone())?).unwrap_or(i32::MAX),
                    (None, None) => 0,
                };
                let target = reminder_target(profile, target)?;
                // Not what it counts from: a moment of its own, read as the apps read it.
                if !matches!(at.trim(), "due" | "start" | "end") && offset == 0 {
                    Response::new(profile.add_reminder_saying(target, at.clone())?)
                } else {
                    Response::new(profile.add_reminder(target, at.clone(), offset)?)
                }
            }
            ReminderCommand::Rm { id } => Response::new(profile.remove_reminder(id.clone())?),
            ReminderCommand::Deliver { id, on } => Response::new(profile.deliver_reminder(id.clone(), on.clone())?),
            ReminderCommand::Upcoming { hours } => {
                let until = jiff::Timestamp::now()
                    .checked_add(jiff::SignedDuration::from_hours(i64::from(*hours)))
                    .unwrap_or_else(|_| jiff::Timestamp::now());
                Response::new(profile.upcoming_reminders(until.to_string())?)
            }
            ReminderCommand::Dismiss { key, occurrence } => Response::new(profile.dismiss_reminder(key.clone(), occurrence.clone())?),
            ReminderCommand::Snooze { key, occurrence, r#for } => {
                let minutes = r#for.as_ref().map(|span| lumenna_host::parse_minutes(span.clone())).transpose()?;
                Response::new(profile.snooze_reminder(key.clone(), occurrence.clone(), minutes)?)
            }
        },
        Command::Notify { speak } => return notify(profile, *speak),
        Command::History => Response::new(profile.history()?),
        Command::Undo => Response::new(profile.undo()?),
        Command::Redo => Response::new(profile.redo()?),
        Command::Backup { to } => {
            Response::new(profile.backup(to.as_ref().map(|p| p.display().to_string()))?)
        }
        Command::Restore { file } => Response::new(profile.restore(&file.display().to_string())?),
        Command::Export { format, output, force } => {
            return durability::export(profile, (*format).into(), output.as_deref(), *force);
        }
        Command::Import { file } => Response::new(profile.import(&file.display().to_string())?),
        Command::EraseDeleted { yes } => {
            let actions = lumenna_host::actions::store_actions();
            if !confirmed(Some(actions), ActionKind::EraseEverythingDeleted, *yes, true)? {
                return Ok(Response::unchanged("nothing was erased"));
            }
            Response::new(profile.erase_everything_deleted()?)
        }
        Command::Assist { list: true, .. } => assist::list(),
        Command::Assist { operation, given, args, .. } => {
            return assist::run(profile, operation.as_deref().unwrap_or_default(), given, args.as_deref());
        }
        Command::Complete { kind, text } => Response::new(profile.complete_typed((*kind).into(), text)?),
    })
}

/// Task commands, gathered under `lum task` the way blocks are under `lum block`.
fn task(profile: &Profile, command: &TaskCommand) -> Result<Response> {
    let id = |input: &str| profile.row(input, "task");
    Ok(match command {
        TaskCommand::Add { text, quiet } => {
            let response = Response::new(profile.add_task(&text.join(" "))?);
            if *quiet { response.quietly() } else { response }
        }
        TaskCommand::List { query } => Response::new(profile.list_tasks(&query.join(" "))?),
        TaskCommand::Show { id: input } => Response::new(profile.show_task(&id(input)?)?),
        TaskCommand::Edit { id: input, title, due, repeat, priority, estimate, notes, project, labels } => {
            Response::new(profile.edit_task(&id(input)?, TaskEdit {
                title: title.clone(),
                due: due.clone(),
                repeat: repeat.clone(),
                priority: *priority,
                estimate: estimate.clone(),
                notes: notes.clone(),
                project: project.clone(),
                labels: labels.as_ref().map(|l| {
                    l.split(',').map(str::trim).filter(|n| !n.is_empty()).map(ToOwned::to_owned).collect()
                }),
            })?)
        }
        TaskCommand::Done { id: input } => Response::new(profile.complete_task(&id(input)?)?),
        TaskCommand::Undone { id: input } => Response::new(profile.uncomplete_task(&id(input)?)?),
        TaskCommand::Rm { id: input } => {
            let mut change = profile.trash_task(&id(input)?)?;
            if profile.at_terminal() {
                change.announcement.push_str(" (recover it with `lum task restore`)");
            }
            Response::new(change)
        }
        TaskCommand::History { id: input } => Response::new(profile.task_history(&id(input)?)?),
        TaskCommand::Restore { id: input } => Response::new(profile.restore_task(&id(input)?)?),
        TaskCommand::Zone { id: input, zone } => Response::new(profile.set_task_zone(&id(input)?, Some(zone.clone()))?),
        TaskCommand::Erase { id: input, permanently: false, yes } => {
            let id = id(input)?;
            let actions = profile.show_task(&id)?.task.actions;
            if !confirmed(Some(actions), ActionKind::DeleteForGood, *yes, true)? {
                return Ok(Response::unchanged("nothing was deleted"));
            }
            Response::new(profile.erase_task(&id)?)
        }
        TaskCommand::Erase { id: input, permanently: true, yes } => {
            let id = id(input)?;
            let actions = profile.show_task(&id)?.task.actions;
            if !confirmed(Some(actions), ActionKind::ErasePermanently, *yes, true)? {
                return Ok(Response::unchanged("nothing was erased"));
            }
            Response::new(profile.erase_task_permanently(&id)?)
        }
        TaskCommand::Move { id: input, parent, project, top } => {
            let to = match (parent, project, top) {
                (Some(parent), _, _) => MoveTarget::Parent { id: id(parent)? },
                (_, Some(project), _) => MoveTarget::Project { name: project.clone() },
                (_, _, true) => MoveTarget::Top,
                _ => {
                    return Err(CliError::Message(
                        "say where: --parent, --project, or --top".to_owned(),
                    ));
                }
            };
            Response::new(profile.move_task(&id(input)?, to)?)
        }
        TaskCommand::Search { text } => Response::new(profile.search_tasks(&text.join(" "))?),
        TaskCommand::Depend(DependCommand::Add { id: input, on }) => {
            Response::new(profile.add_dependency(&id(input)?, &id(on)?)?)
        }
        TaskCommand::Depend(DependCommand::Rm { id: input, on }) => {
            Response::new(profile.remove_dependency(&id(input)?, &id(on)?)?)
        }
    })
}

fn project(profile: &Profile, command: &ProjectCommand) -> Result<Response> {
    Ok(Response::new(match command {
        ProjectCommand::Add { name, parent } => profile.add_project(name, parent.clone())?,
        ProjectCommand::List => return Ok(Response::new(profile.list_projects()?)),
        ProjectCommand::Rename { name, to } => profile.rename_project(name, to)?,
        ProjectCommand::Archive { name } => profile.archive_project(name)?,
        ProjectCommand::Move { name, parent, top } => {
            if parent.is_none() && !top {
                return Err(CliError::Message("say where: --parent <project>, or --top".to_owned()));
            }
            profile.move_project(name, parent.clone())?
        }
        ProjectCommand::Order { name, direction } => {
            profile.reorder_project(name, (*direction).into())?
        }
        ProjectCommand::Rm { name, keep_tasks } => profile.delete_project(name, *keep_tasks)?,
        ProjectCommand::Weight { name, value } => {
            profile.weigh_project(name, lumenna_host::parse_weight(value.clone())?)?
        }
        ProjectCommand::Colour { name, colour } => profile.recolour_project(name, colour_given(colour))?,
    }))
}

/// What a response puts in the last listing, if anything.
///
/// Only listings do: a mutation must not overwrite the numbering the user is working
/// against, or `lum task done 1` twice in a row would mean two different tasks.
fn listing_of(outcome: &Outcome) -> Option<Vec<(String, String)>> {
    match outcome {
        Outcome::Rows(rows) => {
            Some(rows.rows.iter().map(|row| (row.role.clone(), row.id.clone())).collect())
        }
        // Blocks and assignments are numbered within their own kinds but listed together,
        // which is what lets `lum start 1` work straight after `lum plan`.
        Outcome::Plan(plan) => Some(
            plan.blocks
                .iter()
                .flat_map(|block| {
                    std::iter::once(("block".to_owned(), block.id.clone())).chain(
                        block.assignments.iter().map(|a| ("assignment".to_owned(), a.id.clone())),
                    )
                })
                .collect(),
        ),
        Outcome::Change(_)
        | Outcome::Task(_)
        | Outcome::Block(_)
        | Outcome::Filters(_)
        | Outcome::Settings(_)
        | Outcome::Timer(_)
        | Outcome::Choices(_)
        | Outcome::History(_)
        | Outcome::Replan(_)
        | Outcome::DayPlan(_)
        | Outcome::Replanned(_)
        | Outcome::DayPlanned(_)
        | Outcome::DueReminders(_)
        | Outcome::Reminders(_)
        | Outcome::Server(_)
        | Outcome::Backup(_)
        | Outcome::Restore(_)
        | Outcome::Export(_)
        | Outcome::Import(_)
        | Outcome::Paired(_)
        | Outcome::Synced(_)
        | Outcome::SyncStatus(_)
        | Outcome::Devices(_)
        | Outcome::Operations(_)
        | Outcome::Completions(_) => None,
    }
}

/// Asks the question the core puts before the action of `kind` among `actions`, when it
/// asks one: the same words every client asks. `--yes` answers it; without a terminal to
/// ask at, a `strict` one is refused and any other goes ahead, as a script expects.
/// A colour named on the command line: `none` clears it.
fn colour_given(colour: &str) -> Option<String> {
    Some(colour.to_owned()).filter(|c| !c.eq_ignore_ascii_case("none"))
}

fn confirmed(actions: Option<Vec<lumenna_host::Action>>, kind: ActionKind, yes: bool, strict: bool) -> Result<bool> {
    use std::io::IsTerminal;
    let question = actions.into_iter().flatten().find(|a| a.kind == kind).map(|a| a.question);
    let Some(lumenna_host::Question::Confirm { title, message, .. }) = question else { return Ok(true) };
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        if strict {
            // The whole question, so a script's author learns what --yes agrees to.
            return Err(lumenna_host::LumennaError::new(format!("{title} {message} Nothing here can ask, so pass --yes")).into());
        }
        return Ok(true);
    }
    anstream::eprint!("{title} {message} Type yes to confirm: ");
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(answer.trim() == "yes")
}

/// The clock a proposal is said in: this device's `clock` setting.
fn time_style(profile: &Profile) -> lumenna_host::TimeStyle {
    match clock(profile) {
        render::Clock::TwelveHour => lumenna_host::TimeStyle::TwelveHour,
        render::Clock::TwentyFourHour => lumenna_host::TimeStyle::TwentyFourHour,
    }
}

/// The items of a proposal `--apply` names: `all`, or numbers as printed, such as 1,3.
fn numbered<T: Clone>(numbers: &str, items: Vec<T>) -> Result<Vec<T>> {
    if numbers == "all" {
        return Ok(items);
    }
    numbers
        .split(',')
        .map(|number| {
            let n: usize = number
                .trim()
                .parse()
                .map_err(|_| CliError::Message(format!("'{number}' is not the number of a change")))?;
            items.get(n.wrapping_sub(1)).cloned().ok_or_else(|| {
                CliError::Message(format!("there is no change {n}: {} proposed", items.len()))
            })
        })
        .collect()
}

/// A reminder's target from a row of the last listing, a task's or a block's.
fn reminder_target(profile: &Profile, input: &str) -> Result<String> {
    let task = profile.row(input, "task");
    match task {
        Ok(id) if id != input || input.parse::<usize>().is_err() => Ok(id),
        _ => profile.row(input, "block"),
    }
}

/// `lum notify`: says each reminder as it comes due, until stopped. Only one process on a
/// device gives them; while another does (a desktop app), this waits its turn.
fn notify(profile: &Profile, speak: bool) -> Result<Response> {
    use std::io::Write;
    let mut said_waiting = false;
    let lock = loop {
        if let Some(lock) = profile.take_reminders()? {
            break lock;
        }
        if !said_waiting {
            println!("Another app on this device gives reminders; waiting for it to stop.");
            said_waiting = true;
        }
        std::thread::sleep(std::time::Duration::from_secs(5));
    };
    println!("Saying reminders as they come due. Ctrl+C stops.");
    let say = |text: &str| {
        // The bell first, so a speakup or braille user knows a reminder arrived.
        print!("\u{7}");
        println!("{text}");
        let _ = std::io::stdout().flush();
        if speak {
            let _ = std::process::Command::new("spd-say").arg("--wait").arg(text).status();
        }
    };
    loop {
        let given = profile.deliver_reminders(lock.clone(), None)?;
        if !given.missed_summary.is_empty() {
            say(&given.missed_summary);
        }
        for notification in &given.due {
            say(&notification.text);
        }
        std::thread::sleep(std::time::Duration::from_secs(20));
    }
}
