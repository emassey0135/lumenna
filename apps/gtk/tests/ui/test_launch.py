"""The desktop entry's actions: `--launch <id>`, the core's launch shortcuts, done by the app as it
starts, or by the copy already running when it is started again."""

import unittest

from harness import Session, pump


class LaunchTest(unittest.TestCase):
    def tearDown(self):
        self.session.close()

    def test_new_task_on_a_first_start_opens_quick_add(self):
        self.session = Session(arguments=("--launch", "new-task"))
        self.session.wait_for_window("New Task")
        self.assertEqual(self.session.focused().get_name(), "Task")

    def test_new_task_from_another_start_opens_quick_add_in_the_running_app(self):
        self.session = Session()
        pump(1)
        started = self.session.start_again("--launch", "new-task")
        self.assertEqual(started.returncode, 0, started.stderr)
        self.session.wait_for_window("New Task")
        self.session.press("Escape", wait=1)
        self.assertIn("Lumenna", self.session.active_window())

    def test_today_from_another_start_shows_the_day(self):
        self.session = Session()
        # Away from the day first: the app opens on it.
        self.session.press("Control+2", wait=1)
        self.assertNotIn("Today", self.session.active_window())
        started = self.session.start_again("--launch", "today")
        self.assertEqual(started.returncode, 0, started.stderr)
        self.session.wait_for_window("Today – Lumenna")


if __name__ == "__main__":
    unittest.main()
