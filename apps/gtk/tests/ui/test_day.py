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
        block = json.loads(self.session.lum("block", "list", "--json"))["rows"][0]["id"]
        task = json.loads(self.session.lum("task", "list", "--json"))["rows"][0]["id"]
        self.session.lum("assign", task, "--block", block, "--minutes", "45")
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

    def test_space_on_a_sitting_starts_and_stops_its_timer(self):
        self.session.press("Down", "space")
        self.assertEqual(self.session.said(), ["Started timer"])
        self.assertIn("in progress", self.session.focus())
        self.session.press("space")
        self.assertEqual(len(self.session.said()), 1)
        self.assertNotIn("in progress", self.session.focus())

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
