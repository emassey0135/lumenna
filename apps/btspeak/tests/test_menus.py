"""The menus, driven as a person would drive them, against a real `lum rpc`.

Each test is a script: open this, choose that, type this — and then the store is asked
whether it happened. That checks what a mock could not: that every flow sends a method the
server answers, with parameters it reads, and says something back.
"""

from __future__ import annotations

import os
import shutil
import sys
import tempfile
import threading
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import btspeak_stub  # noqa: E402

btspeak_stub.install()

from connect import connect  # noqa: E402
from session import Session  # noqa: E402
from test_client import LUM  # noqa: E402
import day  # noqa: E402
import menus  # noqa: E402
import organise  # noqa: E402
import preferences  # noqa: E402
import tasks  # noqa: E402

play = btspeak_stub.play


@unittest.skipIf(LUM is None, "`lum` has not been built")
class Menus(unittest.TestCase):
    def setUp(self):
        self.profile = Path(tempfile.mkdtemp())
        os.environ["PATH"] = f"{Path(LUM).parent}{os.pathsep}{os.environ['PATH']}"
        os.environ["LUMENNA_BACKUP_DIR"] = str(self.profile / "backups")
        # This app's own preferences, kept out of the real ones.
        os.environ["XDG_CONFIG_HOME"] = str(self.profile / "config")
        self.client = connect(self.profile)
        self.session = Session(self.client)

    def tearDown(self):
        self.client.close()
        shutil.rmtree(self.profile, ignore_errors=True)

    def call(self, method, **params):
        return self.client.call(method, **params)

    def titles(self, query=""):
        rows = self.call("task.list", query=query) if query else self.call("task.list")
        return [row["title"] for row in rows["rows"]]

    def task(self, title, query=""):
        rows = self.call("task.list", query=query) if query else self.call("task.list")
        row = next(row for row in rows["rows"] if row["title"] == title)
        return self.call("task.show", id=row["id"])

    def run_script(self, steps, start):
        script = play(steps)
        start()
        self.assertTrue(script.finished(), f"steps left over: {script.steps}")
        return script

    # -- tasks ------------------------------------------------------------------------

    def test_a_task_added_and_completed_comes_back_with_undo(self):
        script = self.run_script(
            [
                ("menu", "Add a task"),
                ("input", "buy milk tomorrow"),
                ("menu", "Tasks"),
                ("key", "buy milk", "c"),
                ("back",),
                ("menu", "Undo"),
                ("back",),
            ],
            lambda: menus.main_menu(self.session),
        )
        self.assertTrue(any("buy milk" in said for said in script.said), script.said)
        self.assertTrue(any(said.startswith("Undid") for said in script.said), script.said)
        self.assertIn("buy milk", self.titles())

    def test_the_edit_form_sends_only_what_changed_and_keeps_a_repetition(self):
        self.call("project.add", name="Home")
        self.call("task.add", text="water plants every monday")
        self.run_script(
            [
                ("context", "water plants", "Edit"),
                ("form", {"due": "2026-12-10", "priority": "1", "project": "Home", "labels": "garden"}),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        shown = self.task("water plants")
        self.assertEqual(shown["due"], "2026-12-10")
        self.assertEqual(shown["repetition"], "every monday")
        self.assertEqual((shown["priority"], shown["project"], shown["labels"]), (1, "Home", ["garden"]))

    def test_the_edit_form_offers_the_projects_with_their_level_and_a_one_line_title(self):
        self.call("project.add", name="Work")
        self.call("project.add", name="Reports", parent="Work")
        self.call("task.add", text="file the summary")
        script = self.run_script(
            [("context", "file the summary", "Edit"), ("form", {"project": "Reports"}), ("back",)],
            lambda: tasks.task_list(self.session),
        )
        fields = {f.key: f for f in script.forms[0]}
        self.assertEqual(fields["project"].field_type, "choice")
        self.assertEqual(list(fields["project"].choices.values()), ["Inbox", "Work", "Reports, level 2"])
        self.assertEqual(fields["title"].field_type, "text")
        self.assertEqual(self.task("file the summary")["project"], "Reports")

    def test_a_task_in_an_archived_project_keeps_it_on_the_project_list(self):
        self.call("project.add", name="Old")
        self.call("task.add", text="dusty #Old")
        self.call("project.archive", name="Old")
        script = self.run_script(
            [("context", "dusty", "Edit"), ("form", {"priority": "1"}), ("back",)],
            lambda: tasks.task_list(self.session),
        )
        fields = {f.key: f for f in script.forms[0]}
        self.assertEqual(list(fields["project"].choices), ["Inbox", "Old"])
        self.assertEqual(self.task("dusty")["project"], "Old")

    def test_a_task_can_wait_for_another_and_become_a_subtask(self):
        self.call("task.add", text="paint")
        self.call("task.add", text="buy paint")
        self.run_script(
            [
                ("context", "paint", "Wait for"),
                ("choose", "buy paint"),
                ("key", "buy paint", "s"),
                ("choose", "paint, Inbox"),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        self.assertEqual([d["title"] for d in self.task("paint")["depends"]], ["buy paint"])
        self.assertIsNotNone(self.task("buy paint")["parent"])

    def test_the_delete_key_trashes_and_the_trash_restores_or_erases(self):
        self.call("task.add", text="keep me")
        self.call("task.add", text="lose me")
        self.run_script(
            [
                ("menu", "keep me", "delete"),
                ("menu", "lose me", "delete"),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        self.assertEqual(self.titles(), [])
        self.run_script(
            [
                ("menu", "keep me"),
                ("menu", "lose me", "delete"),
                ("confirm", True),
                ("back",),
            ],
            lambda: tasks.trash(self.session),
        )
        self.assertEqual(self.titles(), ["keep me"])
        self.assertEqual(self.titles("deleted"), [])

    # -- the day ----------------------------------------------------------------------

    def test_a_day_adds_a_block_assigns_a_task_and_times_it(self):
        self.call("task.add", text="write the chapter")
        script = self.run_script(
            [
                ("app", "Add a block"),
                ("form", {"title": "Deep work", "start": "9am", "minutes": "90"}),
                ("key", "Deep work", "i"),
                ("choose", "write the chapter"),
                ("input", ""),
                ("key", "write the chapter", "s"),
                ("context", "write the chapter", "Log minutes"),
                ("input", "25"),
                ("back",),
            ],
            lambda: day.day_plan(self.session),
        )
        plan = self.call("plan")
        sitting = plan["blocks"][0]["assignments"][0]
        self.assertEqual((sitting["title"], sitting["minutes"]), ("write the chapter", 25))
        self.assertIn("1 task assigned", script.titles[-1])

    def test_a_sitting_is_given_a_length_when_assigned_and_can_change_it(self):
        self.call("task.add", text="draft")
        self.call("block.add", title="Focus", at="9am", minutes=120, date="today")
        script = self.run_script(
            [
                ("key", "draft", "b"),
                ("choose", "Focus"),
                ("input", "45"),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        self.assertEqual(self.call("plan")["blocks"][0]["assignments"][0]["planned_mins"], 45)
        script = self.run_script(
            [
                ("key", "planned for 45 minutes", "l"),
                ("input", ""),
                ("back",),
            ],
            lambda: day.day_plan(self.session),
        )
        self.assertIsNone(self.call("plan")["blocks"][0]["assignments"][0].get("planned_mins"))
        self.assertTrue(any("Cleared" in said for said in script.said), script.said)

    def test_a_task_goes_in_a_block_of_the_week_or_of_a_day_named(self):
        self.call("task.add", text="draft")
        self.call("task.add", text="review")
        self.call("block.add", title="Focus", at="9am", minutes=90, date="tomorrow")
        self.call("block.add", title="Lunch", at="noon", minutes=30, kind="break", date="tomorrow")
        self.call("block.add", title="Retreat", at="10am", minutes=60, date="in 10 days")
        script = self.run_script(
            [
                ("key", "draft", "b"),
                ("choose", "Focus"),
                ("input", ""),
                ("key", "review", "b"),
                ("choose", "Another day"),
                ("input", "in 10 days"),
                ("choose", "Retreat"),
                ("input", ""),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        offered = script.offered[0]
        self.assertEqual(len(offered), 2, offered)
        self.assertIn("09:00 to 10:30, Focus", offered[0])
        self.assertEqual(offered[1], "Another day")
        self.assertEqual(self.call("plan", date="tomorrow")["blocks"][0]["assignments"][0]["title"], "draft")
        self.assertEqual(self.call("plan", date="in 10 days")["blocks"][0]["assignments"][0]["title"], "review")

    def test_a_sitting_pauses_resumes_and_stops(self):
        self.call("task.add", text="draft")
        self.call("block.add", title="Focus", at="00:00", minutes=1439, date="today")
        self.run_script(
            [
                ("key", "Focus", "i"),
                ("choose", "draft"),
                ("input", ""),
                ("key", "draft", "s"),
                ("context", "draft", "Pause timer"),
                ("context", "draft", "Resume timer"),
                ("context", "draft", "Pause timer"),
                ("context", "draft", "Stop timer"),
                ("back",),
            ],
            lambda: day.day_plan(self.session),
        )
        sitting = self.call("plan")["blocks"][0]["assignments"][0]
        self.assertEqual(sitting["status"], "worked")

    def test_a_break_let_take_tasks_is_offered_for_them(self):
        self.call("task.add", text="read")
        self.call("block.add", title="Train", at="00:00", minutes=1439, date="today", kind="break")
        self.run_script(
            [
                ("context", "Train", "Edit block"),
                ("form", {"accepts_tasks": "yes"}),
                ("key", "Train", "i"),
                ("choose", "read"),
                ("input", ""),
                ("back",),
            ],
            lambda: day.day_plan(self.session),
        )
        block = self.call("plan")["blocks"][0]
        self.assertTrue(block["accepts_tasks"])
        self.assertIn("takes tasks", block["details"])
        self.assertEqual(block["assignments"][0]["title"], "read")

    def test_a_task_is_read_back_before_it_is_added_when_asked_for(self):
        import options
        options.put(options.READ_BACK, True)
        script = self.run_script(
            [
                ("app", "Add a task"),
                ("input", "call mum tomorrow"),
                ("choose", "Change it"),
                ("input", "call mum tomorrow p1"),
                ("choose", "Add it"),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        self.assertTrue(any("priority 1" in asked for asked in script.prompts), script.prompts)
        self.assertEqual(self.titles(), ["call mum"])

    def test_one_day_of_a_repeating_block_is_changed_cancelled_and_put_back(self):
        self.call("block.add", title="Run", at="7am", minutes=30, repeat="every day", date="today")
        self.run_script(
            [
                ("menu", "Run"),
                ("choose", "only"),
                ("form", {"minutes": "45"}),
                ("key", "Run", "x"),
                ("menu", "cancelled for this day"),
                ("back",),
            ],
            lambda: day.day_plan(self.session),
        )
        today = self.call("plan")
        self.assertEqual(today["blocks"][0]["title"], "Run")
        self.assertEqual(today.get("cancelled", []), [])

    def test_the_planner_turns_to_another_day(self):
        self.call("block.add", title="Dentist", at="2pm", minutes=60, kind="event", date="tomorrow")
        script = self.run_script(
            [("key", "Now", "n"), ("back",)],
            lambda: day.day_plan(self.session),
        )
        self.assertTrue(any("1 block" in title for title in script.titles[1:]), script.titles)

    def test_a_block_series_is_edited_for_every_occurrence_and_deleted_with_a_question(self):
        self.call("block.add", title="Standup", at="9am", minutes=15, repeat="every weekday", date="today")
        self.run_script(
            [
                ("menu", "Standup"),
                ("form", {"repeat": "every monday"}),
                ("menu", "every monday", "delete"),
                ("confirm", True),
                ("back",),
            ],
            lambda: day.blocks(self.session),
        )
        self.assertEqual(self.call("block.list")["rows"], [])

    def test_the_day_says_free_time_now_and_a_cancelled_day_in_the_cores_order(self):
        added = self.call("block.add", title="Run", at="7am", minutes=30, repeat="every day", date="today")
        series = self.call("block.list")["rows"][0]["id"].split("@")[0]
        self.call("block.cancel", id=series, date="tomorrow")
        tomorrow = day.plan_rows(self.call("plan", date="tomorrow"))
        cancelled = next(row for row in tomorrow if row["role"] == "cancelled")
        self.assertEqual(day.describe(cancelled), "07:00, Run, cancelled for this day")
        free = next(row for row in tomorrow if row["role"] == "free")
        self.assertRegex(day.describe(free), r"^Free, \d+ hours?( \d+ minutes?)?, \d\d:\d\d to \d\d:\d\d$")
        now = next(row for row in day.plan_rows(self.call("plan")) if row["role"] == "now")
        self.assertRegex(now["title"], r"^Now, \d\d:\d\d$")
        self.assertTrue(added)

    def test_the_block_form_names_its_fields_and_kinds_as_the_core_does(self):
        script = self.run_script(
            [("app", "Add a block"), ("form", None), ("back",)],
            lambda: day.day_plan(self.session),
        )
        fields = {f.key: f for f in script.forms[0]}
        self.assertEqual(list(fields), ["title", "date", "start", "minutes", "kind", "repeat"])
        self.assertEqual(fields["minutes"].prompt, "Lasts, in minutes")
        self.assertEqual(fields["kind"].choices, {"work": "Work", "break": "Break", "event": "Event"})
        self.assertEqual(fields["repeat"].format_hint, "Such as every weekday. Empty for once.")

    def test_a_block_form_for_one_day_asks_only_what_one_day_can_change(self):
        self.call("block.add", title="Run", at="7am", minutes=30, repeat="every day", date="today")
        script = self.run_script(
            [("menu", "Run"), ("choose", "only"), ("form", None), ("back",)],
            lambda: day.day_plan(self.session),
        )
        self.assertEqual(
            [f.prompt for f in script.forms[0]],
            ["Name", "Starts at", "Lasts, in minutes", "Kind", "Takes tasks", "Counts toward hours for work",
             "Anchored, never moved when the day slips"],
        )

    def test_a_rule_the_words_cannot_say_is_noted_at_repeats(self):
        self.call("block.add", title="Board", at="6pm", minutes=60, repeat="every day", date="today")
        real = self.session.call

        def call(method, **params):
            result = real(method, **params)
            if method == "block.show":
                # As a rule from an import or another app would arrive: no words for it.
                result = {**result, "rrule": "FREQ=MONTHLY;BYDAY=2TU"}
                result.pop("repetition", None)
            return result

        self.session.call = call
        script = self.run_script([("menu", "Board"), ("form", None), ("back",)], lambda: day.blocks(self.session))
        repeat = next(f for f in script.forms[0] if f.key == "repeat")
        self.assertEqual(
            repeat.format_hint,
            "It repeats by the rule FREQ=MONTHLY;BYDAY=2TU, which the repetition words cannot say. "
            "Leave Repeats empty to keep it.",
        )

    def test_an_empty_list_leaves_its_count_out_of_its_heading(self):
        script = self.run_script([("back",)], lambda: organise.labels(self.session))
        self.assertEqual(script.titles[0], "Labels")
        script = self.run_script([("back",)], lambda: preferences.devices(self.session))
        self.assertEqual(script.menus[0]["empty"], "Press p to pair one.")
        self.assertTrue(script.titles[0].startswith("Not paired"), script.titles)

    def test_an_empty_task_list_says_the_cores_words_then_the_key_that_adds(self):
        script = self.run_script([("back",)], lambda: tasks.task_list(self.session))
        self.assertEqual(script.menus[0]["empty"], "No open tasks. Press a to add one.")

    def test_saved_filters_and_devices_say_the_cores_words_when_empty(self):
        script = self.run_script([("back",)], lambda: organise.saved_filters(self.session))
        self.assertEqual(
            script.menus[0]["empty"],
            "No saved filters. A filter's query is kept here under a name. Press a to add one, or slash to filter now.",
        )

    def test_going_to_a_day_and_a_new_filter_ask_in_the_cores_words(self):
        script = self.run_script(
            [("app", "Go to a day"), ("input", "tomorrow"), ("back",)],
            lambda: day.day_plan(self.session),
        )
        self.assertIn("Go to day. A date, such as Friday, or 12 October", script.prompts)
        self.assertRegex(script.titles[-1], r"^\w+ \d+ \w+ \d{4}\. ")
        script = self.run_script(
            [("app", "New saved filter"), ("input", "Urgent"), ("input", "p1"), ("back",)],
            lambda: organise.saved_filters(self.session),
        )
        self.assertEqual([f["name"] for f in self.call("filter.list")["filters"]], ["Urgent"])
        self.assertEqual(script.prompts[:2], ["New saved filter. Name", "New saved filter. A filter, such as p1 & due before: friday. Query"])

    # -- organising -------------------------------------------------------------------

    def test_a_project_is_made_renamed_archived_and_unarchived(self):
        self.run_script(
            [
                ("app", "New project"),
                ("input", "Wrok"),
                ("key", "Wrok", "r"),
                ("input", "Work"),
                ("context", "Work", "Archive"),
                ("context", "archived", "Unarchive"),
                ("back",),
            ],
            lambda: organise.projects(self.session),
        )
        work = next(r for r in self.call("project.list")["rows"] if r["title"] == "Work")
        self.assertNotIn("archived", work["state"])

    def test_a_projects_own_task_list_adds_a_project_inside_it(self):
        self.call("project.add", name="Work")
        self.run_script(
            [
                ("menu", "Work"),
                ("app", "New project inside"),
                ("input", "Errands"),
                ("back",),
                ("back",),
            ],
            lambda: organise.projects(self.session),
        )
        rows = [(r["title"], r["depth"]) for r in self.call("project.list")["rows"]]
        self.assertIn(("Errands", 1), rows)
        self.assertEqual(rows.index(("Errands", 1)), rows.index(("Work", 0)) + 1, "listed under Work")

    def test_a_label_takes_a_colour_and_merges_into_another(self):
        self.call("task.add", text="ring the bank @calls")
        self.call("label.add", name="cals")
        self.run_script(
            [
                ("key", "calls", "c"),
                ("input", "teal"),
                ("key", "cals", "m"),
                ("choose", "calls"),
                ("back",),
            ],
            lambda: organise.labels(self.session),
        )
        rows = self.call("label.list")["rows"]
        self.assertEqual([(r["title"], r["value"]) for r in rows], [("calls", "1 open task, teal")])

    def test_a_filter_is_saved_requeried_and_deleted(self):
        self.call("task.add", text="urgent thing p1")
        self.run_script(
            [
                ("app", "New saved filter"),
                ("input", "Urgent"),
                ("input", "p1"),
                ("key", "Urgent", "q"),
                ("input", "p1 | p2"),
                ("menu", "Urgent", "delete"),
                ("confirm", True),
                ("back",),
            ],
            lambda: organise.saved_filters(self.session),
        )
        self.assertEqual(self.call("filter.list")["filters"], [])

    # -- the device's conventions -------------------------------------------------------

    def test_every_list_has_undo_on_u_and_redo_on_y_in_its_main_menu(self):
        self.call("task.add", text="something")
        screens = [
            lambda: tasks.task_list(self.session), lambda: tasks.trash(self.session),
            lambda: day.day_plan(self.session), lambda: day.blocks(self.session),
            lambda: organise.projects(self.session), lambda: organise.labels(self.session),
            lambda: organise.saved_filters(self.session),
        ]
        for open_screen in screens:
            script = self.run_script([("back",)], open_screen)
            menu = script.menus[-1]
            labels = [command.get_label(None) for command in menu["app"]]
            self.assertIn("Undo, u", labels, script.titles[-1])
            self.assertIn("Redo, y", labels, script.titles[-1])
            self.assertTrue({"u", "y"} <= set(menu["keys"]), script.titles[-1])

    def test_a_tasks_context_menu_says_each_entrys_key(self):
        self.call("task.add", text="labelled")
        script = self.run_script([("back",)], lambda: tasks.task_list(self.session))
        menu = btspeak_stub._Menu([])
        row = self.call("task.list")["rows"][0]
        menu.menu = [type("Row", (), {"row": row})()]
        labels = [command.get_label(menu) for command in script.menus[-1]["context"] if command.applies(menu)]
        # The core's actions, in its order and under its names, then this app's Details.
        self.assertEqual(
            labels,
            ["Mark done, c", "Edit details, e", "Put in a block, b", "Move to project, m",
             "Make subtask of, s", "Wait for, w", "Move to trash", "Details"],
        )
        self.assertNotIn("Move to top level, t", labels, "only for a subtask")

    def test_an_empty_list_says_so_and_still_adds_from_its_main_menu(self):
        script = self.run_script(
            [("app", "New label"), ("input", "calls"), ("back",)],
            lambda: organise.labels(self.session),
        )
        self.assertEqual("No labels. Press a to add one.", script.menus[0]["empty"])
        self.assertEqual([r["title"] for r in self.call("label.list")["rows"]], ["calls"])

    def test_a_weight_typed_wrong_is_refused_in_the_cores_words(self):
        self.call("project.add", name="Work")
        script = self.run_script(
            [("key", "Work", "w"), ("input", "1,5"), ("key", "Work", "w"), ("input", "1.5"), ("back",)],
            lambda: organise.projects(self.session),
        )
        self.assertTrue(any("is not a weight" in said for said in script.said), script.said)
        work = next(r for r in self.call("project.list")["rows"] if r["title"] == "Work")
        self.assertIn("1.5", work["value"])

    def test_the_inbox_offers_only_what_the_core_gives_it(self):
        script = self.run_script([("back",)], lambda: organise.projects(self.session))
        menu = btspeak_stub._Menu([])
        inbox = self.call("project.list")["rows"][0]
        menu.menu = [type("Row", (), {"row": inbox})()]
        labels = [c.get_label(menu) for c in script.menus[-1]["context"] if c.applies(menu)]
        self.assertEqual(labels, ["Show its tasks", "Add a task to it, t", "Weight, w"])

    def test_a_letter_a_row_does_not_offer_says_why_in_the_cores_words(self):
        self.call("project.add", name="Work")
        script = self.run_script([("key", "Inbox", "r"), ("back",)], lambda: organise.projects(self.session))
        self.assertIn("The Inbox keeps its name and its place; only its order and weight change.", script.said)

    def test_a_project_with_a_space_opens_its_tasks_by_the_cores_reference(self):
        self.call("project.add", name="Home Office")
        self.call("task.add", text='file receipts #"Home Office"')
        script = self.run_script([("menu", "Home Office"), ("back",), ("back",)], lambda: organise.projects(self.session))
        self.assertTrue(any("1 task" in title for title in script.titles[1:]), script.titles)

    def test_a_pick_with_nothing_to_offer_says_why(self):
        self.call("task.add", text="alone")
        script = self.run_script([("key", "alone", "w"), ("back",)], lambda: tasks.task_list(self.session))
        self.assertIn("There is no task it could wait for.", script.said)

    def test_the_main_menu_lists_the_cores_places(self):
        script = self.run_script([("menu", "Saved filters"), ("back",), ("back",)], lambda: menus.main_menu(self.session))
        self.assertTrue(script.titles[1].startswith("Filters"), script.titles)

    def test_a_priority_is_chosen_by_the_cores_words(self):
        self.call("task.add", text="file taxes")
        self.run_script(
            [("context", "file taxes", "Edit details"), ("form", {"priority": "1"}), ("back",)],
            lambda: tasks.task_list(self.session),
        )
        self.assertEqual(self.task("file taxes")["priority"], 1)

    def test_a_backup_setting_offers_the_cores_options(self):
        script = self.run_script(
            [("menu", "Automatic backups, Every day"), ("choose", "Every week"), ("back",)],
            lambda: preferences.setting_page(self.session, "Backups", preferences.on_this_device),
        )
        self.assertEqual(script.offered[0], ["Every 12 hours", "Every day", "Every week", "Off"])
        self.assertEqual(self.call("config.get", key="backup-every")["settings"][0]["value"], "7d")

    # -- settings and data --------------------------------------------------------------

    def test_a_setting_with_few_values_is_a_choice(self):
        self.run_script(
            [("menu", "Announcements"), ("choose", "Terse"), ("back",)],
            lambda: preferences.setting_page(self.session, "Planning", preferences.planning),
        )
        self.assertEqual(self.call("config.get", key="verbosity")["settings"][0]["value"], "terse")

    def test_a_time_setting_shows_and_takes_hours_and_minutes(self):
        self.run_script(
            [("menu", "Day starts, 08:00"), ("input", "9:30am"), ("back",)],
            lambda: preferences.setting_page(self.session, "Planning", preferences.planning),
        )
        self.assertEqual(self.call("config.get", key="day-start")["settings"][0]["value"], "09:30")

    def test_an_export_goes_to_the_chosen_folder_and_imports_back(self):
        self.call("task.add", text="exported")
        folder = self.profile / "out"
        folder.mkdir()
        self.run_script(
            [("choose", "JSON"), ("directory", str(folder))],
            lambda: preferences.export(self.session),
        )
        written = list(folder.iterdir())
        self.assertEqual(len(written), 1, written)
        said = play([("file", str(written[0]))])
        self.assertTrue(preferences.import_file(self.session))
        self.assertTrue(said.finished())

    def test_the_pairing_code_is_copied_only_when_asked(self):
        btspeak_stub._clipboard[0] = "the person's own"
        # Waiting shows the code, and leaves the clipboard alone.
        script = play([("choose", "Wait for the other device"), ("wait",), ("back",)])
        preferences.pair(self.session)
        self.assertTrue(script.finished(), script.steps)
        self.assertEqual(btspeak_stub._clipboard[0], "the person's own")
        self.assertFalse(any("copied" in said for said in script.said), script.said)
        # Copy code puts it there.
        script = play([("choose", "Wait for the other device"), ("wait",), ("menu", "Copy code"), ("back",)])
        preferences.pair(self.session)
        self.assertTrue(script.finished(), script.steps)
        self.assertIn("The code is copied, so it can be pasted on the other device.", script.said)
        self.assertNotEqual(btspeak_stub._clipboard[0], "the person's own")

    def test_pairing_by_a_typed_code_compares_the_words_and_syncs(self):
        other_profile = Path(tempfile.mkdtemp())
        other = connect(other_profile)
        try:
            other.call("task.add", text="from the other device")
            waiting = other.begin("pair", local_only=True, name="laptop")
            code = other.pairing.get(timeout=30)["code"]

            def confirm_there():
                event = other.pairing.get(timeout=60)
                other.call("pair.confirm", match=bool(event.get("words")))

            threading.Thread(target=confirm_there, daemon=True).start()
            # Something else on the clipboard is never taken for the code.
            btspeak_stub._clipboard[0] = "not the code"
            script = play([
                ("choose", "Pair using this code"),
                ("input", code),
                ("wait",),
                ("confirm", True),
                ("wait",),
            ])
            said = preferences.pair(self.session)
            self.assertTrue(script.finished(), script.steps)
            self.assertIn("Paired with laptop", said)
            self.assertTrue(any(asked.startswith("Pair a device.") for asked in script.prompts), script.prompts)
            self.assertEqual(script.offered[0], ["Wait for the other device", "Pair using this code"])
            self.assertEqual(waiting.result(timeout=60)["result"], "paired")
            self.assertIn("from the other device", self.titles())
        finally:
            other.close()
            shutil.rmtree(other_profile, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
