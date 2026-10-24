"""Settings: each page applies a change as it is made, and says so."""

import json
import unittest
from pathlib import Path

from harness import Atspi, Session


class SettingsTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([])
        self.session.press("Control+comma")
        self.session.wait_for_window("Preferences")
        self.session.said()

    def tearDown(self):
        self.session.close()

    def page(self, steps):
        self.session.press(*["Control+Page_Down"] * steps)

    def test_settings_opens_on_its_tab_and_tab_comes_back_to_it(self):
        self.assertEqual(self.session.focus(), "[page tab] 'General' selected")
        self.session.press("Tab")
        self.assertIn("[check box]", self.session.focus())
        self.session.press("Shift+Tab")
        self.assertEqual(self.session.focus(), "[page tab] 'General' selected")

    def test_the_arrows_and_control_tab_go_from_page_to_page(self):
        self.session.press("Right")
        self.assertEqual(self.session.focus(), "[page tab] 'Planning' selected")
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[check box] 'Completing a task completes its subtasks' checked")
        self.session.press("Control+Tab")
        self.assertEqual(self.session.focus(), "[page tab] 'Devices' selected")
        self.session.press("Control+Shift+Tab")
        self.assertEqual(self.session.focus(), "[page tab] 'Planning' selected")
        self.session.press("Control+Page_Up")
        self.assertEqual(self.session.focus(), "[page tab] 'General' selected")

    def test_a_check_box_is_named_without_its_mnemonic_marker(self):
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[check box] 'Open Lumenna when you sign in, in the background'")

    def test_opening_at_sign_in_is_the_cores_setting_and_toggling_it_makes_the_entry_and_says_so(self):
        self.session.press("Tab")
        check = self.session.focused()
        self.assertEqual(check.get_name(), "Open Lumenna when you sign in, in the background")
        self.assertTrue(check.get_description().startswith("Lumenna keeps running after its window is closed"))
        autostart = Path(self.session.directory) / "config" / "autostart"
        self.session.press("space", wait=1)
        self.assertEqual(self.session.said(), ["Lumenna opens when you sign in"])
        (entry,) = autostart.glob("*.desktop")
        self.assertIn("--background", entry.read_text())
        self.assertIn("checked", self.session.focus())
        self.session.press("space", wait=1)
        self.assertEqual(self.session.said(), ["Lumenna no longer opens when you sign in"])
        self.assertEqual(list(autostart.glob("*.desktop")), [])
        self.assertNotIn("checked", self.session.focus())

    def test_opening_at_sign_in_that_cannot_be_made_says_why_and_stays_off(self):
        config = Path(self.session.directory) / "config"
        config.mkdir(exist_ok=True)
        (config / "autostart").write_text("a file where the folder would be")
        self.session.press("Tab")
        self.session.press("space", wait=1)
        (said,) = self.session.said()
        self.assertTrue(said.startswith("That could not be changed: "), said)
        self.assertNotIn("checked", self.session.focus())

    def test_a_check_box_applies_at_once_and_says_so(self):
        self.page(1)
        self.session.press("Tab", "space")
        self.assertEqual(self.session.said(), ["Completing a task completes its subtasks: Off"])
        self.assertEqual(self.session.lum("config", "get", "cascade-complete-subtasks").strip(), "false")

    def test_a_text_setting_applies_when_it_is_left(self):
        self.page(1)
        self.session.press("Tab", "Tab")
        self.assertEqual(self.session.focus(), "[text] 'Day starts'")
        self.session.type("7:30")
        self.assertEqual(self.session.said(), [])
        self.session.press("Tab")
        self.assertEqual(self.session.said(), ["Day starts: 7:30"])
        self.assertEqual(self.session.lum("config", "get", "day-start").strip(), "07:30")

    def test_a_setting_that_does_not_read_is_put_back(self):
        self.page(1)
        self.session.press("Tab", "Tab")
        self.session.type("whenever")
        self.session.press("Tab")
        self.session.wait_for_alert()
        self.session.press("Return")
        self.session.wait_for_window("Preferences")
        self.assertEqual(self.session.lum("config", "get", "day-start").strip(), "08:00")

    def test_the_devices_page_says_the_sync_state_and_skips_an_empty_list(self):
        self.page(2)
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[text] 'Sync status'")
        status = self.session.focused()
        self.assertIn("Not paired", Atspi.Text.get_text(status, 0, -1))
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[button] 'Sync Now'")

    def test_a_code_entered_while_waiting_gives_up_the_wait_and_joins(self):
        self.page(2)
        self.session.press("Tab", "Tab", "Tab")
        self.assertEqual(self.session.focus(), "[button] 'Pair a Device…'")
        self.session.press("Return")
        self.session.wait_for_window("Pair a Device")
        self.session.press("Return", wait=4)
        self.assertEqual(self.session.focus(), "[text] \"This device's code\"")
        self.session.said()
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[button] 'Copy Code'")
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[text] 'Code from the other device'")
        self.assertFalse(self.session.focused().get_description(), "nothing is said of the clipboard")
        self.session.type("notacode")
        self.session.press("Return", wait=4)
        said = self.session.said()
        self.assertEqual(said[0], "Stopping the wait, then connecting with this code.")
        self.assertIn("'notacode' is not a pairing code", said[-1])

    def test_an_empty_code_says_to_type_or_paste_one_and_reads_no_clipboard(self):
        self.page(2)
        self.session.press("Tab", "Tab", "Tab")
        self.session.press("Return")
        self.session.wait_for_window("Pair a Device")
        self.session.tab_to("Pair Using This Code")
        self.session.press("Return")
        self.session.wait_for_window("Type or paste the code the other device shows.")
        self.session.press("Return")
        self.session.wait_for_window("Pair a Device")
        self.assertEqual(self.session.focus(), "[text] 'Code from the other device'")

    def test_erase_everything_deleted_is_a_button_beside_import_that_asks_first_and_empties_the_trash(self):
        self.session.lum("task", "add", "Buy milk")
        task = json.loads(self.session.lum("task", "list", "--json"))["rows"][0]["id"]
        self.session.lum("task", "rm", task)
        self.page(4)
        self.session.tab_to("Import or Restore…")
        self.session.press("Tab")
        self.assertEqual(self.session.focus(), "[button] 'Erase Everything Deleted…'")
        # The core's caption is said with the button, and shown under it.
        caption = self.session.focused().get_description()
        self.assertTrue(caption.startswith("Takes what was deleted out of the history"), caption)
        self.assertIn(caption, [Atspi.Text.get_text(label, 0, -1) for label in self.session.find_all_in_front("label")])
        self.session.press("Return")
        # Over Settings, which is not the main window, so known by its heading.
        self.session.wait_for_window("Erase everything deleted permanently?")
        self.assertEqual(self.session.focus(), "[button] 'Cancel'", "Cancel is the default")
        self.session.tab_to("Erase Everything Deleted")
        self.session.press("Return", wait=2)
        self.session.wait_for_window("Preferences")
        said = self.session.said()
        self.assertTrue(said and said[0].startswith("Erased 1 task and the history of everything deleted"), said)
        self.assertNotIn("Buy milk", self.session.lum("task", "list", "deleted"))

    def test_escape_closes_settings(self):
        self.session.press("Escape")
        self.session.wait_for_window("Today – Lumenna")


if __name__ == "__main__":
    unittest.main()
