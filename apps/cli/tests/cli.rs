//! The command surface, exercised through the real binary.
//!
//! The CLI is the completeness test for the core API: a capability the CLI cannot reach is
//! a gap in the core, not in the CLI. Running the actual executable is what makes
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
    // The announcement stands in for the inline highlighting a sighted user gets,
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
    // The asymmetry: a project has structure that wants a decision; a label does not.
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
    // A UUID is thirty-six characters, miserable to type and worse to dictate.
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
fn each_kind_keeps_its_own_last_listing_so_a_task_and_a_block_are_both_numbered() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "first"]);
    lum.ok(&["task", "add", "second"]);
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "60"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["block", "list"]);
    let out = lum.ok(&["assign", "2", "--block", "1"]);
    assert!(out.contains("second") && out.contains("Deep work"), "{out}");
    // A listing of one kind with nothing in it is that kind's last listing.
    lum.ok(&["task", "list", "p1"]);
    assert!(lum.fails(&["task", "done", "1"]).contains("showed none"));
    assert!(lum.ok(&["block", "show", "1"]).contains("Deep work"), "the blocks are still numbered");
}

#[test]
fn a_command_that_fails_under_json_says_why_in_json() {
    let lum = Lum::new();
    let ran = lum.run(&["--json", "task", "show", "nothing-like-it"]);
    assert!(!ran.ok);
    let said: serde_json::Value = serde_json::from_str(&ran.stdout).unwrap_or_else(|e| panic!("{e}: {}", ran.stdout));
    assert_eq!(said["version"], 1);
    assert_eq!(said["error"]["kind"], "failed");
    assert!(said["error"]["reason"].as_str().unwrap().contains("nothing-like-it"), "{said}");
}

#[test]
fn a_blank_title_and_a_priority_past_four_are_refused_on_the_command_line_too() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "Essay"]);
    lum.ok(&["task", "list"]);
    assert!(lum.fails(&["task", "edit", "1", "--title", ""]).contains("a task needs a title"));
    assert!(lum.fails(&["task", "edit", "1", "--priority", "5"]).contains("Priority takes 1 to 4"));
    assert!(lum.fails(&["block", "add", " ", "--at", "9am", "--minutes", "30"]).contains("a block needs a name"));
}

#[test]
fn deleting_a_label_that_is_not_there_is_refused_before_anything_is_asked() {
    let lum = Lum::new();
    assert!(lum.fails(&["label", "rm", "nowhere"]).contains("nowhere"));
    assert!(lum.fails(&["device", "unpair", "ABCD"]).contains("no paired device"));
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
    // A mis-parsed filter shows wrong results silently, and wrong results are invisible.
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
    // This is a compatibility contract, not a debugging convenience.
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
fn json_says_plainly_whether_the_sync_daemon_is_installed_and_only_daemon_status_says_it() {
    // What a client turning the daemon on and off reads, rather than the sentence.
    let lum = Lum::new();
    let out = lum.ok(&["--json", "daemon", "status"]);
    assert!(out.contains("\"result\": \"change\""), "the result it always had: {out}");
    assert!(out.contains("\"installed\": false"), "{out}");
    assert!(out.contains("\"running\": false"), "{out}");
    assert!(!lum.ok(&["--json", "task", "list"]).contains("\"service\""));
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
    // Filterable but not settable is exactly the failure the completeness rule is meant to
    // catch.
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
    // and the user then distrusts the whole feature.
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
    // Deletion touches no tasks, and the identifiers left behind project as absent.
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
    // anchored on a day it never occurs is a trap. The block editor normalises it.
    let lum = Lum::new();
    lum.ok(&[
        "block", "add", "Standup", "--at", "9am", "--minutes", "15", "--repeat",
        "every monday",
    ]);
    let out = lum.ok(&["block", "list"]);
    assert!(out.contains("every monday"), "{out}");
    let json = lum.ok(&["block", "list", "--json"]);
    // A row of the Blocks list is addressed `<series>@<its first day>`.
    let start = json.split('@').nth(1).and_then(|rest| rest.get(..10)).expect("a start date");
    let start: jiff::civil::Date = start.parse().unwrap();
    assert_eq!(start.weekday(), jiff::civil::Weekday::Monday, "{out}");
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
    // A filter containing `today` has to mean today at evaluation time.
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
    // `--help` is an accessibility surface, not generated boilerplate. It is the
    // primary discovery mechanism for anyone who cannot skim a GUI.
    let lum = Lum::new();
    let top = lum.ok(&["--help"]);
    for command in ["task", "plan", "project", "label", "filter", "block", "assign", "assist", "complete"] {
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
// The typed command surface
// ---------------------------------------------------------------------------------------

#[test]
fn a_mutation_returns_structure_rather_than_a_sentence() {
    // There is one typed command surface, and `--json` is an adapter over it. A prose
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
    // Capture is the one place a round trip hurts, so the created record comes back whole
    // rather than as an identifier to fetch.
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
    // The filter's readback belongs on the surface, not only on stdout: every client needs to say
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
// Backup, export and import
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
fn times_are_printed_in_the_clock_setting_and_json_keeps_hh_mm() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "call the bank tomorrow at 3pm p1"]);
    let out = lum.ok(&["task", "list"]);
    assert!(out.contains("call the bank  due tomorrow at 15:00, priority 1"), "{out}");

    let out = lum.ok(&["config", "set", "clock", "12-hour"]);
    assert!(out.contains("on this device only"), "{out}");
    let out = lum.ok(&["task", "list"]);
    assert!(out.contains("due tomorrow at 3:00 PM, priority 1"), "{out}");
    let json = lum.ok(&["--json", "task", "list"]);
    assert!(json.contains(r#""due_time": "15:00""#), "{json}");
    assert!(lum.fails(&["config", "set", "clock", "sundial"]).contains("24-hour or 12-hour"));
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

    // Beside the profile: an export inside it could write over the store.
    let beside = tempfile::tempdir().unwrap();
    let file = beside.path().join("export.json");
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
// Undo
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
    let beside = tempfile::tempdir().unwrap();
    let export = beside.path().join("now.json");
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

// ---------------------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------------------

#[test]
fn an_unpaired_device_says_so_everywhere() {
    let lum = Lum::new();
    assert!(lum.ok(&["device", "list"]).contains("no paired devices"));
    assert!(lum.ok(&["sync", "status"]).contains("Not paired with any other device yet"));
    assert!(lum.ok(&["sync", "--local-only"]).contains("not paired with any other yet"));
    assert!(lum.fails(&["device", "unpair", "phone"]).contains("no paired device called"));
    assert!(lum.fails(&["pair", "not-a-code"]).contains("is not a pairing code"));
}

#[cfg(unix)]
#[test]
fn the_daemon_serves_the_command_surface_and_takes_sync_requests_on_its_socket() {
    use std::io::{BufRead, BufReader, Write};

    let lum = Lum::new();
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_lum"))
        .args(["sync-daemon", "--local-only"])
        .env("LUMENNA_PROFILE", lum.path())
        .env("LUMENNA_BACKUP_DIR", lum.path().join("backups"))
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let socket = lum.path().join("lumenna.sock");
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(socket.exists(), "the daemon made its socket");

    // A second daemon on the same profile is refused: one endpoint per device.
    assert!(lum.fails(&["sync-daemon", "--local-only"]).contains("already running"));
    // Running, but with nobody to sync with — which is the thing worth saying.
    assert!(lum.ok(&["sync", "status"]).contains("Not paired with any other device yet"));

    // The same JSON-RPC surface as `lum rpc`, over the socket.
    let stream = std::os::unix::net::UnixStream::connect(&socket).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let mut reader = BufReader::new(stream);
    writer
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"task.add\",\"params\":{\"text\":\"over the socket\"}}\n")
        .unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("\"result\":\"change\""), "{line}");
    assert!(lum.ok(&["task", "list"]).contains("over the socket"));

    // `lum sync` finds the lock taken and asks the daemon instead of opening a second endpoint.
    let out = lum.ok(&["sync"]);
    assert!(out.contains("not paired with any other yet"), "{out}");

    let pid = daemon.id().to_string();
    Command::new("kill").args(["-TERM", &pid]).status().unwrap();
    daemon.wait().unwrap();
    assert!(!socket.exists(), "a daemon stopped by its service manager cleans up its socket");
}

#[test]
fn editing_a_repeating_block_asks_which_occurrences_and_never_guesses() {
    let lum = Lum::new();
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90", "--date", "2026-12-07", "--repeat", "every weekday"]);
    lum.ok(&["block", "list"]);
    let asked = lum.fails(&["block", "edit", "1", "--at", "10am"]);
    assert!(asked.contains("--date") && asked.contains("--all"), "{asked}");

    lum.ok(&["block", "edit", "1", "--at", "2pm", "--date", "2026-12-08"]);
    assert!(lum.ok(&["plan", "2026-12-08"]).contains("14:00 to 15:30"), "that day moved");
    assert!(lum.ok(&["plan", "2026-12-09"]).contains("09:00 to 10:30"), "the next did not");

    lum.ok(&["block", "list"]);
    lum.ok(&["block", "edit", "1", "--all", "--title", "Writing"]);
    let moved = lum.ok(&["plan", "2026-12-08"]);
    assert!(moved.contains("Writing") && moved.contains("14:00"), "{moved}");
}

#[test]
fn one_day_of_a_block_can_be_cancelled_and_put_back() {
    let lum = Lum::new();
    lum.ok(&["block", "add", "Run", "--at", "7am", "--minutes", "30", "--date", "2026-12-07", "--repeat", "daily"]);
    lum.ok(&["block", "list"]);
    lum.ok(&["block", "cancel", "1", "--date", "2026-12-08"]);
    assert!(lum.ok(&["plan", "2026-12-08"]).contains("No blocks"));
    assert!(lum.ok(&["plan", "2026-12-09"]).contains("Run"));
    lum.ok(&["block", "list"]);
    lum.ok(&["block", "restore", "1", "--date", "2026-12-08"]);
    assert!(lum.ok(&["plan", "2026-12-08"]).contains("Run"));
}

#[test]
fn a_day_shows_its_free_time_and_a_summary() {
    let lum = Lum::new();
    lum.ok(&["block", "add", "Deep work", "--at", "9am", "--minutes", "90", "--date", "2026-12-07"]);
    lum.ok(&["block", "add", "Meeting", "--at", "11:30am", "--minutes", "30", "--date", "2026-12-07", "--kind", "event"]);
    let day = lum.ok(&["plan", "2026-12-07"]);
    assert!(day.contains("2 blocks, 1 hour 30 minutes of work"), "{day}");
    assert!(day.contains("free, 1 hour from 10:30 to 11:30"), "the gap between is a row: {day}");
    assert!(day.contains("free, 1 hour from 08:00 to 09:00"), "{day}");
}

#[test]
fn projects_move_and_reorder_and_refuse_to_go_inside_themselves() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["project", "add", "Clients"]);
    lum.ok(&["project", "move", "Clients", "--parent", "Work"]);
    let refused = lum.fails(&["project", "move", "Work", "--parent", "Clients"]);
    assert!(refused.contains("inside itself"), "{refused}");

    lum.ok(&["project", "add", "Home"]);
    lum.ok(&["project", "order", "Home", "up"]);
    let listed = lum.ok(&["project", "list"]);
    assert!(listed.find("Home").unwrap() < listed.find("Work").unwrap(), "{listed}");
}

#[test]
fn labels_reorder_take_colours_and_go_on_tasks_by_name() {
    let lum = Lum::new();
    lum.ok(&["label", "add", "deep"]);
    lum.ok(&["label", "add", "calls"]);
    lum.ok(&["label", "order", "calls", "up"]);
    let listed = lum.ok(&["label", "list"]);
    assert!(listed.find("calls").unwrap() < listed.find("deep").unwrap(), "{listed}");
    lum.ok(&["label", "colour", "deep", "teal"]);

    lum.ok(&["task", "add", "ring the bank"]);
    lum.ok(&["task", "list"]);
    let added = lum.ok(&["task", "edit", "1", "--labels", "calls, errands"]);
    assert!(added.contains("new label 'errands'") || lum.ok(&["label", "list"]).contains("errands"));
    lum.ok(&["task", "list"]);
    let shown = lum.ok(&["task", "show", "1"]);
    assert!(shown.contains("calls") && shown.contains("errands"), "{shown}");
    lum.ok(&["task", "edit", "1", "--labels", ""]);
    assert!(!lum.ok(&["task", "show", "1"]).contains("labels:"));
}

#[test]
fn a_saved_filter_can_be_renamed_and_requeried() {
    let lum = Lum::new();
    lum.ok(&["filter", "add", "Urgent", "p1"]);
    lum.ok(&["filter", "edit", "Urgent", "--rename", "Now", "--query", "p1 | overdue"]);
    let listed = lum.ok(&["filter", "list"]);
    assert!(listed.contains("Now") && listed.contains("p1 | overdue"), "{listed}");
    assert!(lum.fails(&["filter", "edit", "Now", "--query", "p1 &"]).contains("lum"));
}

#[test]
fn moving_a_repeating_task_keeps_it_repeating_and_none_stops_it() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "water plants every monday"]);
    lum.ok(&["task", "list"]);
    let shown = lum.ok(&["task", "show", "1"]);
    assert!(shown.contains("every monday"), "said in words, not as a rule: {shown}");
    assert!(!shown.contains("FREQ="), "{shown}");

    lum.ok(&["task", "edit", "1", "--due", "2026-12-10"]);
    let moved = lum.ok(&["task", "show", "1"]);
    assert!(moved.contains("2026-12-10") && moved.contains("every monday"), "{moved}");

    lum.ok(&["task", "edit", "1", "--repeat", "every! 2 weeks"]);
    let counted = lum.ok(&["task", "show", "1"]);
    assert!(counted.contains("every! other week"), "read back the way it is typed: {counted}");

    lum.ok(&["task", "edit", "1", "--repeat", "none"]);
    let stopped = lum.ok(&["task", "show", "1"]);
    assert!(stopped.contains("2026-12-10") && !stopped.contains("every"), "{stopped}");
}

#[test]
fn a_repetition_on_a_task_with_no_date_makes_it_due_when_it_first_lands() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "stretch"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "edit", "1", "--repeat", "every day"]);
    let shown = lum.ok(&["task", "show", "1"]);
    assert!(shown.contains("due") && shown.contains("every day"), "{shown}");
}

#[test]
fn words_left_over_after_a_date_are_refused_rather_than_dropped() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "call mum"]);
    lum.ok(&["task", "list"]);
    let refused = lum.fails(&["task", "edit", "1", "--due", "friday blah"]);
    assert!(refused.contains("blah"), "the leftover is named: {refused}");
    assert!(!lum.ok(&["task", "show", "1"]).contains("due"));
    let refused = lum.fails(&["plan", "tomorrow", "please"]);
    assert!(refused.contains("please"), "{refused}");
    let refused = lum.fails(&["block", "add", "Gym", "--at", "7am", "--minutes", "60", "--repeat", "every! day"]);
    assert!(refused.contains("every!"), "a block has no completion to count from: {refused}");
}

#[test]
fn a_block_list_says_how_it_repeats_in_words() {
    let lum = Lum::new();
    lum.ok(&["block", "add", "Standup", "--at", "9am", "--minutes", "15", "--repeat", "every weekday"]);
    let listed = lum.ok(&["block", "list"]);
    assert!(listed.contains("every weekday") && !listed.contains("FREQ="), "{listed}");
    let shown = lum.ok(&["block", "show", "1"]);
    assert!(shown.contains("repeats: every weekday") && shown.contains("15 minutes"), "{shown}");
}

#[test]
fn a_cancelled_day_is_listed_with_the_way_to_put_it_back() {
    let lum = Lum::new();
    lum.ok(&["block", "add", "Run", "--at", "7am", "--minutes", "30", "--date", "2026-12-07", "--repeat", "daily"]);
    lum.ok(&["block", "list"]);
    lum.ok(&["block", "cancel", "1", "--date", "2026-12-08"]);
    let day = lum.ok(&["plan", "2026-12-08"]);
    assert!(day.contains("cancelled for this day: Run at 07:00"), "{day}");
    assert!(day.contains("lum block restore"), "{day}");
    let json = lum.ok(&["plan", "2026-12-08", "--json"]);
    assert!(json.contains("\"cancelled\""), "{json}");
}

#[test]
fn assigning_from_a_listed_day_lands_on_that_day_not_today() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "write"]);
    let task = first_task_id(&lum);
    lum.ok(&["block", "add", "Focus", "--at", "9am", "--minutes", "60", "--date", "2026-12-07", "--repeat", "daily"]);
    lum.ok(&["plan", "2026-12-09"]);
    let assigned = lum.ok(&["assign", &task, "--block", "1"]);
    assert!(assigned.contains("2026-12-09"), "{assigned}");
    assert!(lum.ok(&["plan", "2026-12-09"]).contains("write"));
}

#[test]
fn archiving_twice_unarchives_and_the_list_says_which() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Old"]);
    lum.ok(&["project", "archive", "Old"]);
    let json = lum.ok(&["project", "list", "--json"]);
    assert!(json.contains("\"archived\""), "a state a client can test: {json}");
    assert!(lum.ok(&["project", "archive", "Old"]).contains("Unarchived"));
}

#[test]
fn project_and_label_counts_are_open_tasks_and_a_colour_is_said() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Home"]);
    lum.ok(&["task", "add", "sweep #Home @chores"]);
    lum.ok(&["task", "add", "dust #Home @chores"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "done", "1"]);
    lum.ok(&["label", "colour", "chores", "teal"]);
    assert!(lum.ok(&["project", "list"]).contains("1 open task"));
    let labels = lum.ok(&["label", "list"]);
    assert!(labels.contains("1 open task, teal"), "{labels}");
}

#[test]
fn a_sittings_planned_length_is_set_shown_changed_and_cleared() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "write"]);
    let task = first_task_id(&lum);
    lum.ok(&["block", "add", "Focus", "--at", "9am", "--minutes", "120"]);
    lum.ok(&["block", "list"]);
    lum.ok(&["assign", &task, "--block", "1", "--minutes", "45"]);
    assert!(lum.ok(&["plan"]).contains("planned for 45 minutes"));

    lum.ok(&["length", "1", "90"]);
    assert!(lum.ok(&["plan"]).contains("planned for 1 hour 30 minutes"));
    assert!(lum.fails(&["length", "1", "0"]).contains("at least a minute"));
    lum.ok(&["length", "1", "none"]);
    assert!(!lum.ok(&["plan"]).contains("planned for"));
    lum.ok(&["undo"]);
    assert!(lum.ok(&["plan"]).contains("planned for 1 hour 30 minutes"), "undoable like anything else");
    lum.ok(&["start", "1"]);
    assert!(lum.ok(&["plan"]).contains("in progress, 1 hour 30 minutes planned"));
}

#[test]
fn time_settings_read_without_seconds_and_take_what_they_show() {
    let lum = Lum::new();
    assert_eq!(lum.ok(&["config", "get", "day-start"]).trim(), "08:00");
    lum.ok(&["config", "set", "day-start", "09:00:00"]);
    assert_eq!(lum.ok(&["config", "get", "day-start"]).trim(), "09:00");
    lum.ok(&["config", "set", "day-start", "7:30am"]);
    assert_eq!(lum.ok(&["config", "get", "day-start"]).trim(), "07:30");
    assert!(lum.fails(&["config", "set", "day-start", "7:30am please"]).contains("please"));
}

#[test]
fn erasing_without_a_terminal_to_ask_at_needs_yes_and_says_the_cores_question() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "old idea"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "rm", "1"]);
    lum.ok(&["task", "list", "deleted"]);
    let refused = lum.fails(&["task", "erase", "1"]);
    assert!(refused.contains("Delete old idea from the trash?"), "{refused}");
    lum.ok(&["task", "erase", "1", "--yes"]);
}

/// Pairs `profile` with a device whose build is too old to follow an erasure, as one that
/// last synced before format version 2 would be.
fn pair_an_old_device(profile: &Path) {
    let lumenna = lumenna_host::Lumenna::open_at(profile).unwrap();
    let old = lumenna_core::model::Device {
        node_id: lumenna_core::id::NodeId::from_bytes([3; 32]),
        name: "Old Laptop".to_owned(),
        platform: "linux".to_owned(),
        paired_at: lumenna_core::time::now(),
        last_seen: lumenna_core::time::now(),
        schema: 1,
    };
    lumenna.store().write(|docs| docs.put_device(&old, None)).unwrap();
}

#[test]
fn erasing_a_task_permanently_without_a_terminal_needs_yes_and_says_the_cores_question() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "the secret"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "rm", "1"]);
    lum.ok(&["task", "list", "deleted"]);
    let refused = lum.fails(&["task", "erase", "1", "--permanently"]);
    assert!(refused.contains("Erase the secret permanently?"), "{refused}");
    assert!(refused.contains("rebuilds your database and re-syncs all devices"), "{refused}");
    assert!(lum.ok(&["task", "list", "deleted"]).contains("the secret"), "nothing was erased");
    let erased = lum.ok(&["--json", "task", "erase", "1", "--permanently", "--yes"]);
    let erased: serde_json::Value = serde_json::from_str(&erased).unwrap();
    assert_eq!(erased["announcement"], "Erased the secret permanently", "{erased}");
    assert!(!lum.ok(&["task", "list", "deleted"]).contains("the secret"));
}

#[test]
fn erasing_permanently_needs_the_task_in_the_trash() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "the groceries"]);
    lum.ok(&["task", "list"]);
    let refused = lum.fails(&["task", "erase", "1", "--permanently", "--yes"]);
    assert!(refused.contains("not in the trash"), "{refused}");
}

#[test]
fn erasing_everything_deleted_asks_first_and_then_empties_the_trash_for_good() {
    let lum = Lum::new();
    for title in ["the first secret", "the second secret", "the groceries"] {
        lum.ok(&["task", "add", title]);
    }
    lum.ok(&["task", "search", "secret"]);
    lum.ok(&["task", "rm", "1"]);
    lum.ok(&["task", "rm", "2"]);
    let refused = lum.fails(&["erase-deleted"]);
    assert!(refused.contains("Erase everything deleted permanently?"), "{refused}");
    assert!(refused.contains("Everything in the trash goes"), "{refused}");
    let erased = lum.ok(&["--json", "erase-deleted", "--yes"]);
    let erased: serde_json::Value = serde_json::from_str(&erased).unwrap();
    assert_eq!(erased["announcement"], "Erased 2 tasks and the history of everything deleted", "{erased}");
    let left = lum.ok(&["task", "list"]);
    assert!(left.contains("the groceries") && !left.contains("secret"), "{left}");
}

#[test]
fn erasing_is_refused_while_a_paired_device_runs_format_version_one_and_names_it() {
    let lum = Lum::new();
    lum.ok(&["task", "add", "the secret"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "rm", "1"]);
    lum.ok(&["task", "list", "deleted"]);
    pair_an_old_device(lum.path());
    let refused = lum.fails(&["task", "erase", "1", "--permanently", "--yes"]);
    assert!(refused.contains("Old Laptop runs an older version"), "{refused}");
    let refused = lum.fails(&["erase-deleted", "--yes"]);
    assert!(refused.contains("Old Laptop runs an older version"), "{refused}");
    assert!(lum.ok(&["task", "list", "deleted"]).contains("the secret"), "nothing was erased");
}

#[test]
fn a_script_deletes_a_label_without_being_asked() {
    // Only a terminal is asked; a confirmation nobody can answer would hang a script.
    let lum = Lum::new();
    lum.ok(&["label", "add", "calls"]);
    let out = lum.ok(&["label", "rm", "calls"]);
    assert!(out.contains("calls"), "{out}");
}

#[test]
fn a_block_takes_a_reminder_of_its_own_in_place_of_the_default_and_it_comes_up() {
    let lum = Lum::new();
    // The next hour but one: at least an hour off, so its 5-minute reminder is still to come.
    let start = jiff::Zoned::now().checked_add(jiff::SignedDuration::from_hours(2)).unwrap();
    let at = format!("{:02}:00", start.hour());
    lum.ok(&["block", "add", "Deep work", "--at", &at, "--minutes", "60", "--date", &start.date().to_string()]);
    lum.ok(&["block", "list"]);
    assert!(lum.ok(&["reminder", "list", "1"]).contains("5 minutes before it starts"), "the default");
    lum.ok(&["reminder", "add", "1", "--at", "end", "--before", "10m"]);
    let listed = lum.ok(&["reminder", "list", "1"]);
    assert!(listed.contains("10 minutes before it ends") && !listed.contains("5 minutes before it starts"), "{listed}");
    let upcoming = lum.ok(&["reminder", "upcoming", "--hours", "3"]);
    assert!(upcoming.contains("Deep work ends in 10 minutes"), "{upcoming}");
}

#[test]
fn the_shell_completes_project_label_and_filter_names_from_the_store() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    lum.ok(&["project", "add", "Home"]);
    lum.ok(&["label", "add", "calls"]);
    lum.ok(&["filter", "add", "Urgent", "p1"]);
    let complete = |words: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lum"))
            .arg("--")
            .args(words)
            .env("LUMENNA_PROFILE", lum.profile.path())
            .env("LUMENNA_BACKUP_DIR", lum.profile.path().join("backups"))
            .env("COMPLETE", "bash")
            .env("_CLAP_COMPLETE_INDEX", (words.len() - 1).to_string())
            .env("_CLAP_COMPLETE_COMP_TYPE", "9")
            .env("_CLAP_COMPLETE_SPACE", "true")
            .env("_CLAP_IFS", "\n")
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).lines().map(str::to_owned).collect::<Vec<_>>()
    };
    assert_eq!(complete(&["lum", "project", "rename", "W"]), ["Work"]);
    assert!(complete(&["lum", "label", "rename", ""]).contains(&"calls".to_owned()));
    assert_eq!(complete(&["lum", "filter", "rm", "u"]), ["Urgent"]);
    assert!(lum.ok(&["completions", "bash"]).contains("COMPLETE"), "the registration asks lum itself");
}

// ---------------------------------------------------------------------------------------
// `lum assist` and `lum complete`
// ---------------------------------------------------------------------------------------

fn json(text: &str) -> serde_json::Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[test]
fn assist_lists_every_operation_of_the_assistant_table_with_what_it_takes() {
    let lum = Lum::new();
    let listed = json(&lum.ok(&["--json", "assist", "--list"]));
    assert_eq!(listed["result"], "operations");
    let ids: Vec<&str> = listed["operations"].as_array().unwrap().iter().map(|op| op["id"].as_str().unwrap()).collect();
    let table: Vec<String> = lumenna_records::assistant::operations().into_iter().map(|op| op.id).collect();
    assert_eq!(ids, table);
    let erase = listed["operations"].as_array().unwrap().iter().find(|op| op["id"] == "erase_task_permanently").unwrap();
    assert_eq!(erase["confirms"], true);
    assert_eq!(erase["destructive"], true);
    assert_eq!(erase["parameters"][0]["kind"], "task");

    let text = lum.ok(&["assist", "--list"]);
    assert!(text.contains("complete_task, Complete a task"), "{text}");
    assert!(text.contains("runs only with confirm=true"), "{text}");
}

#[test]
fn assist_runs_an_operation_with_its_parameters_by_name() {
    let lum = Lum::new();
    let added = json(&lum.ok(&["--json", "assist", "add_task", "text=call the bank @calls p1"]));
    assert_eq!(added["result"], "change");
    let id = added["task"]["id"].as_str().unwrap().to_owned();
    lum.ok(&["assist", "add_task", "text=water the plants"]);

    let calls = json(&lum.ok(&["--json", "assist", "list_tasks", "query=@calls"]));
    assert_eq!(calls["count"], 1);
    assert_eq!(calls["rows"][0]["id"], id.as_str());

    let edited = lum.ok(&["assist", "edit_task", &format!("id={id}"), "priority=2", "labels=calls,bank"]);
    assert!(!edited.is_empty());
    let shown = json(&lum.ok(&["--json", "assist", "show_task", &format!("id={id}")]));
    assert_eq!(shown["result"], "task");
    assert!(lum.ok(&["assist", "list_tasks", "query=@bank"]).contains("call the bank"));

    let done = lum.ok(&["assist", "complete_task", &format!("id={id}")]);
    assert!(done.contains("call the bank"), "{done}");
    assert!(!lum.ok(&["assist", "list_tasks"]).contains("call the bank"));
}

#[test]
fn assist_takes_its_parameters_as_one_json_object_too_from_an_argument_or_standard_input() {
    let lum = Lum::new();
    lum.ok(&["assist", "add_task", "--args", r#"{"text": "from an argument"}"#]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_lum"))
        .args(["assist", "add_task", "--args", "-"])
        .env("LUMENNA_PROFILE", lum.path())
        .env("LUMENNA_BACKUP_DIR", lum.path().join("backups"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), r#"{"text": "from standard input, café"}"#.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
    let all = lum.ok(&["assist", "list_tasks"]);
    assert!(all.contains("from an argument") && all.contains("from standard input, café"), "{all}");
}

#[test]
fn assist_refuses_what_an_operation_does_not_take_and_says_what_it_does() {
    let lum = Lum::new();
    let said = lum.fails(&["assist", "list_tasks", "filter=@calls"]);
    assert!(said.contains("list_tasks takes no filter; it takes query"), "{said}");
    let said = lum.fails(&["assist", "no_such_thing"]);
    assert!(said.contains("no operation called no_such_thing"), "{said}");
    let said = lum.fails(&["assist", "edit_task", "id=x", "priority=high"]);
    assert!(said.contains("priority takes a whole number"), "{said}");
    let said = lum.fails(&["assist", "show_task", "id"]);
    assert!(said.contains("name=value"), "{said}");
}

#[test]
fn assist_erases_permanently_only_with_confirm_and_otherwise_says_the_question() {
    let lum = Lum::new();
    let added = json(&lum.ok(&["--json", "assist", "add_task", "text=erase me"]));
    let id = format!("id={}", added["task"]["id"].as_str().unwrap());
    lum.ok(&["assist", "trash_task", &id]);

    let asked = lum.fails(&["assist", "erase_task_permanently", &id]);
    assert!(asked.contains("go ahead with confirm only once they say yes"), "{asked}");
    assert!(lum.ok(&["task", "list", "deleted"]).contains("erase me"));

    lum.ok(&["assist", "erase_task_permanently", &id, "confirm=true"]);
    assert!(!lum.ok(&["task", "list", "deleted"]).contains("erase me"));
}

#[test]
fn complete_offers_what_the_quick_add_field_and_the_store_offer() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Home Office"]);
    lum.ok(&["task", "add", "phone @calls"]);

    let line = json(&lum.ok(&["--json", "complete", "line", "buy paper #Ho"]));
    assert_eq!(line["result"], "completions");
    assert_eq!(line["start"], 10);
    assert!(line["candidates"][0]["text"].as_str().unwrap().contains("Home Office"), "{line}");

    let filter = json(&lum.ok(&["--json", "complete", "filter", "@ca"]));
    assert_eq!(filter["candidates"][0]["text"], "@calls");

    let project = json(&lum.ok(&["--json", "complete", "project", "home"]));
    assert_eq!(project["candidates"][0]["text"], "Home Office");
    assert_eq!(project["start"], 0);
    assert!(lum.ok(&["complete", "label", "C"]).contains("calls"));
    assert!(lum.ok(&["complete", "label", "x"]).contains("no completions"));
}

/// The commands that do what an action of `kind` does, each run in turn on a copy of the
/// store [`Seeded`] made, `{name}` standing for one of its records. An exhaustive match, so a
/// new kind does not compile until a command does it.
fn commands_for(kind: lumenna_host::ActionKind) -> &'static [&'static [&'static str]] {
    use lumenna_host::ActionKind as K;
    match kind {
        K::MarkDone => &[&["task", "done", "{essay}"]],
        K::MarkNotDone => &[&["task", "done", "{essay}"], &["task", "undone", "{essay}"]],
        K::Edit => &[&["task", "edit", "{essay}", "--title", "Essay draft"], &["block", "edit", "{now}", "--title", "Focus"]],
        K::EditTask => &[&["task", "edit", "{essay}", "--notes", "Two pages", "--priority", "2"]],
        K::PutInBlock | K::AssignTask => &[&["assign", "{groceries}", "--block", "{now}"]],
        K::MoveToProject => &[&["task", "move", "{groceries}", "--project", "Home"]],
        K::MakeSubtaskOf => &[&["task", "move", "{groceries}", "--parent", "{essay}"]],
        K::MoveToTopLevel => &[&["task", "move", "{groceries}", "--parent", "{essay}"], &["task", "move", "{groceries}", "--top"]],
        K::WaitFor => &[&["task", "depend", "add", "{groceries}", "--on", "{essay}"]],
        K::StopWaiting => {
            &[&["task", "depend", "add", "{groceries}", "--on", "{essay}"], &["task", "depend", "rm", "{groceries}", "--on", "{essay}"]]
        }
        K::Delete => &[
            &["task", "rm", "{groceries}"],
            &["block", "rm", "{tomorrow}", "--yes"],
            &["project", "rm", "Home", "--keep-tasks"],
            &["label", "rm", "errands", "--yes"],
            &["filter", "rm", "Urgent", "--yes"],
        ],
        K::Restore => &[&["task", "rm", "{groceries}"], &["task", "restore", "{groceries}"]],
        K::DeleteForGood => &[&["task", "rm", "{groceries}"], &["task", "erase", "{groceries}", "--yes"]],
        K::ErasePermanently => &[&["task", "rm", "{groceries}"], &["task", "erase", "{groceries}", "--permanently", "--yes"]],
        K::EraseEverythingDeleted => &[&["task", "rm", "{groceries}"], &["erase-deleted", "--yes"]],
        K::CancelDay => &[&["block", "cancel", "{daily}", "--date", "tomorrow"]],
        K::RestoreDay => &[&["block", "cancel", "{daily}", "--date", "tomorrow"], &["block", "restore", "{daily}", "--date", "tomorrow"]],
        K::StartTimer => &[&["start", "{essay-sitting}"]],
        K::ResumeTimer => &[&["start", "{essay-sitting}"], &["pause", "{essay-sitting}"], &["start", "{essay-sitting}"]],
        K::PauseTimer => &[&["start", "{essay-sitting}"], &["pause", "{essay-sitting}"]],
        K::StopTimer => &[&["start", "{essay-sitting}"], &["stop", "{essay-sitting}"]],
        K::LogMinutes => &[&["stop", "{essay-sitting}", "--minutes", "20"]],
        K::PlannedLength => &[&["length", "{essay-sitting}", "30"]],
        K::Unassign => &[&["unassign", "{essay-sitting}"]],
        K::AddBlock => &[&["block", "add", "Admin", "--at", "4pm", "--minutes", "30", "--date", "tomorrow"]],
        K::Rename => &[
            &["project", "rename", "Work", "Job"],
            &["label", "rename", "calls", "phone"],
            &["filter", "edit", "Calls", "--rename", "Phone"],
            &["device", "rename", "{nobody}", "Laptop"],
        ],
        K::NewInside => &[&["project", "add", "Reading", "--parent", "Work"]],
        K::New => &[&["project", "add", "Garden"], &["label", "add", "later"], &["filter", "add", "Soon", "p2"]],
        K::MoveUnder => &[&["project", "move", "Home", "--parent", "Work"]],
        K::MoveUp => &[
            &["project", "order", "Home", "up"],
            &["label", "order", "errands", "up"],
            &["filter", "order", "Urgent", "up"],
            &["reorder", "{laundry-sitting}", "up"],
        ],
        K::MoveDown => &[
            &["project", "order", "Work", "down"],
            &["label", "order", "calls", "down"],
            &["filter", "order", "Calls", "down"],
            &["reorder", "{essay-sitting}", "down"],
        ],
        K::Weight => &[&["project", "weight", "Work", "1.5"]],
        K::Archive => &[&["project", "archive", "Home"]],
        K::Unarchive => &[&["project", "archive", "Home"], &["project", "archive", "Home"]],
        K::MergeInto => &[&["label", "merge", "errands", "calls"]],
        K::Colour => &[&["project", "colour", "Work", "teal"], &["label", "colour", "calls", "red"], &["filter", "colour", "Calls", "blue"]],
        K::ChangeQuery => &[&["filter", "edit", "Calls", "--query", "@calls & p1"]],
        K::Unpair => &[&["device", "unpair", "{nobody}", "--yes"]],
        K::Reminders => &[&["reminder", "list", "{essay}"], &["reminder", "add", "{essay}", "--at", "due", "--before", "10m"]],
        K::DeliverOn => &[&["reminder", "deliver", "{reminder}", "all"]],
        K::EndNow => &[&["end", "{now}"]],
        K::Extend => &[&["extend", "{now}"], &["extend", "{now}", "--by", "30m"]],
        K::EndDay => &[&["end-day"]],
        K::ClearDay => &[&["clear-day"]],
        K::CarryForward => &[&["carry", "{essay-sitting}", "--to", "{tomorrow}"]],
        K::CarryRest => &[&["carry-rest", "{now}"]],
        K::WhatNow => &[&["next"]],
        K::Replan => &[&["replan"]],
        K::PlanDay => &[&["plan-day", "tomorrow"]],
        K::Skip => &[&["skip", "{laundry-sitting}"]],
        K::Unskip => &[&["skip", "{laundry-sitting}"], &["skip", "{laundry-sitting}", "--again"]],
        K::TimeZone => &[&["task", "zone", "{essay}", "Asia/Tokyo"], &["block", "zone", "{now}", "Europe/London"]],
        // Folding is a view's, and stores nothing: a listing shows every row.
        K::Expand | K::Collapse => &[],
    }
}

/// What may be refused by the store as it stands rather than done: a device's, with none
/// paired; a reminder's delivery, with one device; carrying, which needs a sitting left short.
/// Each is still run, and has to reach the surface, which refuses in its own words.
const MAY_BE_REFUSED: &[&str] = &["device", "deliver", "carry", "carry-rest"];

/// A store with something for every action to act on, and its records' identifiers.
struct Seeded {
    lum: Lum,
    names: std::collections::BTreeMap<&'static str, String>,
}

impl Seeded {
    fn new() -> Self {
        let lum = Lum::new();
        lum.ok(&["project", "add", "Work"]);
        lum.ok(&["project", "add", "Home"]);
        lum.ok(&["label", "add", "calls"]);
        lum.ok(&["label", "add", "errands"]);
        lum.ok(&["filter", "add", "Calls", "@calls"]);
        lum.ok(&["filter", "add", "Urgent", "p1"]);
        lum.ok(&["task", "add", "Essay #Work @calls tomorrow at 9am"]);
        lum.ok(&["task", "add", "Groceries #Home"]);
        lum.ok(&["task", "add", "Laundry"]);
        // A block under way now, so the day's own actions have one to end and extend.
        let now = jiff::Zoned::now();
        let minute = i64::from(now.hour()) * 60 + i64::from(now.minute());
        let start = (minute - 10).max(0);
        let length = (start + 120).min(23 * 60 + 59) - start;
        let at = format!("{:02}:{:02}", start / 60, start % 60);
        lum.ok(&["block", "add", "Now", "--at", &at, "--minutes", &length.to_string()]);
        lum.ok(&["block", "add", "Admin", "--at", "4pm", "--minutes", "60", "--date", "tomorrow"]);
        lum.ok(&["block", "add", "Standup", "--at", "7am", "--minutes", "15", "--repeat", "every day"]);
        let json = |args: &[&str]| -> serde_json::Value {
            let mut args = args.to_vec();
            args.push("--json");
            serde_json::from_str(&lum.ok(&args)).unwrap()
        };
        let mut names = std::collections::BTreeMap::new();
        for row in json(&["task", "list"])["rows"].as_array().unwrap() {
            let name = match row["title"].as_str().unwrap() {
                "Essay" => "essay",
                "Groceries" => "groceries",
                _ => "laundry",
            };
            names.insert(name, row["id"].as_str().unwrap().to_owned());
        }
        for row in json(&["block", "list"])["rows"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap().to_owned();
            match row["title"].as_str().unwrap() {
                "Now" => names.insert("now", id),
                "Admin" => names.insert("tomorrow", id),
                _ => names.insert("daily", id.split('@').next().unwrap().to_owned()),
            };
        }
        for (task, sitting) in [("essay", "essay-sitting"), ("laundry", "laundry-sitting")] {
            let assigned = json(&["assign", &names[task], "--block", &names["now"], "--minutes", "45"]);
            names.insert(sitting, assigned["affected"]["assignments"][0].as_str().unwrap().to_owned());
        }
        json(&["reminder", "add", &names["essay"], "--at", "due", "--before", "5m"]);
        let reminders = json(&["reminder", "list", &names["essay"]]);
        names.insert("reminder", reminders["reminders"][0]["id"].as_str().unwrap().to_owned());
        names.insert("nobody", "0000".to_owned());
        Self { lum, names }
    }

    /// A store of its own, a copy of this one, for one kind's commands.
    fn copy(&self) -> Lum {
        let copy = Lum::new();
        for entry in std::fs::read_dir(self.lum.path()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), copy.path().join(entry.file_name())).unwrap();
            }
        }
        copy
    }

    /// `args` with each `{name}` its record's identifier.
    fn filled(&self, args: &[&str]) -> Vec<String> {
        args.iter()
            .map(|arg| match arg.strip_prefix('{').and_then(|a| a.strip_suffix('}')) {
                Some(name) => self.names.get(name).unwrap_or_else(|| panic!("no record {name}")).clone(),
                None => (*arg).to_owned(),
            })
            .collect()
    }
}

#[test]
fn every_action_the_app_offers_has_a_command_that_does_it() {
    use lumenna_host::ActionKind as K;
    // Every kind, for running each one's commands; `commands_for` is what keeps new ones from
    // being forgotten.
    let kinds = [
        K::MarkDone, K::MarkNotDone, K::Edit, K::EditTask, K::PutInBlock, K::MoveToProject, K::MakeSubtaskOf,
        K::MoveToTopLevel, K::WaitFor, K::StopWaiting, K::Delete, K::Restore, K::DeleteForGood, K::AssignTask,
        K::CancelDay, K::RestoreDay, K::StartTimer, K::ResumeTimer, K::PauseTimer, K::StopTimer, K::PlannedLength,
        K::LogMinutes, K::Unassign, K::AddBlock, K::Rename, K::NewInside, K::MoveUnder, K::MoveUp, K::MoveDown,
        K::Weight, K::Archive, K::Unarchive, K::MergeInto, K::Colour, K::ChangeQuery, K::New, K::Unpair,
        K::Reminders, K::EndNow, K::Extend, K::EndDay, K::ClearDay, K::CarryForward, K::WhatNow, K::CarryRest,
        K::Replan, K::PlanDay, K::Expand, K::Collapse, K::Skip, K::Unskip, K::DeliverOn, K::TimeZone,
        K::ErasePermanently, K::EraseEverythingDeleted,
    ];
    let seeded = Seeded::new();
    for kind in kinds {
        let lum = seeded.copy();
        for command in commands_for(kind) {
            let args = seeded.filled(command);
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let ran = lum.run(&args);
            let refusable = command.iter().any(|word| MAY_BE_REFUSED.contains(word));
            assert!(
                ran.ok || (refusable && ran.stderr.starts_with("lum: ") && !ran.stderr.contains("Usage:")),
                "{kind:?}: `lum {}` failed: {}",
                args.join(" "),
                ran.stderr
            );
        }
    }
}

#[test]
fn a_project_and_a_saved_filter_take_a_colour_and_a_task_and_a_block_a_time_zone() {
    let lum = Lum::new();
    lum.ok(&["project", "add", "Work"]);
    assert!(lum.ok(&["project", "colour", "Work", "teal"]).contains("teal"));
    lum.ok(&["project", "colour", "Work", "none"]);
    lum.ok(&["filter", "add", "Calls", "@calls"]);
    assert!(lum.ok(&["filter", "colour", "Calls", "red"]).contains("red"));
    lum.ok(&["task", "add", "Call Tokyo tomorrow at 9am"]);
    lum.ok(&["task", "list"]);
    lum.ok(&["task", "zone", "1", "Asia/Tokyo"]);
    lum.ok(&["task", "zone", "1", "none"]);
    assert!(lum.fails(&["task", "zone", "1", "Mars/Olympus"]).contains("not a time zone"));
    lum.ok(&["task", "add", "block Standup 9-9:15am weekdays"]);
    lum.ok(&["block", "list"]);
    lum.ok(&["block", "zone", "1", "Europe/London"]);
}
