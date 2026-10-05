"""The task list and its tree, as a screen reader is given them."""

import json
import unittest

from harness import Atspi, Session

SEED = [
    ("project", "add", "Work"),
    ("project", "add", "Reports", "--parent", "Work"),
    ("task", "add", "Buy milk"),
    ("task", "add", "Write the report p1 #Work"),
    ("task", "add", "Outline #Work"),
    ("task", "add", "Find sources #Work"),
]


class TaskListTest(unittest.TestCase):
    def setUp(self):
        self.session = Session(SEED)
        ids = {row["title"]: row["id"] for row in json.loads(self.session.lum("task", "list", "--json"))["rows"]}
        self.ids = ids
        self.session.lum("task", "move", ids["Outline"], "--parent", ids["Write the report"])
        self.session.lum("task", "move", ids["Find sources"], "--parent", ids["Write the report"])
        # Another process wrote those; the app notices within a second.
        self.session.press("Control+2", wait=1.5)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_the_places_are_a_tree_with_levels_and_positions_among_siblings(self):
        self.session.press("Shift+F6", "Down", "Down")
        self.assertEqual(self.session.focus(), "[tree item] 'Inbox, 1 open task' level 2 1 of 2")
        self.session.press("Down")
        self.assertEqual(self.session.focus(), "[tree item] 'Work, 3 open tasks' level 2 2 of 2 expanded")

    def test_a_subtask_is_a_level_deeper_and_counted_among_its_own_siblings(self):
        self.assertEqual(self.session.focus(), "[tree item] 'Buy milk' level 1 1 of 2")
        self.session.press("Down")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 2 of 2 expanded")
        self.session.press("Right")
        self.assertEqual(self.session.focus(), "[tree item] 'Outline, subtask' level 2 1 of 2")

    def test_left_collapses_then_goes_to_the_parent_and_right_expands(self):
        self.session.press("Down", "Down")
        self.session.press("Left")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 2 of 2 expanded")
        self.session.press("Left")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 2 of 2 collapsed")
        self.session.press("Right")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 2 of 2 expanded")

    def test_space_completes_a_task_and_says_so_from_the_row_focus_moved_to(self):
        self.session.press("space")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 1 of 1 expanded")
        self.assertEqual(self.session.said(), ["Completed Buy milk"])

    def test_undo_puts_the_task_back_and_focus_stays_where_it_was(self):
        self.session.press("space", "Control+z")
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 2 of 2 expanded")
        self.assertEqual(self.session.said(), ["Completed Buy milk", "Undid: Completed Buy milk"])

    def test_enter_opens_the_details_and_escape_comes_back(self):
        self.session.press("Return")
        self.assertEqual(self.session.focus(), "[text] 'Title'")
        self.session.press("Escape")
        self.assertEqual(self.session.focus(), "[tree item] 'Buy milk' level 1 1 of 2")

    def test_saving_the_details_sends_what_changed(self):
        self.session.press("Return", "End")
        self.session.type(" today")
        self.session.press("Return")
        self.assertIn("Buy milk today", self.session.lum("task", "list"))

    def test_f6_goes_through_the_places_the_list_and_the_details(self):
        self.session.press("F6")
        self.assertEqual(self.session.focus(), "[text] 'Title'")
        self.session.press("F6")
        self.assertEqual(self.session.focus(), "[tree item] 'Tasks' level 1 2 of 7")
        self.session.press("F6")
        self.assertEqual(self.session.focus(), "[tree item] 'Buy milk' level 1 1 of 2")
        self.session.press("Shift+F6")
        self.assertEqual(self.session.focus(), "[tree item] 'Tasks' level 1 2 of 7")

    def test_the_filter_narrows_the_list_and_enter_says_what_it_found(self):
        self.session.press("Control+f")
        self.assertEqual(self.session.focus(), "[text] 'Filter'")
        self.session.type("p1")
        self.session.press("Return")
        # Only the parent matches, so it is listed without its subtasks.
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 1 of 1")
        said = self.session.said()
        self.assertEqual(len(said), 1)
        self.assertIn("1 task", said[0])

    def test_down_in_the_filter_offers_what_could_come_next(self):
        self.session.press("Control+f")
        self.session.press("Control+a", "BackSpace")
        self.session.type("#")
        self.session.press("Down")
        self.assertEqual(self.session.focus(), "[menu item] ''")
        self.session.press("Down", "Return")
        self.assertEqual(self.session.focus(), "[text] 'Filter'")
        self.assertEqual(Atspi.Text.get_text(self.session.focused(), 0, -1), "#Work")

    def test_a_change_from_another_process_appears_without_moving_focus(self):
        self.session.press("Down")
        self.session.lum("task", "add", "Call the bank")
        self.session.press("Shift", wait=2.5)
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 2 of 3 expanded")
        self.assertIn("Call the bank", [a.get_name() for a in self.session.find_all("tree item")])

    def test_delete_moves_a_task_to_the_trash(self):
        self.session.press("Delete")
        self.assertEqual(self.session.said(), ["Deleted Buy milk"])
        self.assertEqual(self.session.focus(), "[tree item] 'Write the report' level 1 1 of 1 expanded")


if __name__ == "__main__":
    unittest.main()
