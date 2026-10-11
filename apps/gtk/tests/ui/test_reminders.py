"""Reminders: given as desktop notifications while the app runs, and set from a task's or
block's Reminders list."""

import datetime
import json
import unittest

from harness import Notifications, Session, pump


def ago(minutes):
    return datetime.datetime.now() - datetime.timedelta(minutes=minutes)


def utc(moment):
    return moment.astimezone(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class DeliveryTest(unittest.TestCase):
    def setUp(self):
        self.notifications = Notifications()
        self.addCleanup(self.notifications.stop)

    def start(self, due_minutes_ago, delivered_minutes_ago):
        # Named by its day, which after midnight can be yesterday.
        due = ago(due_minutes_ago)
        day = "today" if due.date() == datetime.date.today() else "yesterday"
        self.session = Session(
            [("config", "set", "default-task-reminders", "at due"),
             ("task", "add", f"Call the bank {day} at {due.strftime('%H:%M')}")],
            device_settings={"reminders-delivered": utc(ago(delivered_minutes_ago))},
        )
        self.addCleanup(self.session.close)

    def test_a_due_reminder_is_a_notification_with_the_cores_buttons_and_opens_its_task(self):
        self.start(due_minutes_ago=2, delivered_minutes_ago=4)
        number, shown = self.notifications.wait_for("Call the bank")
        self.assertIn("Call the bank", shown["body"])
        buttons = [label for label in shown["actions"] if label != ""]
        self.assertEqual(len(buttons), 2, shown["actions"])
        self.notifications.press(number, "")
        self.assertEqual(self.session.focus(), "[text] 'Title'")
        self.assertEqual(self.session.lum("task", "list").count("Call the bank"), 1)

    def test_dismissing_a_reminder_takes_its_notification_away(self):
        self.start(due_minutes_ago=2, delivered_minutes_ago=4)
        number, shown = self.notifications.wait_for("Call the bank")
        dismiss = next(label for label in shown["actions"] if label != "")
        self.notifications.press(number, dismiss)
        self.assertIn(number, self.notifications.closed)
        self.assertNotIn("Reminders:", open(f"{self.session.directory}/app.log").read())

    def test_what_came_while_nothing_ran_is_one_notification(self):
        self.start(due_minutes_ago=60, delivered_minutes_ago=120)
        _, shown = self.notifications.wait_for("Lumenna")
        self.assertIn("Call the bank", shown["body"])


class ListTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([("task", "add", "Call the bank tomorrow at 15:00")])
        self.session.press("Control+2", wait=1.5)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def open_reminders(self):
        # The menu's items read as nameless here, so Reminders is found by where the row's
        # actions put it: those that remove nothing come first, in order.
        row = json.loads(self.session.lum("task", "list", "--json"))["rows"][0]
        keeping = [a["kind"] for a in row["actions"] if not a.get("destructive")]
        self.session.press("Shift+F10")
        self.session.press(*["Down"] * keeping.index("reminders"))
        self.session.press("Return")
        self.session.wait_for_window("Reminders for Call the bank")
        # What the list holds is said once the window is in front.
        pump(1.5)
        self.assertEqual(self.session.said(), ["No reminders"])

    def test_reminders_are_added_by_a_phrase_and_a_refused_one_stays_in_its_dialog(self):
        self.open_reminders()
        self.assertEqual(self.session.focus(), "[button] 'Add Reminder…'", "an empty list is not a stop")
        self.session.press("Return")
        self.session.wait_for_window("New reminder for Call the bank")
        self.session.type("whenever")
        self.session.press("Return")
        self.session.wait_for_alert()
        self.session.press("Return")
        self.session.wait_for_window("New reminder for Call the bank")
        self.session.press("Control+a")
        self.session.type("30m before")
        self.session.press("Return", wait=1)
        self.session.wait_for_window("Reminders for Call the bank")
        said = self.session.said()
        self.assertEqual(len(said), 1, said)
        self.assertEqual(self.session.focus(), "[tree item] '30 minutes before it is due' level 1 1 of 1")

    def test_delete_on_a_reminder_removes_it(self):
        self.open_reminders()
        self.session.press("Return")
        self.session.wait_for_window("New reminder for Call the bank")
        self.session.type("at due")
        self.session.press("Return", wait=1)
        self.session.wait_for_window("Reminders for Call the bank")
        self.assertEqual(self.session.focus(), "[tree item] 'when it is due' level 1 1 of 1", "on the one added")
        self.session.said()
        self.session.press("Delete", wait=1)
        self.assertEqual(len(self.session.said()), 1)
        self.assertFalse(self.session.find_all_in_front("tree item"))
        self.assertEqual(self.session.focus(), "[button] 'Add Reminder…'")

    def test_escape_closes_the_list(self):
        self.open_reminders()
        self.session.press("Escape")
        self.session.wait_for_window("Tasks – Lumenna")


class BlockTest(unittest.TestCase):
    def test_a_block_offers_its_reminders_and_says_it_takes_the_defaults(self):
        session = Session([("config", "set", "default-block-reminders", "5m before start"),
                           ("block", "add", "Deep work", "--at", "09:00", "--minutes", "90")])
        self.addCleanup(session.close)
        session.press("Control+3", wait=1.5)
        # Edit Block, then Reminders.
        session.press("Shift+F10", "Down", "Return")
        session.wait_for_window("Reminders for Deep work")
        self.assertIn("5 minutes before", session.focus())
        pump(1.5)
        said = session.said()
        self.assertEqual(len(said), 1, said)


class SettingsTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([])
        self.session.press("Control+comma")
        self.session.wait_for_window("Preferences")
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_reminders_on_this_device_is_a_general_setting(self):
        self.session.press("Tab", "Tab")
        self.assertEqual(self.session.focus(), "[check box] 'Reminders on this device' checked")
        self.session.press("space")
        self.assertEqual(self.session.lum("config", "get", "reminders").strip(), "false")

    def test_the_default_reminders_are_saved_when_left(self):
        self.session.press("Control+Page_Down")
        self.session.press("Alt+r")
        self.assertEqual(self.session.focus(), "[text] 'Reminders for a task with a due date'")
        self.session.type("30m before")
        self.assertEqual(self.session.said(), [], "nothing is saved per keystroke")
        self.session.press("Tab")
        self.assertEqual(len(self.session.said()), 1)
        self.assertEqual(self.session.lum("config", "get", "default-task-reminders").strip(), "30m before due")


if __name__ == "__main__":
    unittest.main()
