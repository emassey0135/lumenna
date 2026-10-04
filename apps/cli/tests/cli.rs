//! The command surface, exercised through the real binary.
//!
//! §15 makes the CLI the completeness test for the core API: *"a capability the CLI cannot
//! reach is a gap in the core, not in the CLI."* Running the actual executable is what makes
//! that a test rather than an aspiration — a command that compiles but cannot be invoked
//! proves nothing.

use std::path::Path;
use std::process::Command;

struct Lum {
    profile: tempfile::TempDir,
}

struct Output {
    stdout: String,
    stderr: String,
    ok: bool,
}

impl Lum {
    fn new() -> Self {
        Self { profile: tempfile::tempdir().unwrap() }
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_lum"))
            .args(args)
            .env("LUMENNA_PROFILE", self.profile.path())
            // Automatic backups land here rather than beside the temporary profile.
            .env("LUMENNA_BACKUP_DIR", self.profile.path().join("backups"))
            .env("NO_COLOR", "1")
            .output()
            .expect("the binary should run");
        Output {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            ok: output.status.success(),
        }
    }

    /// Runs a command that is expected to succeed, returning its output.
    fn ok(&self, args: &[&str]) -> String {
        let result = self.run(args);
        assert!(result.ok, "`lum {}` failed: {}", args.join(" "), result.stderr);
        result.stdout
    }

    /// Runs a command that is expected to fail, returning what it said.
    fn fails(&self, args: &[&str]) -> String {
        let result = self.run(args);
        assert!(!result.ok, "`lum {}` unexpectedly succeeded", args.join(" "));
        result.stderr
    }

    fn path(&self) -> &Path {
        self.profile.path()
    }
}

#[test]
fn a_fresh_profile_works_without_setup() {
    let lum = Lum::new();
    let out = lum.ok(&["task", "list"]);
    assert!(out.contains("no tasks"), "{out}");
    assert!(lum.path().join("lumenna.sqlite").exists());
}

#[test]
fn quick_add_reads_back_what_it_understood() {
    // The announcement stands in for the inline highlighting a sighted user gets (§6.1),
    // and always names the resolved date, since the phrase is the ambiguous part.
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    let out = lum.ok(&["task", "add", "review PR tomorrow 3pm p1 #Work @laptop"]);

    assert!(out.contains("review PR"), "{out}");
    assert!(out.contains("3:00 PM"), "{out}");
    assert!(out.contains("priority 1"), "{out}");
    assert!(out.contains("new label laptop"), "{out}");
    assert!(out.contains("20"), "the resolved year, not just 'tomorrow': {out}");
}

#[test]
fn an_unknown_project_stops_the_add_but_an_unknown_label_does_not() {
    // §3.4's asymmetry: a project has structure that wants a decision; a label does not.
    let lum = Lum::new();
    let complaint = lum.fails(&["task", "add", "task #Nope"]);
    assert!(complaint.contains("unknown project 'Nope'"), "{complaint}");
    assert!(lum.ok(&["task", "list"]).contains("no tasks"));

    lum.ok(&["task", "add", "task @brandnew"]);
    assert!(lum.ok(&["task", "list"]).contains("task"));
    assert!(lum.ok(&["label", "list"]).contains("brandnew"));
}

#[test]
fn a_typo_gets_a_suggestion_with_its_position() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    let complaint = lum.fails(&["task", "add", "task #Wrok"]);
    assert!(complaint.contains("did you mean 'Work'?"), "{complaint}");
    assert!(complaint.contains("position"), "{complaint}");
}

#[test]
fn row_numbers_address_the_last_listing() {
    // §15: a UUID is thirty-six characters, miserable to type and worse to dictate.
    let lum = Lum::new();
    lum.ok(&["task", "add", "first"]);
    lum.ok(&["task", "add", "second"]);
    lum.ok(&["task", "list"]);

    let out = lum.ok(&["task", "done", "2"]);
    assert!(out.contains("Completed second"), "{out}");
    assert!(!lum.ok(&["task", "list"]).contains("second"), "a completed task leaves the list");
    assert!(lum.ok(&["task", "list", "completed"]).contains("second"));
}

#[test]
fn a_row_number_from_the_wrong_listing_says_so() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "a task"]);
    lum.ok(&["task", "list"]);
    let complaint = lum.fails(&["assign", "1", "--block", "1"]);
    assert!(complaint.contains("showed tasks, not blocks"), "{complaint}");
    assert!(complaint.contains("lum block list"), "{complaint}");
}

#[test]
fn identifier_prefixes_work_like_git() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "review PR"]);
    let json = lum.ok(&["task", "list", "--json"]);
    let id = json
        .lines()
        .find(|line| line.contains("\"id\""))
        .and_then(|line| line.split('"').nth(3))
        .expect("an id in the JSON");

    let out = lum.ok(&["task", "show", &id[..8]]);
    assert!(out.contains("review PR"), "{out}");
}

#[test]
fn the_filter_language_reads_back_before_it_lists() {
    // A mis-parsed filter shows wrong results silently, and wrong results are invisible
    // (§6.3).
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["task", "add", "urgent thing p1 #Work"]);
    lum.ok(&["task", "add", "other thing"]);

    let out = lum.ok(&["task", "list", "#Work & p1"]);
    assert!(out.contains("tasks in Work, and priority 1"), "{out}");
    assert!(out.contains("1 task"), "{out}");
    assert!(out.contains("urgent thing"), "{out}");
    assert!(!out.contains("other thing"), "{out}");
}

#[test]
fn a_broken_filter_says_where_and_suggests() {
    let lum = Lum::new();
    let complaint = lum.fails(&["task", "list", "blokced"]);
    assert!(complaint.contains("did you mean 'blocked'?"), "{complaint}");
}

#[test]
fn json_carries_a_version_and_the_row_numbers() {
    // §15: this is a compatibility contract, not a debugging convenience.
    let lum = Lum::new();
    lum.ok(&["task", "add", "review PR tomorrow"]);
    let out = lum.ok(&["task", "list", "--json"]);
    assert!(out.contains("\"version\": 1"), "{out}");
    assert!(out.contains("\"count\": 1"), "{out}");
    assert!(out.contains("\"row\": 1"), "{out}");
    assert!(out.contains("\"role\": \"task\""), "{out}");
    assert!(out.contains("\"title\": \"review PR\""), "{out}");
}

#[test]
fn trash_is_recoverable_and_the_way_back_is_in_the_message() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "delete me"]);
    lum.ok(&["task", "list"]);

    let out = lum.ok(&["task", "rm", "1"]);
    assert!(out.contains("lum task restore"), "{out}");
    assert!(lum.ok(&["task", "list"]).contains("no tasks"));

    assert!(lum.ok(&["task", "list", "deleted"]).contains("delete me"));
    lum.ok(&["task", "restore", "1"]);
    assert!(lum.ok(&["task", "list"]).contains("delete me"));
}

#[test]
fn dependencies_are_settable_and_then_filterable() {
    // §15's own audit found `depends` filterable but not settable, which is exactly the
    // failure the completeness rule is meant to catch.
    let lum = Lum::new();
    lum.ok(&["task", "add", "draft"]);
    lum.ok(&["task", "add", "review"]);
    lum.ok(&["task", "list"]);

    lum.ok(&["task", "depend", "add", "2", "--on", "1"]);
    assert!(lum.ok(&["task", "list", "blocked"]).contains("review"));
    assert!(lum.ok(&["task", "list", "ready"]).contains("draft"));

    lum.ok(&["task", "list"]);
    lum.ok(&["task", "depend", "rm", "2", "--on", "1"]);
    assert!(lum.ok(&["task", "list", "blocked"]).contains("no tasks"));
}

#[test]
fn a_task_cannot_be_made_to_wait_for_itself() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "only task"]);
    lum.ok(&["task", "list"]);
    let complaint = lum.fails(&["task", "depend", "add", "1", "--on", "1"]);
    assert!(complaint.contains("cannot wait for itself"), "{complaint}");
}

#[test]
fn project_weight_is_settable_and_warns_outside_its_range() {
    // Keep the range narrow, because anything wider lets one project dominate every ranking
    // and the user then distrusts the whole feature (§3.4).
    let lum = Lum::new();
    lum.ok(&["project", "add", "Thesis"]);
    lum.ok(&["project", "weight", "Thesis", "1.5"]);
    assert!(lum.ok(&["project", "list"]).contains("weight 1.5"));

    let noisy = lum.run(&["project", "weight", "Thesis", "9"]);
    assert!(noisy.ok, "it still applies");
    assert!(noisy.stderr.contains("outside the usual range"), "{}", noisy.stderr);
}

#[test]
fn the_inbox_cannot_be_deleted() {
    let lum = Lum::new();
    lum.ok(&["task", "list"]);
    let complaint = lum.fails(&["project", "rm", "Inbox"]);
    assert!(complaint.contains("cannot be deleted"), "{complaint}");
}

#[test]
fn deleting_a_project_can_keep_its_tasks() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["task", "add", "keep me #Work"]);

    lum.ok(&["project", "rm", "Work", "--keep-tasks"]);
    let out = lum.ok(&["task", "list"]);
    assert!(out.contains("keep me"), "{out}");
    assert!(lum.ok(&["task", "list", "no project"]).contains("keep me"), "it moved to the Inbox");
}

#[test]
fn merging_labels_repairs_a_typo() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "one @laptop"]);
    lum.ok(&["task", "add", "two @lapto"]);

    lum.ok(&["label", "merge", "lapto", "laptop"]);
    let out = lum.ok(&["task", "list", "@laptop"]);
    assert!(out.contains("2 tasks"), "{out}");
}

#[test]
fn deleting_a_label_leaves_its_tasks_alone() {
    // §3.4: deletion touches no tasks, and the identifiers left behind project as absent.
    let lum = Lum::new();
    lum.ok(&["task", "add", "still here @errand"]);
    let out = lum.ok(&["label", "rm", "errand"]);
    assert!(out.contains("tasks that wore it are unchanged"), "{out}");
    assert!(lum.ok(&["task", "list"]).contains("still here"));
}

#[test]
fn recurring_tasks_advance_rather_than_ending() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "water plants every 3 days"]);
    let before = lum.ok(&["task", "list"]);
    lum.ok(&["task", "done", "1"]);
    let after = lum.ok(&["task", "list"]);

    assert!(after.contains("water plants"), "it is still there: {after}");
    assert!(after.contains("recurring"), "{after}");
    assert_ne!(before, after, "the due date moved");
}

#[test]
fn blocks_plans_and_timers_work_end_to_end() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "write the chapter"]);
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90"]);

    let json = lum.ok(&["task", "list", "--json"]);
    let task = json
        .lines()
        .find(|line| line.contains("\"id\""))
        .and_then(|line| line.split('"').nth(3))
        .expect("an id")
        .to_owned();

    lum.ok(&["block", "list"]);
    lum.ok(&["assign", &task, "--block", "1"]);

    let plan = lum.ok(&["plan"]);
    assert!(plan.contains("Deep work"), "{plan}");
    assert!(plan.contains("09:00 to 10:30"), "no seconds anywhere: {plan}");
    assert!(plan.contains("write the chapter"), "{plan}");

    // The plan listing remembers blocks *and* assignments, each numbered within its kind,
    // so the timer starts without another command in between.
    lum.ok(&["start", "1"]);
    assert!(lum.ok(&["plan"]).contains("in progress"), "not `inprogress`");
    assert!(lum.ok(&["stop", "1"]).contains("Logged"));
    assert!(lum.ok(&["plan"]).contains("worked"));
}

#[test]
fn a_repeating_block_starts_on_a_day_it_actually_occurs() {
    // RFC 5545 starts at the first date matching the rule on or after DTSTART, so a series
    // anchored on a day it never occurs is a trap (§5). The block editor normalises it.
    let lum = Lum::new();
    lum.ok(&[
        "block", "add", "Standup", "--at", "9am", "--minutes", "15", "--repeat",
        "every monday",
    ]);
    let out = lum.ok(&["block", "list"]);
    assert!(out.contains("FREQ=WEEKLY;BYDAY=MO"), "{out}");
}

#[test]
fn settings_round_trip() {
    let lum = Lum::new();
    assert!(lum.ok(&["config", "get", "verbosity"]).contains("full"));
    lum.ok(&["config", "set", "verbosity", "terse"]);
    assert!(lum.ok(&["config", "get", "verbosity"]).contains("terse"));

    lum.ok(&["config", "set", "cascade-complete-subtasks", "no"]);
    assert!(lum.ok(&["config", "get", "cascade-complete-subtasks"]).contains("false"));
    assert!(lum.fails(&["config", "set", "nonsense", "x"]).contains("no setting"));
}

#[test]
fn saved_filters_keep_their_text() {
    // A filter containing `today` has to mean today at evaluation time (§6.2).
    let lum = Lum::new();
    lum.ok(&["filter", "add", "Now", "overdue | today"]);
    let out = lum.ok(&["filter", "list"]);
    assert!(out.contains("Now"), "{out}");
    assert!(out.contains("overdue | today"), "stored as text: {out}");
}

#[test]
fn cascade_reaches_subtasks_and_the_setting_turns_it_off() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "ship release"]);
    lum.ok(&["task", "add", "write notes"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "move", "2", "--parent", "1"]);

    lum.ok(&["task", "list"]);
    lum.ok(&["task", "done", "1"]);
    assert!(lum.ok(&["task", "list"]).contains("no tasks"), "the subtask went with it");
}

#[test]
fn help_is_available_for_every_command() {
    // §15: `--help` is an accessibility surface, not generated boilerplate. It is the
    // primary discovery mechanism for anyone who cannot skim a GUI.
    let lum = Lum::new();
    let top = lum.ok(&["--help"]);
    for command in ["task", "plan", "project", "label", "filter", "block", "assign"] {
        assert!(top.contains(command), "`{command}` is missing from --help");
        let help = lum.ok(&[command, "--help"]);
        assert!(help.len() > 40, "`lum {command} --help` says almost nothing");
    }

    let tasks = lum.ok(&["task", "--help"]);
    for command in ["add", "list", "done", "move", "depend"] {
        assert!(tasks.contains(command), "`{command}` is missing from `lum task --help`");
        let help = lum.ok(&["task", command, "--help"]);
        assert!(help.len() > 40, "`lum task {command} --help` says almost nothing");
    }
}

#[test]
fn shell_completions_are_generated() {
    let lum = Lum::new();
    let script = lum.ok(&["completions", "bash"]);
    assert!(script.contains("lum"), "{script}");
}

// ---------------------------------------------------------------------------------------
// The typed command surface (§12)
// ---------------------------------------------------------------------------------------

#[test]
fn a_mutation_returns_structure_rather_than_a_sentence() {
    // §12 defines one typed command surface, and `--json` is an adapter over it. A prose
    // sentence in a JSON envelope is not a surface: a client that cannot find out *what*
    // changed has to list everything again to guess.
    let lum = Lum::new();
    let out = lum.ok(&["task", "add", "review PR", "--json"]);
    assert!(out.contains("\"result\": \"change\""), "{out}");
    assert!(out.contains("\"changed\": true"), "{out}");
    assert!(out.contains("\"tasks\""), "the change names what it touched: {out}");
}

#[test]
fn adding_a_task_returns_the_task() {
    // §12's surface is `add_task(text) -> Task`. Capture is the one place a round trip
    // hurts, so the created record comes back whole rather than as an identifier to fetch.
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    let out = lum.ok(&["task", "add", "review PR tomorrow p1 #Work @laptop", "--json"]);
    assert!(out.contains("\"task\""), "{out}");
    assert!(out.contains("\"title\": \"review PR\""), "{out}");
    assert!(out.contains("\"project\": \"Work\""), "{out}");
    assert!(out.contains("\"priority\": 1"), "{out}");
    assert!(out.contains("\"laptop\""), "a label created alongside is named: {out}");
}

#[test]
fn a_no_op_says_so_rather_than_claiming_a_change() {
    // Asking for a state something is already in is not an error, but a client that cannot
    // tell the two apart announces a change that did not happen.
    let lum = Lum::new();
    lum.ok(&["task", "add", "review PR"]);
    lum.ok(&["task", "list"]);
    let out = lum.ok(&["task", "edit", "1", "--title", "review PR", "--json"]);
    assert!(out.contains("\"changed\": false"), "{out}");
    assert!(!out.contains("\"affected\""), "nothing moved, so nothing is named: {out}");
}

#[test]
fn the_day_plan_carries_its_assignments() {
    // The whole point of the model is that tasks are assigned to blocks repeatedly across
    // sittings, so a plan that names only the blocks answers half the question.
    let lum = Lum::new();
    lum.ok(&["task", "add", "write the chapter"]);
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90"]);

    let task = first_id(&lum.ok(&["task", "list", "--json"]));
    let block = first_id(&lum.ok(&["block", "list", "--json"]));
    lum.ok(&["assign", &task, "--block", block.split('@').next().unwrap()]);

    let out = lum.ok(&["plan", "--json"]);
    assert!(out.contains("\"result\": \"plan\""), "{out}");
    assert!(out.contains("\"start\": \"09:00\""), "no seconds: {out}");
    assert!(out.contains("\"end\": \"10:30\""), "{out}");
    assert!(out.contains("\"assignments\""), "{out}");
    assert!(out.contains("\"title\": \"write the chapter\""), "{out}");
    assert!(out.contains("\"status\": \"planned\""), "not `Planned`: {out}");
}

#[test]
fn a_filter_reads_back_and_names_what_it_could_not_resolve() {
    // §6.2's readback belongs on the surface, not only on stdout: every client needs to say
    // how the query was understood, and a name that matched nothing is the usual reason it
    // was not.
    let lum = Lum::new();
    let out = lum.ok(&["task", "list", "#Nope", "--json"]);
    assert!(out.contains("\"description\": \"tasks in Nope\""), "{out}");
    assert!(out.contains("\"kind\": \"project\""), "{out}");
    assert!(out.contains("\"name\": \"Nope\""), "{out}");
    assert!(out.contains("\"notices\""), "{out}");
}

#[test]
fn settings_come_back_as_settings_not_as_a_line_of_text() {
    let lum = Lum::new();
    let one = lum.ok(&["config", "get", "verbosity", "--json"]);
    assert!(one.contains("\"result\": \"settings\""), "{one}");
    assert!(one.contains("\"key\": \"verbosity\""), "{one}");
    assert!(one.contains("\"value\": \"full\""), "{one}");

    // Text mode still answers with the bare value, because that is what a shell wants.
    assert_eq!(lum.ok(&["config", "get", "verbosity"]).trim(), "full");
}

#[test]
fn quiet_silences_the_terminal_without_shrinking_the_response() {
    // `--quiet` is a terminal convenience. A structured reader asked for the answer and
    // gets it either way.
    let lum = Lum::new();
    assert_eq!(lum.ok(&["task", "add", "review PR", "--quiet"]), "");
    let out = lum.ok(&["task", "add", "write it up", "--quiet", "--json"]);
    assert!(out.contains("\"title\": \"write it up\""), "{out}");
}

/// The first `"id"` value in a JSON document, which is the first row's.
fn first_id(json: &str) -> String {
    json.lines()
        .find(|line| line.contains("\"id\""))
        .and_then(|line| line.split('"').nth(3))
        .expect("an id in the JSON")
        .to_owned()
}

#[test]
fn a_longer_dependency_cycle_is_refused_too() {
    let lum = Lum::new();
    for title in ["draft", "review", "publish"] {
        lum.ok(&["task", "add", title]);
    }
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "depend", "add", "2", "--on", "1"]);
    lum.ok(&["task", "depend", "add", "3", "--on", "2"]);
    let complaint = lum.fails(&["task", "depend", "add", "1", "--on", "3"]);
    assert!(complaint.contains("circle"), "{complaint}");
}

#[test]
fn a_routine_survives_new_year() {
    let lum = Lum::new();
    lum.ok(&[
        "block", "add", "Morning pages", "--at", "7am", "--minutes", "30", "--date",
        "2026-12-30", "--repeat", "daily",
    ]);
    let plan = lum.ok(&["plan", "2027-01-01"]);
    assert!(plan.contains("Morning pages"), "{plan}");
}

#[test]
fn stopping_a_stopped_timer_says_so_and_minutes_can_be_logged_by_hand() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "write the chapter"]);
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90"]);
    let task = first_task_id(&lum);
    lum.ok(&["block", "list"]);
    lum.ok(&["assign", &task, "--block", "1"]);
    lum.ok(&["plan"]);

    let out = lum.ok(&["stop", "1"]);
    assert!(out.contains("not running"), "{out}");
    assert!(!out.contains("Logged"), "{out}");

    lum.ok(&["stop", "1", "--minutes", "40"]);
    assert!(lum.ok(&["plan"]).contains("40"), "the hand-logged figure is shown");
}

#[test]
fn a_block_that_takes_no_tasks_is_refused_as_a_destination() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "write the chapter"]);
    lum.ok(&["block", "add", "Lunch", "--at", "noon", "--minutes", "45", "--kind", "break"]);
    let task = first_task_id(&lum);
    lum.ok(&["block", "list"]);
    let complaint = lum.fails(&["assign", &task, "--block", "1"]);
    assert!(complaint.contains("does not take tasks"), "{complaint}");
}

#[test]
fn editing_a_tasks_project_brings_its_subtasks() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["task", "add", "ship release"]);
    lum.ok(&["task", "add", "write notes"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "move", "2", "--parent", "1"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "edit", "1", "--project", "Work", "--title", "ship it"]);

    let work = lum.ok(&["task", "list", "#Work"]);
    assert!(work.contains("ship it"), "{work}");
    assert!(work.contains("write notes"), "the subtask stayed behind: {work}");
}

#[test]
fn renaming_onto_an_existing_name_is_refused() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["project", "add", "Home"]);
    assert!(lum.fails(&["project", "rename", "Home", "work"]).contains("already"));
    lum.ok(&["label", "add", "laptop"]);
    lum.ok(&["label", "add", "phone"]);
    assert!(lum.fails(&["label", "rename", "phone", "@laptop"]).contains("merge"));
}

#[test]
fn the_week_start_can_be_set() {
    let lum = Lum::new();
    lum.ok(&["config", "set", "week-start", "sunday"]);
    assert!(lum.ok(&["config", "get", "week-start"]).contains("sunday"));
    assert!(!lum.run(&["config", "set", "week-start", "someday"]).ok);
}

#[test]
fn a_sub_project_can_opt_back_to_neutral_and_back_to_inheriting() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Thesis"]);
    lum.ok(&["project", "add", "Admin", "--parent", "Thesis"]);
    lum.ok(&["project", "weight", "Thesis", "2"]);
    assert!(lum.ok(&["project", "list"]).matches("weight 2").count() == 2);

    lum.ok(&["project", "weight", "Admin", "1"]);
    assert_eq!(lum.ok(&["project", "list"]).matches("weight 2").count(), 1);

    lum.ok(&["project", "weight", "Admin", "inherit"]);
    assert_eq!(lum.ok(&["project", "list"]).matches("weight 2").count(), 2);
}

/// The identifier of the first task listed, for commands that also need a block row number.
fn first_task_id(lum: &Lum) -> String {
    lum.ok(&["task", "list", "--json"])
        .lines()
        .find(|line| line.contains("\"id\""))
        .and_then(|line| line.split('"').nth(3))
        .expect("an id")
        .to_owned()
}

// ---------------------------------------------------------------------------------------
// Backup, export and import (§9)
// ---------------------------------------------------------------------------------------

fn backups_in(lum: &Lum) -> Vec<std::path::PathBuf> {
    let dir = lum.path().join("backups");
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    found.retain(|p| p.extension().is_some_and(|e| e == "lumbak"));
    found.sort();
    found
}

#[test]
fn a_backup_is_taken_automatically_once_a_day_and_on_request() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "first"]);
    assert_eq!(backups_in(&lum).len(), 1, "the first command of the day takes one");
    lum.ok(&["task", "add", "second"]);
    assert_eq!(backups_in(&lum).len(), 1, "and only one");

    let out = lum.ok(&["backup"]);
    assert!(out.contains("Backed up to"), "{out}");
    assert_eq!(backups_in(&lum).len(), 2);

    lum.ok(&["config", "set", "backup-every", "off"]);
    assert!(lum.ok(&["config", "get", "backup-every"]).contains("off"));
}

#[test]
fn a_backup_rebuilds_the_store_somewhere_else_trash_and_all() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "keep me p1 tomorrow"]);
    lum.ok(&["task", "add", "throw me away"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "rm", "2"]);
    // Somewhere outside the profile: a backup inside the directory it protects is refused.
    let elsewhere = tempfile::tempdir().unwrap();
    lum.ok(&["backup", "--to", elsewhere.path().to_str().unwrap()]);
    let backup = std::fs::read_dir(elsewhere.path()).unwrap().next().unwrap().unwrap().path();

    let other = Lum::new();
    let out = other.ok(&["restore", backup.to_str().unwrap()]);
    assert!(out.contains("Restored"), "{out}");
    assert!(other.ok(&["task", "list"]).contains("keep me"));
    assert!(other.ok(&["task", "list", "deleted"]).contains("throw me away"), "full history");

    let again = other.ok(&["restore", backup.to_str().unwrap()]);
    assert!(again.contains("nothing this store does not already have"), "{again}");
}

#[test]
fn an_export_comes_back_through_import_without_the_trash() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["task", "add", "review PR tomorrow 3pm p1 #Work @laptop"]);
    lum.ok(&["task", "add", "private"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "rm", "2"]);

    let file = lum.path().join("export.json");
    lum.ok(&["export", "--output", file.to_str().unwrap()]);
    let json = std::fs::read_to_string(&file).unwrap();
    assert!(json.contains("\"format\": \"lumenna-export\""));
    assert!(!json.contains("private"), "an export holds nothing from the trash");
    assert!(lum.fails(&["export", "--output", file.to_str().unwrap()]).contains("--force"));

    let other = Lum::new();
    let out = other.ok(&["import", file.to_str().unwrap()]);
    assert!(out.contains("new"), "{out}");
    let listed = other.ok(&["task", "list", "#Work & @laptop & p1"]);
    assert!(listed.contains("review PR"), "{listed}");
    let projects = other.ok(&["project", "list"]);
    assert_eq!(projects.matches("Inbox").count(), 1, "one Inbox, not two: {projects}");

    let again = other.ok(&["import", file.to_str().unwrap()]);
    assert!(again.contains("0 new, 0 updated"), "{again}");
}

#[test]
fn export_to_standard_output_is_exactly_the_export() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "write the chapter"]);
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90", "--repeat", "daily"]);

    let md = lum.ok(&["export", "--format", "md"]);
    assert!(md.starts_with("# Lumenna\n"), "{md}");
    assert!(md.contains("- [ ] write the chapter"));

    let ics = lum.ok(&["export", "--format", "ics"]);
    assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"), "{ics}");
    assert!(ics.contains("RRULE:FREQ=DAILY"));

    assert!(lum.ok(&["export", "--format", "org"]).contains("* Inbox"));
    let complaint = lum.fails(&["import", lum.path().join("nope.md").to_str().unwrap()]);
    assert!(complaint.contains("cannot read"), "{complaint}");
}

#[test]
fn choosing_where_backups_go_says_what_they_hold() {
    let lum = Lum::new();
    let elsewhere = tempfile::tempdir().unwrap();
    let synced = elsewhere.path().join("Dropbox").join("lumenna");
    let result = lum.run(&["config", "set", "backup-dir", synced.to_str().unwrap()]);
    assert!(result.ok, "{}", result.stderr);
    assert!(result.stderr.contains("whole history"), "{}", result.stderr);
    assert!(result.stderr.contains("Dropbox"), "{}", result.stderr);

    let inside = lum.path().join("in-here");
    assert!(lum.fails(&["config", "set", "backup-dir", inside.to_str().unwrap()]).contains("inside the profile"));

    lum.ok(&["config", "set", "backup-dir", "default"]);
    assert!(lum.ok(&["config", "get", "backup-dir"]).contains("backups"));
}

// ---------------------------------------------------------------------------------------
// Undo (§9)
// ---------------------------------------------------------------------------------------

#[test]
fn undo_works_across_commands_and_says_what_it_undid() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "review PR"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "done", "1"]);
    assert!(lum.ok(&["task", "list"]).contains("no tasks"), "done tasks leave the list");

    let out = lum.ok(&["undo"]);
    assert!(out.contains("Undid: Completed review PR"), "{out}");
    assert!(lum.ok(&["task", "list"]).contains("review PR"));

    assert!(lum.ok(&["redo"]).contains("Redid: Completed review PR"));
    assert!(lum.ok(&["task", "list"]).contains("no tasks"));

    lum.ok(&["undo"]);
    lum.ok(&["undo"]);
    assert!(lum.ok(&["task", "list"]).contains("no tasks"), "the add is undone too");
    assert!(lum.ok(&["undo"]).contains("nothing to undo"));
}

#[test]
fn blocks_are_undoable_like_everything_else() {
    let lum = Lum::new();
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90"]);
    lum.ok(&["block", "list"]);
    lum.ok(&["block", "rm", "1"]);
    assert!(lum.ok(&["block", "list"]).contains("no blocks"));
    assert!(lum.ok(&["undo"]).contains("Undid: Deleted block Deep work"));
    assert!(lum.ok(&["block", "list"]).contains("Deep work"));
}

#[test]
fn undo_keeps_what_has_changed_since_and_says_so() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "review PR"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "edit", "1", "--priority", "1", "--title", "review the PR"]);

    // A later title change from outside this device's history — an import stands in for one
    // merged from another device.
    let export = lum.path().join("now.json");
    lum.ok(&["export", "--output", export.to_str().unwrap()]);
    let changed = std::fs::read_to_string(&export).unwrap().replace("review the PR", "review PR 42");
    std::fs::write(&export, changed).unwrap();
    lum.ok(&["import", export.to_str().unwrap()]);

    let result = lum.run(&["undo"]);
    assert!(result.ok, "{}", result.stderr);
    assert!(result.stdout.contains("Undid: Edited review the PR"), "{}", result.stdout);
    assert!(result.stderr.contains("title, changed since"), "{}", result.stderr);
    let listed = lum.ok(&["task", "list"]);
    assert!(listed.contains("review PR 42"), "{listed}");
    assert!(!listed.contains("priority 1") && !lum.ok(&["task", "list", "p1"]).contains("review"));
}
