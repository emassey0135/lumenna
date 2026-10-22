"""The planner's day: its own actions, Replan from Now, Plan the Day, why a choice is offered,
a task's history, the carry offer at a block's end, and how urgency is weighed."""

import datetime
import json
import unittest

from harness import Atspi, Notifications, Session, pump


class DayActions:
    """Runs one of the day's own actions from its heading row's menu, found by where the
    core's order puts it: the menu's items read as nameless to AT-SPI here."""

    def run_day_action(self, kind, date="today"):
        # The item is found by its title, not counted: lum's clock is the real one, and the
        # app's is pinned, so the two can offer different actions around a block's edge. A
        # menu item has no name of its own: Orca reads the label it is labelled by.
        actions = json.loads(self.session.lum("plan", date, "--json"))["actions"]
        title = next(a["title"] for a in actions if a["kind"] == kind)
        self.session.press("Home", "Shift+F10")
        for _ in range(12):
            if self.session.menu_item().startswith(title):
                break
            self.session.press("Down", wait=0.2)
        else:
            self.fail(f"no {title!r} in the day's menu")
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
        # The rest of today from the real now, since the day offers Replan from Now only while
        # a block is under way or to come by the clock, pinned through LUMENNA_NOW as the
        # other apps pass now through so the proposal holds while the test runs: three blocks
        # crowding the next half hour.
        now = day_around_now(self, after=35)
        seed = [("config", "set", "day-start", "00:00"), ("config", "set", "day-end", "23:59")]
        seed += [("block", "add", name, "--at", at(now, minutes), "--minutes", "15")
                 for name, minutes in (("First", 3), ("Second", 10), ("Third", 17))]
        self.session = Session(seed, environment={"LUMENNA_NOW": now.strftime("%Y-%m-%dT%H:%M")})
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


def wait_out_midnight():
    """Waits out the two minutes before midnight: a pinned now keeps the clock's date."""
    while datetime.datetime.now() + datetime.timedelta(minutes=2) > datetime.datetime.combine(
            datetime.date.today() + datetime.timedelta(days=1), datetime.time()):
        pump(5)


def day_around_now(test, before=0, after=0):
    """The real now to the minute, for blocks the day offers its actions over, which are read
    by the clock. A day holds no block past midnight, so a test whose blocks would cross it
    skips: waiting out as much as an hour and a half is no better."""
    now = datetime.datetime.now().replace(second=0, microsecond=0)
    midnight = datetime.datetime.combine(now.date(), datetime.time())
    if now - datetime.timedelta(minutes=before) < midnight \
            or now + datetime.timedelta(minutes=after) >= midnight + datetime.timedelta(days=1):
        test.skipTest("its blocks would cross midnight")
    return now


def at(now, minutes):
    return (now + datetime.timedelta(minutes=minutes)).strftime("%H:%M")


class OtherOptionsTest(unittest.TestCase, DayActions):
    def setUp(self):
        # At the real now, pinned as in ReplanTest, Deep work runs into Lunch, which pushes
        # Review: shortening or skipping either is another option.
        now = day_around_now(self, before=20, after=85)
        self.session = Session([
            ("config", "set", "day-start", "00:00"),
            ("block", "add", "Deep work", "--at", at(now, -20), "--minutes", "40"),
            ("block", "add", "Lunch", "--at", at(now, -10), "--minutes", "30", "--kind", "break"),
            ("block", "add", "Review", "--at", at(now, 20), "--minutes", "60"),
        ], environment={"LUMENNA_NOW": now.strftime("%Y-%m-%dT%H:%M")})
        self.session.press("Control+1", wait=1.5)
        self.session.said()

    def tearDown(self):
        self.session.close()

    def test_another_option_is_proposed_in_place_and_said(self):
        self.run_day_action("replan")
        self.session.wait_for_window("Replan from Now")
        self.assertIsNotNone(self.session.find("heading", "Other options"))
        pump(1.5)
        self.session.said()
        skip = "Skip Lunch today, your only break left today. Then nothing else needs to change."
        self.session.tab_to(skip, most=12)
        self.session.press("Return", wait=1)
        self.assertEqual(self.session.said(), ["Lunch is skipped today."])
        self.assertEqual(self.session.focus(), "[check box] 'Lunch is skipped today' checked")
        self.assertIsNone(self.session.find("heading", "Other options"), "nothing does better")


class OverrunTest(unittest.TestCase):
    def setUp(self):
        # At a pinned 10:00, a block begun at 9:57 holds forty minutes of work in thirty, a
        # meeting straight after: its work outruns its time from the start. The app gives
        # reminders from LUMENNA_NOW too, last given four minutes before it.
        wait_out_midnight()
        now = datetime.datetime.combine(datetime.date.today(), datetime.time(10, 0))
        self.notifications = Notifications()
        self.addCleanup(self.notifications.stop)
        self.session = Session([
            ("config", "set", "day-start", "00:00"),
            ("block", "add", "Deep work", "--at", "09:57", "--minutes", "30"),
            ("block", "add", "Meeting", "--at", "10:27", "--minutes", "60", "--kind", "event"),
            ("task", "add", "Write the report"),
            assign_forty_minutes,
        ], environment={"LUMENNA_NOW": now.strftime("%Y-%m-%dT%H:%M")}, device_settings={
            "reminders-delivered": (now - datetime.timedelta(minutes=4)).astimezone(datetime.timezone.utc)
            .strftime("%Y-%m-%dT%H:%M:%SZ"),
        })
        self.addCleanup(self.session.close)

    def test_an_overrun_warning_offers_replan_which_opens_replan_from_now(self):
        number, shown = self.notifications.wait_for("Deep work")
        self.assertIn("more than it holds", shown["body"])
        self.assertIn("Replan", shown["actions"], shown)
        self.notifications.press(number, "Replan")
        self.session.wait_for_window("Replan from Now")


def assign_forty_minutes(session):
    block = next(r for r in json.loads(session.lum("block", "list", "--json"))["rows"] if r["title"] == "Deep work")
    task = json.loads(session.lum("task", "list", "--json"))["rows"][0]["id"]
    session.lum("assign", task, "--block", block["id"], "--minutes", "40")


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
        # What the pick offers, said as it opens, then what starting did.
        self.assertEqual(len(self.session.said()), 2)

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
        # A drop-down's mnemonic opens its list; Escape closes it on the drop-down.
        session.press("Alt+k", "Escape")
        self.assertIn("'Task lists are in'", session.focus())


if __name__ == "__main__":
    unittest.main()
