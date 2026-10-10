"""The places: projects, labels and saved filters, and what can be done to them."""

import unittest

from harness import Session

SEED = [
    ("project", "add", "Work"),
    ("task", "add", "Write the report #Work"),
    ("label", "add", "calls"),
    ("label", "add", "phone"),
    ("filter", "add", "Urgent", "p1"),
]


class SidebarTest(unittest.TestCase):
    def setUp(self):
        self.session = Session(SEED)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def go_to(self, text):
        """Moves down the places to the first whose line starts with `text`."""
        for _ in range(20):
            if f"'{text}" in self.session.focus():
                return
            self.session.press("Down", wait=0.2)
        self.fail(f"no place {text!r}")

    def test_a_new_project_from_the_projects_heading_is_gone_to(self):
        self.go_to("Projects")
        self.session.press("Shift+F10")
        self.assertIn("[menu item]", self.session.focus())
        self.session.press("Return")
        self.session.wait_for_window("New Project")
        self.assertEqual(self.session.focus(), "[text] 'Name'")
        self.session.type("Home")
        self.session.press("Return", wait=1)
        self.assertEqual(self.session.said(), ["Added project Home"])
        self.assertEqual(self.session.active_window(), "Home – Lumenna")
        self.assertIn("Home", self.session.lum("project", "list"))

    def test_a_blank_name_is_refused_and_said(self):
        self.go_to("Projects")
        self.session.press("Shift+F10", "Return")
        self.session.wait_for_window("New Project")
        self.session.press("Return")
        # The refusal is said in an alert.
        self.session.wait_for_alert()
        self.assertTrue(self.session.lum("project", "list").startswith("2 projects"))

    def test_renaming_a_project_keeps_it_shown(self):
        self.go_to("Work")
        self.session.press("Menu")
        self.session.press("Return")
        self.session.wait_for_window("Rename Work")
        self.session.press("Control+a")
        self.session.type("Job")
        self.session.press("Return", wait=1)
        self.assertEqual(self.session.active_window(), "Job – Lumenna")
        self.assertIn("'Job, 1 open task' level 2", self.session.focus())

    def test_a_weight_that_does_not_read_is_refused_and_asked_again(self):
        self.go_to("Work")
        self.session.press("Menu")
        # Rename, New Project Inside, Move Under, Move Up (it is below the Inbox), Weight.
        self.session.press(*["Down"] * 4)
        self.session.press("Return")
        self.session.wait_for_window("Weight of Work")
        self.session.press("Control+a")
        self.session.type("1,5")
        self.session.press("Return")
        self.session.wait_for_alert()
        self.session.press("Return")
        self.session.wait_for_window("Weight of Work")
        self.session.press("Control+a")
        self.session.type("1.5")
        self.session.press("Return", wait=1)
        said = self.session.said()
        self.assertEqual(len(said), 1, said)
        self.assertIn("1.5", said[0])

    def test_delete_on_a_label_asks_first_and_cancel_is_the_default(self):
        self.go_to("calls")
        self.session.press("Delete")
        self.session.wait_for_alert()
        self.session.press("Return")
        self.assertIn("calls", self.session.lum("label", "list"))

    def test_delete_on_a_label_deletes_it_when_confirmed(self):
        self.go_to("calls")
        self.session.press("Delete")
        self.session.wait_for_alert()
        self.assertEqual(self.session.focus(), "[button] 'Cancel'", "Cancel is the default")
        self.session.tab_to("Delete Label")
        self.session.press("Return", wait=1)
        self.assertNotIn("calls", self.session.lum("label", "list"))
        self.assertEqual(self.session.said(), ["Deleted label calls; tasks that wore it are unchanged"])

    def test_the_fixed_places_have_no_delete(self):
        self.go_to("Tasks")
        self.session.press("Delete", wait=1)
        self.assertEqual(self.session.active_window(), "Tasks – Lumenna")


if __name__ == "__main__":
    unittest.main()
