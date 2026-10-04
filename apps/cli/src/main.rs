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
mod network;
mod profile;
mod render;
mod rpc;
mod service;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use lumenna_surface::{MoveTarget, NewBlock, TaskEdit, Weight};

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

    let profile = Profile::open(cli.profile.as_deref())?;
    if matches!(cli.command, Command::Rpc) {
        return rpc::serve(profile);
    }
    if let Command::SyncDaemon { local_only } = cli.command {
        durability::back_up_if_due(&profile);
        return network::daemon(&profile, local_only);
    }

    // §9: every client backs up opportunistically, because a schedule only exists where
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
    render::emit(&response, format);
    Ok(())
}

/// Runs one command and returns what it produced.
///
/// The one place a command becomes a [`Response`], so `lum rpc` reaches every operation the
/// command line does by building the same [`Command`] (§12). Each arm resolves row numbers —
/// the terminal's own affordance — and calls the surface, which does the rest.
pub(crate) fn dispatch(profile: &Profile, command: &Command) -> Result<Response> {
    Ok(match command {
        Command::Completions { .. } | Command::Rpc | Command::SyncDaemon { .. } => {
            unreachable!("handled before dispatch")
        }
        Command::Task(command) => return task(profile, command),
        Command::Project(command) => return project(profile, command),
        Command::Label(command) => Response::new(match command {
            LabelCommand::Add { name } => profile.add_label(name)?,
            LabelCommand::List => return Ok(Response::new(profile.list_labels()?)),
            LabelCommand::Rename { name, to } => profile.rename_label(name, to)?,
            LabelCommand::Merge { from, into } => profile.merge_labels(from, into)?,
            LabelCommand::Rm { name } => profile.delete_label(name)?,
        }),
        Command::Filter(command) => match command {
            FilterCommand::Add { name, query } => {
                Response::new(profile.add_filter(name, &query.join(" "))?)
            }
            FilterCommand::List => Response::new(profile.list_filters()?),
            FilterCommand::Rm { name } => Response::new(profile.delete_filter(name)?),
        },
        Command::Plan { date } => {
            let date = date.join(" ");
            Response::new(profile.plan(Some(date).filter(|d| !d.trim().is_empty()))?)
        }
        Command::Block(command) => match command {
            BlockCommand::Add { title, at, minutes, date, kind, repeat } => {
                Response::new(profile.add_block(NewBlock {
                    title: title.clone(),
                    at: at.clone(),
                    minutes: *minutes,
                    date: date.clone(),
                    kind: kind.clone(),
                    repeat: repeat.clone(),
                })?)
            }
            BlockCommand::List => Response::new(profile.list_blocks()?),
            BlockCommand::Rm { id } => {
                Response::new(profile.delete_block(&profile.row(id, "block")?)?)
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
        Command::Start { assignment } => {
            Response::new(profile.start_timer(&profile.row(assignment, "assignment")?)?)
        }
        Command::Stop { assignment, minutes } => {
            let timer = profile.stop_timer(&profile.row(assignment, "assignment")?, *minutes)?;
            let capped = timer.changed && timer.capped;
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
        Command::Device(DeviceCommand::Unpair { device }) => {
            return network::unpair_device(profile, device);
        }
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
        TaskCommand::Edit { id: input, title, due, priority, estimate, notes, project } => {
            Response::new(profile.edit_task(&id(input)?, TaskEdit {
                title: title.clone(),
                due: due.clone(),
                priority: *priority,
                estimate: estimate.clone(),
                notes: notes.clone(),
                project: project.clone(),
            })?)
        }
        TaskCommand::Done { id: input } => Response::new(profile.complete_task(&id(input)?)?),
        TaskCommand::Undone { id: input } => Response::new(profile.uncomplete_task(&id(input)?)?),
        TaskCommand::Rm { id: input } => {
            let mut change = profile.trash_task(&id(input)?)?;
            change.announcement.push_str(" (recover it with `lum task restore`)");
            Response::new(change)
        }
        TaskCommand::Restore { id: input } => Response::new(profile.restore_task(&id(input)?)?),
        TaskCommand::Erase { id: input, yes } => {
            let id = id(input)?;
            if !yes {
                let title = profile.show_task(&id)?.task.title;
                anstream::eprint!(
                    "Permanently delete '{title}' and its history? Type yes to confirm: "
                );
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                if answer.trim() != "yes" {
                    return Ok(Response::unchanged("nothing was deleted"));
                }
            }
            Response::new(profile.erase_task(&id)?)
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
        ProjectCommand::Rm { name, keep_tasks } => profile.delete_project(name, *keep_tasks)?,
        ProjectCommand::Weight { name, value } => {
            let weight = if value.eq_ignore_ascii_case("inherit") {
                Weight::Inherit
            } else {
                let value = value.parse::<f32>().ok().filter(|v| v.is_finite() && *v > 0.0);
                Weight::Value {
                    value: value.ok_or_else(|| {
                        CliError::Message(
                            "a weight has to be a positive number, or `inherit`".to_owned(),
                        )
                    })?,
                }
            };
            profile.weigh_project(name, weight)?
        }
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
        // which is what lets `lum start 1` work straight after `lum plan` (§15).
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
        | Outcome::Filters(_)
        | Outcome::Settings(_)
        | Outcome::Timer(_)
        | Outcome::Completions(_)
        | Outcome::Preview(_)
        | Outcome::Server(_)
        | Outcome::Backup(_)
        | Outcome::Restore(_)
        | Outcome::Export(_)
        | Outcome::Import(_)
        | Outcome::Paired(_)
        | Outcome::Synced(_)
        | Outcome::SyncStatus(_)
        | Outcome::Devices(_) => None,
    }
}
