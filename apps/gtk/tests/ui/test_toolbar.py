"""The toolbar over the middle pane: the place shown's own actions, as the core gives them."""

import unittest

from harness import Atspi, Session

SEED = [
    ("task", "add", "Buy milk"),
]


class ToolbarTest(unittest.TestCase):
    def setUp(self):
        self.session = Session(SEED)

    def tearDown(self):
        self.session.close()

    def go(self, key):
        self.session.press(key, wait=1.5)
        self.session.said()

    def buttons(self):
        """The toolbar's buttons, in order, each with whether it does anything now."""
        toolbar = self.session.find("tool bar", "Commands")
        self.assertIsNotNone(toolbar, "no toolbar named Commands")
        found = []
        for index in range(toolbar.get_child_count()):
            child = toolbar.get_child_at_index(index)
            if child.get_role_name() == "button":
                found.append(child.get_name())
        return found

    def test_the_toolbar_holds_what_tasks_offers_of_its_own(self):
        self.go("Control+2")
        self.assertEqual(self.buttons(), ["New Task…", "Filter Tasks", "Undo", "Redo"])

    def test_the_toolbar_holds_what_today_offers_of_its_own(self):
        self.go("Control+1")
        self.assertEqual(
            self.buttons(),
            ["New Block…", "Previous Day", "Next Day", "Go to Day…", "Go to Now", "Time Left", "Undo", "Redo"],
        )

    def test_the_trash_offers_no_filter_and_its_toolbar_undo_and_redo(self):
        self.go("Control+4")
        self.assertEqual(self.buttons(), ["Undo", "Redo"])
        self.assertIsNone(self.session.find("text", "Filter"))

    def test_the_toolbar_is_one_tab_stop_whose_arrows_move_along_it(self):
        self.go("Control+1")
        self.session.press("Shift+Tab")
        self.assertEqual(self.session.focus(), "[button] 'New Block…'")
        self.session.press("Right")
        self.assertEqual(self.session.focus(), "[button] 'Previous Day'")
        self.session.press("End")
        self.assertEqual(self.session.focus(), "[button] 'Redo'")
        # Out of the toolbar on one Tab, back onto the button last used on one Shift+Tab.
        self.session.press("Tab")
        self.assertIn("[tree item]", self.session.focus())
        self.session.press("Shift+Tab")
        self.assertEqual(self.session.focus(), "[button] 'Redo'")

    def test_a_toolbar_button_runs_its_action(self):
        self.go("Control+1")
        self.session.press("Shift+Tab", "Right")
        self.assertEqual(self.session.focus(), "[button] 'Previous Day'")
        self.session.press("space", wait=1)
        said = self.session.said()
        self.assertEqual(len(said), 1)
        self.assertTrue(said[0].startswith("Yesterday"), said)
        self.assertIn("Yesterday – Lumenna", [w.get_name() for w in self.session.find_all("frame")])
        # Focus stays on the button pressed, to press again.
        self.assertEqual(self.session.focus(), "[button] 'Previous Day'")


if __name__ == "__main__":
    unittest.main()
