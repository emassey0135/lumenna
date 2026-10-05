"""Settings: each page applies a change as it is made, and says so."""

import unittest

from harness import Atspi, Session


class SettingsTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([])
        self.session.press("Control+comma")
        self.session.wait_for_window("Settings")
        self.session.said()

    def tearDown(self):
        self.session.close()

    def page(self, steps):
        self.session.press(*["Control+Page_Down"] * steps)

    def test_a_check_box_is_named_without_its_mnemonic_marker(self):
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[check box] 'Open Lumenna when you sign in, in the background'")

    def test_a_check_box_applies_at_once_and_says_so(self):
        self.page(1)
        self.session.press("Tab", "space")
        self.assertEqual(self.session.said(), ["Cascade-complete-subtasks is now false"])
        self.assertEqual(self.session.lum("config", "get", "cascade-complete-subtasks").strip(), "false")

    def test_a_text_setting_applies_when_it_is_left(self):
        self.page(1)
        self.session.press("Tab", "Tab")
        self.assertEqual(self.session.focus(), "[text] 'Day starts'")
        self.session.type("7:30")
        self.assertEqual(self.session.said(), [])
        self.session.press("Tab")
        self.assertEqual(self.session.said(), ["Day-start is now 7:30"])
        self.assertEqual(self.session.lum("config", "get", "day-start").strip(), "07:30")

    def test_a_setting_that_does_not_read_is_put_back(self):
        self.page(1)
        self.session.press("Tab", "Tab")
        self.session.type("whenever")
        self.session.press("Tab")
        self.session.wait_for_window("")
        self.session.press("Return")
        self.session.wait_for_window("Settings")
        self.assertEqual(self.session.lum("config", "get", "day-start").strip(), "08:00")

    def test_the_devices_page_says_the_sync_state_and_skips_an_empty_list(self):
        self.page(2)
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[text] 'Sync status'")
        status = self.session.focused()
        self.assertIn("Not paired", Atspi.Text.get_text(status, 0, -1))
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[button] 'Sync Now'")

    def test_escape_closes_settings(self):
        self.session.press("Escape")
        self.session.wait_for_window("Today – Lumenna")


if __name__ == "__main__":
    unittest.main()
