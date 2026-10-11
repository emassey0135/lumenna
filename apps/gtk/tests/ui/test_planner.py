"""The planner's day: its own actions, Replan from Now, Plan the Day, why a choice is offered,
a task's history, the carry offer at a block's end, and how urgency is weighed."""

import datetime
import json
import unittest

from harness import Atspi, Notifications, Session, pump


def at(minutes):
    """A time of day `minutes` from now, `HH:MM`."""
    return (datetime.datetime.now() + datetime.timedelta(minutes=minutes)).strftime("%H:%M")


def today_holds(earliest, latest):
    """Whether every time from `earliest` to `latest` minutes from now falls today."""
    now = datetime.datetime.now()
    return all((now + datetime.timedelta(minutes=m)).date() == now.date() for m in (earliest, latest))


class DayActions:
    """Runs one of the day's own actions from its heading row's menu, found by where the
    core's order puts it: the menu's items read as nameless to AT-SPI here."""

    def run_day_action(self, kind, date="today"):
        actions = json.loads(self.session.lum("plan", date, "--json"))["actions"]
        keeping = [a["kind"] for a in actions if not a.get("destructive")]
        self.session.press("Home", "Shift+F10")
        self.session.press(*["Down"] * keeping.index(kind))
        self.session.press("Return")


class PlanTheDayTest(unittest.TestCase, DayActions):
    def setUp(self):
        self.session = Session([
            ("block", "add", "Morning", "--at", "09:00", "--minutes", "60", "--date", "tomorrow"),
            ("block", "add", "Afternoon", "--at", "14:00", "--minutes", "60", "--date", "tomorrow"),
            ("task", "add", "Write the report tomorrow p1 45m"),
        ])
        self.session.press("Control+1", wait=1.5)
        self.session.press("Control+Page_Down")
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_plan_the_day_says_its_plan_and_each_sitting_says_why(self):
        self.run_day_action("plan_day", "tomorrow")
        self.session.wait_for_window("Plan the Day")
        focus = self.session.focus()
        self.assertTrue(focus.startswith("[check box] 'Write the report goes in Morning at "), focus)
        self.assertTrue(focus.endswith(", for 45 minutes' checked"), focus)
        self.assertTrue(self.session.focused().get_description().startswith("Suggested because"))
        pump(1.5)
        said = self.session.said()
        self.assertEqual(len(said), 1, said)
        self.assertIn("Write the report", said[0])

    def test_a_sitting_turned_down_is_planned_again_elsewhere(self):
        self.run_day_action("plan_day", "tomorrow")
        self.session.wait_for_window("Plan the Day")
        pump(1.5)
        self.session.said()
        self.session.press("space")
        self.session.tab_to("Plan the Ones Checked")
        self.session.press("Return", wait=1)
        # Still open, on the plan the core makes next: the task in the other block.
        self.assertEqual(self.session.active_window(), "Plan the Day")
        self.assertIn("Afternoon", self.session.focus())
        self.assertIn("checked", self.session.focus())
        self.session.press("Tab")
        self.session.tab_to("Plan the Ones Checked")
        self.session.press("Return", wait=1)
        self.session.wait_for_window("Tomorrow – Lumenna")
        self.assertIn("Write the report", self.session.lum("plan", "tomorrow"))


class ReplanTest(unittest.TestCase, DayActions):
    def setUp(self):
        # Three blocks crowding the next half hour, and room after them, all of it today.
        if not today_holds(0, 90):
            self.skipTest("too near midnight for blocks still to come today")
        seed = [("config", "set", "day-start", "00:00"), ("config", "set", "day-end", "23:59")]
        seed += [("block", "add", name, "--at", at(start), "--minutes", "15")
                 for name, start in (("First", 3), ("Second", 10), ("Third", 17))]
        self.session = Session(seed)
        self.session.press("Control+1", wait=1.5)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_replan_says_its_proposal_and_shows_the_next_after_part_is_made(self):
        self.run_day_action("replan")
        self.session.wait_for_window("Replan from Now")
        first = self.session.focus()
        self.assertTrue(first.startswith("[check box]") and first.endswith("checked"), first)
        pump(1.5)
        said = self.session.said()
        self.assertEqual(len(said), 1, said)
        self.assertIn("moves from", said[0])
        # Leave the second change unmade: the core proposes the day again around it.
        self.session.press("Tab", "space")
        self.session.tab_to("Make the Changes Checked")
        self.session.press("Return", wait=1)
        self.assertEqual(self.session.active_window(), "Replan from Now")
        said = self.session.said()
        self.assertEqual(len(said), 1, said)
        self.assertTrue(said[0].startswith("Replanned"), said)
        self.assertTrue(self.session.focus().startswith("[check box]"))

    def test_with_nothing_to_change_it_says_so_and_offers_no_apply(self):
        for name in ("Second", "Third"):
            block = next(r for r in json.loads(self.session.lum("block", "list", "--json"))["rows"] if r["title"] == name)
            self.session.lum("block", "rm", block["id"])
        pump(1.5)
        self.run_day_action("replan")
        self.session.wait_for_window("Replan from Now")
        self.assertEqual(self.session.focus(), "[button] 'Cancel'")
        self.assertIsNone(self.session.find("button", "Make the Changes Checked"))
        pump(1.5)
        self.assertEqual(self.session.said(), ["Nothing needs to change: the rest of the day still fits."])


class AllDayTest(unittest.TestCase, DayActions):
    def setUp(self):
        self.session = Session([
            ("block", "add", "All day", "--at", "00:00", "--minutes", "1440"),
            ("task", "add", "Write the report p1 30m"),
            ("task", "add", "Call the bank 15m"),
        ])
        self.session.press("Control+1", wait=1.5)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_what_should_i_work_on_says_why_each_is_suggested(self):
        self.run_day_action("what_now")
        self.session.wait_for_window("What should I work on?")
        focus = self.session.focused()
        self.assertTrue(focus.get_name().startswith("Write the report"), focus.get_name())
        self.assertNotIn("Suggested", focus.get_name(), "the reason is never in the name")
        self.assertTrue(focus.get_description().startswith("Suggested because"), focus.get_description())
        self.session.tab_to("Start")
        self.session.press("Return", wait=1)
        self.assertEqual(len(self.session.said()), 1)

    def test_a_tasks_history_is_under_its_own_heading_in_its_details(self):
        self.session.press("Control+2", wait=1)
        self.session.press("Return")
        history = self.session.find("text", "History")
        self.assertIsNotNone(history)
        self.assertIn("made today", Atspi.Text.get_text(history, 0, -1))


def assign_to_deep_work(session):
    block = next(r for r in json.loads(session.lum("block", "list", "--json"))["rows"] if r["title"] == "Deep work")
    task = json.loads(session.lum("task", "list", "--json"))["rows"][0]["id"]
    session.lum("assign", task, "--block", block["id"], "--minutes", "30")


class CarryTest(unittest.TestCase):
    def setUp(self):
        # A block that ended two minutes ago, with its sitting left short; after midnight it
        # began yesterday.
        start = datetime.datetime.now() - datetime.timedelta(minutes=62)
        day = "today" if start.date() == datetime.date.today() else "yesterday"
        self.notifications = Notifications()
        self.addCleanup(self.notifications.stop)
        seed = [
            ("config", "set", "day-start", "00:00"),
            ("config", "set", "default-block-reminders", "at end"),
            ("block", "add", "Deep work", "--at", start.strftime("%H:%M"), "--minutes", "60", "--date", day),
            ("block", "add", "Tomorrow's work", "--at", "09:00", "--minutes", "60", "--date", "tomorrow"),
            ("task", "add", "Write the report 30m"),
            assign_to_deep_work,
        ]
        self.session = Session(seed, device_settings={
            "reminders-delivered": (datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(minutes=4))
            .strftime("%Y-%m-%dT%H:%M:%SZ"),
        })
        self.addCleanup(self.session.close)

    def test_a_blocks_end_offers_to_carry_forward_what_was_left_short(self):
        number, shown = self.notifications.wait_for("Deep work")
        self.assertIn("Carry Forward", shown["actions"], shown)
        self.notifications.press(number, "Carry Forward")
        pump(1)
        self.assertIn("Write the report", self.session.lum("plan", "tomorrow"))


class UrgencyTest(unittest.TestCase):
    def test_how_urgency_is_weighed_is_a_planning_setting(self):
        session = Session([])
        self.addCleanup(session.close)
        session.press("Control+comma")
        session.wait_for_window("Preferences")
        session.press("Control+Page_Down", "Alt+u")
        self.assertEqual(session.focus(), "[text] 'How urgency is weighed'")


if __name__ == "__main__":
    unittest.main()
