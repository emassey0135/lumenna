"""A row's level, said in its text where GTK cannot report it (before GTK 4.16)."""

import json
import unittest

from harness import Session
from test_tasks import SEED


class LevelsInTextTest(unittest.TestCase):
    """GTK 4.16 and later report the level themselves, which every other test checks; this
    forces the way older GTK is read."""

    def setUp(self):
        self.session = Session(SEED + [("task", "add", "Call the bank")], environment={"LUMENNA_LEVEL_IN_TEXT": "1"})
        ids = {row["title"]: row["id"] for row in json.loads(self.session.lum("task", "list", "--json"))["rows"]}
        self.session.lum("task", "move", ids["Outline"], "--parent", ids["Write the report"])
        self.session.lum("task", "move", ids["Find sources"], "--parent", ids["Write the report"])
        self.session.press("Control+2", wait=1.5)

    def tearDown(self):
        self.session.close()

    def names(self):
        return [row.get_name() for row in self.session.find_all("tree item") if "Lumenna" not in row.get_name()]

    def test_the_level_is_said_only_where_it_changes(self):
        names = self.names()
        self.assertIn("Outline, subtask, level 2", names, "deeper than the row before")
        self.assertIn("Find sources, subtask", names, "the same level as the row before")
        self.assertIn("Call the bank, level 1", names, "back out")
        self.assertIn("Write the report, priority 1", names, "the same as the row before, at the top")
        self.assertIn("Inbox, 2 open tasks, level 2", names, "the sidebar too")

    def test_collapsing_says_the_level_against_the_new_row_before(self):
        self.session.press("Down", "Left")
        self.assertIn("Call the bank", self.names(), "now straight after its sibling")
        self.assertNotIn("Call the bank, level 1", self.names())
        self.session.press("Right")
        self.assertIn("Call the bank, level 1", self.names())


if __name__ == "__main__":
    unittest.main()
