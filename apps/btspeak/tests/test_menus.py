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
                ("menu", "buy milk"),
                ("choose", "Complete"),
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
                ("menu", "water plants"),
                ("choose", "Edit"),
                ("form", {"due": "2026-12-10", "priority": "1", "project": "Home", "labels": "garden"}),
                ("back",),
            ],
            lambda: tasks.task_list(self.session),
        )
        shown = self.task("water plants")
        self.assertEqual(shown["due"], "2026-12-10")
        self.assertEqual(shown["repetition"], "every monday")
        self.assertEqual((shown["priority"], shown["project"], shown["labels"]), (1, "Home", ["garden"]))

    def test_a_task_can_wait_for_another_and_become_a_subtask(self):
        self.call("task.add", text="paint")
        self.call("task.add", text="buy paint")
        self.run_script(
            [
                ("menu", "paint"),
                ("choose", "Wait for another task"),
                ("choose", "buy paint"),
                ("menu", "buy paint"),
                ("choose", "subtask"),
                ("choose", "paint"),
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
                ("choose", "Restore"),
                ("menu", "lose me"),
                ("choose", "Erase"),
                ("confirm", True),
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
                ("menu", "Add a block"),
                ("form", {"title": "Deep work", "at": "9am", "minutes": "90"}),
                ("menu", "Deep work"),
                ("choose", "Assign a task"),
                ("choose", "write the chapter"),
                ("menu", "write the chapter"),
                ("choose", "Start the timer"),
                ("menu", "write the chapter"),
                ("choose", "Log minutes"),
                ("input", "25"),
                ("back",),
            ],
            lambda: day.day_plan(self.session),
        )
        plan = self.call("plan")
        sitting = plan["blocks"][0]["assignments"][0]
        self.assertEqual((sitting["title"], sitting["minutes"]), ("write the chapter", 25))
        self.assertIn("1 task assigned", script.titles[-1])

    def test_one_day_of_a_repeating_block_is_changed_cancelled_and_put_back(self):
        self.call("block.add", title="Run", at="7am", minutes=30, repeat="every day", date="today")
        self.run_script(
            [
                ("menu", "Run"),
                ("choose", "Edit"),
                ("choose", "only"),
                ("form", {"minutes": "45"}),
                ("menu", "Run"),
                ("choose", "Cancel this day"),
                ("menu", "cancelled for this day"),
                ("choose", "Put this day back"),
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
            [("menu", "Next day"), ("menu", "Dentist"), ("choose", None), ("back",)],
            lambda: day.day_plan(self.session),
        )
        self.assertTrue(any("1 block" in title for title in script.titles[1:]), script.titles)

    def test_a_block_series_is_edited_for_every_occurrence_and_deleted_with_a_question(self):
        self.call("block.add", title="Standup", at="9am", minutes=15, repeat="every weekday", date="today")
        self.run_script(
            [
                ("menu", "Standup"),
                ("choose", "Edit every occurrence"),
                ("form", {"repeat": "every monday"}),
                ("menu", "every monday"),
                ("choose", "Delete"),
                ("confirm", True),
                ("back",),
            ],
            lambda: day.blocks(self.session),
        )
        self.assertEqual(self.call("block.list")["rows"], [])

    # -- organising -------------------------------------------------------------------

    def test_a_project_is_made_renamed_archived_and_unarchived(self):
        self.run_script(
            [
                ("menu", "Add a project"),
                ("input", "Wrok"),
                ("menu", "Wrok"),
                ("choose", "Rename"),
                ("input", "Work"),
                ("menu", "Work"),
                ("choose", "Archive"),
                ("menu", "archived"),
                ("choose", "Unarchive"),
                ("back",),
            ],
            lambda: organise.projects(self.session),
        )
        work = next(r for r in self.call("project.list")["rows"] if r["title"] == "Work")
        self.assertNotIn("archived", work["state"])

    def test_a_label_takes_a_colour_and_merges_into_another(self):
        self.call("task.add", text="ring the bank @calls")
        self.call("label.add", name="cals")
        self.run_script(
            [
                ("menu", "calls"),
                ("choose", "Colour"),
                ("input", "teal"),
                ("menu", "cals"),
                ("choose", "Merge"),
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
                ("menu", "Add a filter"),
                ("input", "Urgent"),
                ("input", "p1"),
                ("menu", "Urgent"),
                ("choose", "Change the query"),
                ("input", "p1 | p2"),
                ("menu", "Urgent"),
                ("choose", "Delete"),
                ("confirm", True),
                ("back",),
            ],
            lambda: organise.saved_filters(self.session),
        )
        self.assertEqual(self.call("filter.list")["filters"], [])

    # -- settings and data --------------------------------------------------------------

    def test_a_setting_with_few_values_is_a_choice(self):
        self.run_script(
            [("menu", "Announcements"), ("choose", "Terse"), ("back",)],
            lambda: preferences.setting_page(self.session, "Planning", preferences.PLANNING),
        )
        self.assertEqual(self.call("config.get", key="verbosity")["settings"][0]["value"], "terse")

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

    def test_pairing_by_code_compares_the_words_and_syncs(self):
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
            script = play([
                ("choose", "Type the code"),
                ("input", code),
                ("wait",),
                ("confirm", True),
                ("wait",),
            ])
            said = preferences.pair(self.session)
            self.assertTrue(script.finished(), script.steps)
            self.assertIn("Paired with laptop", said)
            self.assertEqual(waiting.result(timeout=60)["result"], "paired")
            self.assertIn("from the other device", self.titles())
        finally:
            other.close()
            shutil.rmtree(other_profile, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
