"""The planner, as a screen reader is given it."""

import json
import unittest

from harness import Session

# A block over the whole day is happening now whenever the tests run.
SEED = [
    ("task", "add", "Write the report"),
    ("block", "add", "All day", "--at", "00:00", "--minutes", "1439"),
]


class DayTest(unittest.TestCase):
    def setUp(self):
        self.session = Session(SEED)
        self.block = json.loads(self.session.lum("block", "list", "--json"))["rows"][0]["id"]
        task = json.loads(self.session.lum("task", "list", "--json"))["rows"][0]["id"]
        self.session.lum("assign", task, "--block", self.block, "--minutes", "45")
        self.session.press("Control+1", wait=1.5)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_the_day_opens_on_the_block_happening_now(self):
        focus = self.session.focus()
        self.assertIn("All day", focus)
        self.assertIn("now, 1 task assigned' level 1", focus)

    def test_the_summary_comes_first(self):
        self.session.press("Home")
        self.assertIn("1 block", self.session.focus())

    def test_a_sitting_sits_under_its_block(self):
        self.session.press("Down")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report, planned for 45 minutes' level 2 1 of 1")

    def test_space_on_a_sitting_starts_pauses_and_resumes_its_timer(self):
        self.session.press("Down", "space")
        self.assertEqual(self.session.said(), ["Started timer"])
        self.assertIn("in progress", self.session.focus())
        self.session.press("space")
        said = self.session.said()
        self.assertEqual(len(said), 1)
        self.assertTrue(said[0].startswith("Paused timer"), said)
        self.assertIn("paused", self.session.focus())
        self.session.press("space")
        self.assertEqual(self.session.said(), ["Resumed timer"])
        self.assertIn("in progress", self.session.focus())

    def test_stop_in_a_running_sittings_menu_ends_the_sitting(self):
        self.session.press("Down", "space", "Shift+F10")
        self.assertIn("[menu item]", self.session.focus())
        self.session.press("Down", "Return", wait=1)
        said = self.session.said()
        self.assertEqual(len(said), 2, said)
        self.assertNotIn("in progress", self.session.focus())
        self.assertNotIn("paused", self.session.focus())

    def edit_block(self):
        self.session.press("Return")
        self.session.wait_for_window("Change All day, Every Occurrence")

    def test_the_block_form_sends_a_flag_set_apart_from_the_kind(self):
        self.edit_block()
        self.session.press(*["Tab"] * 6)
        self.assertEqual(self.session.focus(), "[check box] 'Fixed in time, never moved when the day slips'")
        self.session.press("space", "Alt+v", wait=1)
        self.assertEqual(self.session.said(), ["Changed block All day"])
        self.assertIn("anchored", self.session.focus())
        self.assertIn("anchored: yes", self.session.lum("block", "show", self.block))

    def test_a_new_kind_brings_its_own_flags(self):
        self.edit_block()
        self.session.press("Tab", "Tab", "Tab", "space", "Down", "Return", "Tab")
        self.assertEqual(self.session.focus(), "[check box] 'Tasks can go here'", "a break takes no tasks")
        self.session.press("Alt+v", wait=1)
        shown = self.session.lum("block", "show", self.block)
        self.assertIn("kind: break", shown)
        self.assertIn("takes tasks: no", shown)

    def test_one_days_form_holds_only_what_a_day_can_change(self):
        self.session.lum("block", "edit", self.block, "--repeat", "every day")
        self.session.press("Shift", wait=1.5)
        self.session.press("Return")
        self.session.wait_for_window("")
        # Cancel is the default; "Today Only" is two before it.
        self.session.press("Shift+Tab", "Shift+Tab")
        self.assertEqual(self.session.focus(), "[button] 'Today Only'")
        self.session.press("Return")
        self.session.wait_for_window("Change All day, This Day Only")
        self.session.press(*["Tab"] * 6)
        self.assertEqual(self.session.focus(), "[check box] 'Fixed in time, never moved when the day slips'")
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[button] 'Cancel'", "no repetition, filter, colour or notes")
        self.session.press("Shift+Tab", "space", "Alt+v", wait=1)
        self.assertIn("changed for this day", self.session.focus())
        self.assertIn("anchored: no", self.session.lum("block", "show", self.block), "every other day as it was")

    def test_delete_on_a_sitting_takes_it_out_of_the_block(self):
        self.session.press("Down", "Delete")
        self.assertIn("nothing assigned", self.session.focus())
        self.assertEqual(len(self.session.said()), 1)

    def test_the_next_day_says_its_summary(self):
        self.session.press("Control+Page_Down")
        said = self.session.said()
        self.assertEqual(len(said), 1)
        self.assertTrue(said[0].startswith("Tomorrow"), said)
        self.assertIn("Tomorrow – Lumenna", [w.get_name() for w in self.session.find_all("frame")])

    def test_a_sitting_shows_its_task_in_the_details(self):
        self.session.press("Down", "F6")
        self.assertEqual(self.session.focus(), "[text] 'Title'")


if __name__ == "__main__":
    unittest.main()
