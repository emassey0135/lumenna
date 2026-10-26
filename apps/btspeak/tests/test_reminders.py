"""Reminders: the resident service, the app's screens, and installing the service — against a
real store, at fixed moments in 2030 in UTC, so nothing depends on when the tests run."""

from __future__ import annotations

import datetime
import json
import shutil
import socket
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import btspeak_stub  # noqa: E402

btspeak_stub.install()

from harness import LUM, Store  # noqa: E402
from actions import L  # noqa: E402
import background  # noqa: E402
import day  # noqa: E402
import preferences  # noqa: E402
import reminder_lists  # noqa: E402
import reminders  # noqa: E402
import service  # noqa: E402

play = btspeak_stub.play


def answering(profile: Path, within: float = 0) -> str | None:
    """What answers at the profile's address, by what `initialize` says it is, as `lum` asks;
    waiting up to `within` seconds for something to: the address is served from a thread of its
    own, started once the endpoint is held, so it can be a moment behind."""
    deadline = time.monotonic() + within
    while True:
        process = _answering(profile)
        if process is not None or time.monotonic() >= deadline:
            return process
        time.sleep(0.02)


def _answering(profile: Path) -> str | None:
    path = profile / "lumenna.sock"
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(5)
            connection.connect(str(path))
            connection.sendall(b'{"jsonrpc":"2.0","id":1,"method":"initialize"}\n')
            line = connection.makefile("r", encoding="utf-8").readline()
    except OSError:
        return None
    return json.loads(line).get("result", {}).get("process")

STARTS = "Deep work starts in 5 minutes — nothing assigned yet"


@unittest.skipIf(LUM is None, "`lum` has not been built")
class Reminders(unittest.TestCase):
    def setUp(self):
        self.store = Store()
        self.profile = self.store.profile
        self.client = self.store.client
        self.session = self.store.session
        self.ran = []
        service.RUN = self.ran.append
        reminder_lists.NOW = None
        btspeak_stub.given.clear()
        self.client.call("block.add", title="Deep work", at="9:00", minutes=60, date="2030-01-15")
        self.givers = []

    def tearDown(self):
        for giver in self.givers:
            giver.let_go()
        service.RUN = service._run
        reminder_lists.NOW = None
        self.store.close()

    def giver(self, at: dict):
        # The sound file the device plays, recorded rather than played.
        reminders.play = lambda path: btspeak_stub.given.append(("sound", Path(path).stem))
        giver = reminders.Giver(open_store=lambda: L.Device(str(self.profile)), now=lambda: at["now"])
        self.givers.append(giver)
        return giver

    def run_script(self, steps, start):
        script = play(steps)
        self.store.caught_up()
        start()
        self.assertTrue(script.finished(), f"steps left over: {script.steps}")
        return script

    # -- the service ----------------------------------------------------------------------

    def test_the_service_gives_each_reminder_once_with_its_sound_then_speech_and_braille(self):
        at = {"now": "2030-01-15T08:50:00Z"}
        giver = self.giver(at)
        self.assertEqual(giver.once(), [])
        at["now"] = "2030-01-15T08:55:30Z"
        self.assertEqual(giver.once(), [STARTS])
        self.assertEqual(btspeak_stub.given, [("sound", "notification"), ("say", STARTS), ("braille", STARTS)])
        at["now"] = "2030-01-15T08:56:00Z"
        self.assertEqual(giver.once(), [])

    def test_a_restarted_service_says_what_came_while_it_was_stopped_as_one_summary(self):
        at = {"now": "2030-01-15T08:50:00Z"}
        self.giver(at).once()
        self.givers[0].let_go()
        at["now"] = "2030-01-15T09:30:00Z"
        self.assertEqual(self.giver(at).once(), ["1 reminder missed: Deep work started."])

    def test_while_the_service_gives_them_no_other_client_does_and_stopped_it_lets_go(self):
        at = {"now": "2030-01-15T08:50:00Z"}
        giver = self.giver(at)
        giver.once()
        held = self.client.call("reminder.deliver", now="2030-01-15T08:51:00Z")
        self.assertEqual(held["announcement"], "Another app on this device gives reminders")
        other = self.giver(at)
        self.assertEqual(other.once(), [], "says nothing while the first gives them")
        self.assertEqual(other.pause(), reminders.WAITING)
        self.assertEqual(giver.pause(), reminders.INTERVAL)
        giver.let_go()
        free = self.client.call("reminder.deliver", now="2030-01-15T08:52:00Z")
        self.assertNotEqual(free["announcement"], "Another app on this device gives reminders")
        self.assertNotIn("elsewhere", free)
        self.assertTrue(held["elsewhere"])

    def test_the_app_gives_reminders_while_it_runs_and_lets_go_when_it_closes(self):
        at = {"now": "2030-01-15T08:50:00Z"}
        giver = self.giver(at)
        giver.once()
        at["now"] = "2030-01-15T08:55:30Z"
        stop = reminders.give_in_background(giver)
        deadline = time.monotonic() + 10
        while ("say", STARTS) not in btspeak_stub.given and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertIn(("say", STARTS), btspeak_stub.given)
        stop()
        free = self.client.call("reminder.deliver", now="2030-01-15T08:56:00Z")
        self.assertNotIn("elsewhere", free)

    # -- a task's or block's own ----------------------------------------------------------

    def test_a_blocks_reminders_take_a_phrase_asked_again_when_refused_and_one_is_removed(self):
        script = self.run_script(
            [
                ("context", "Deep work", "Reminders"),
                ("app", "Add reminder"),
                ("input", "when it feels right"),
                ("input", "10m after end"),
                ("menu", "10 minutes after it ends", "delete"),
                ("back",),
                ("back",),
            ],
            lambda: day.blocks(self.session),
        )
        self.assertTrue(script.prompts[1].startswith("'when it feels right' is not a reminder"), script.prompts)
        self.assertIn("Reminders for Deep work. 1 reminder, the defaults", script.titles)
        self.assertIn("Reminders for Deep work. 1 reminder", script.titles)
        series = self.client.call("block.list")["rows"][0]["id"]
        self.assertEqual(self.client.call("reminder.list", target=series)["announcement"], "1 reminder, the defaults")

    # -- what came due --------------------------------------------------------------------

    def test_what_came_due_is_listed_and_dismissed_on_every_device(self):
        reminder_lists.NOW = lambda: datetime.datetime(2030, 1, 15, 9, 0, tzinfo=datetime.timezone.utc)
        script = self.run_script(
            [("menu", "Deep work starts", "delete"), ("back",)],
            lambda: reminder_lists.due(self.session),
        )
        self.assertIn("Dismissed", script.said)
        # What the look found is said once, as the screen opens; the rows say it after that.
        self.assertEqual(script.titles, ["Reminders. Deep work starts in 5 minutes — nothing assigned yet.", "Reminders"])
        self.assertEqual(script.menus[-1]["empty"], "Nothing has come due.")

    def test_what_came_due_is_snoozed_by_its_letter(self):
        reminder_lists.NOW = lambda: datetime.datetime(2030, 1, 15, 9, 0, tzinfo=datetime.timezone.utc)
        script = self.run_script(
            [("key", "Deep work starts", "z"), ("back",)],
            lambda: reminder_lists.due(self.session),
        )
        self.assertIn("Snoozed for 10 minutes", script.said)

    def test_a_blocks_end_that_left_work_short_is_carried_forward_from_what_came_due(self):
        self.client.call("config.set", key="default-block-reminders", value="")
        self.client.call("block.add", title="Admin", at="9:00", minutes=60, date="2030-01-16")
        self.client.call("task.add", text="Essay")
        block = self.client.call("plan", date="2030-01-15")["blocks"][0]["id"]
        task = self.client.call("task.list")["rows"][0]["id"]
        self.client.call("assign", task=task, block=block, date="2030-01-15", minutes=90)
        sitting = self.client.call("plan", date="2030-01-15")["blocks"][0]["assignments"][0]["id"]
        self.client.call("stop", assignment=sitting, minutes=40)
        reminder_lists.NOW = lambda: datetime.datetime(2030, 1, 15, 10, 1, tzinfo=datetime.timezone.utc)
        ending = "Deep work is ending. Essay: 40 minutes of 1 hour 30 minutes. Carry the remaining 50 minutes to Admin tomorrow?"
        script = self.run_script(
            [("context", ending, "Carry forward"), ("back",)],
            lambda: reminder_lists.due(self.session),
        )
        self.assertIn("Carried forward Essay to Admin", script.said)
        admin = self.client.call("plan", date="2030-01-16")["blocks"][0]
        self.assertEqual(admin["assignments"][0]["planned_mins"], 50)

    def test_carrying_forward_where_a_reminder_does_not_offer_it_says_why(self):
        reminder_lists.NOW = lambda: datetime.datetime(2030, 1, 15, 9, 0, tzinfo=datetime.timezone.utc)
        script = self.run_script(
            [("key", "Deep work starts", "f"), ("back",)],
            lambda: reminder_lists.due(self.session),
        )
        self.assertIn("That does not apply here.", script.said)

    def test_a_blocks_work_outrunning_it_opens_replan_from_what_came_due(self):
        import proposals
        self.client.call("config.set", key="default-block-reminders", value="")
        self.client.call("block.add", title="Meeting", at="10:00", minutes=60, date="2030-01-15")
        block = self.client.call("plan", date="2030-01-15")["blocks"][0]["id"]
        for title, minutes in [("Essay", 40), ("Email", 30)]:
            task = self.client.call("task.add", text=title)["task"]["id"]
            self.client.call("assign", task=task, block=block, date="2030-01-15", minutes=minutes)
        reminder_lists.NOW = lambda: datetime.datetime(2030, 1, 15, 9, 1, tzinfo=datetime.timezone.utc)
        proposals.NOW = lambda: "2030-01-15T09:01"
        self.addCleanup(setattr, proposals, "NOW", None)
        overrun = "Deep work has 1 hour left and 1 hour 10 minutes of work planned, more than it holds"
        script = self.run_script(
            [("context", overrun, "Replan"), ("menu", "Cancel"), ("key", overrun, "p"), ("menu", "Cancel"), ("back",)],
            lambda: reminder_lists.due(self.session),
        )
        self.assertEqual(script.titles[0], f"Reminders. {overrun}.")
        replans = [title for title in script.titles if title.startswith("Replan from now. ")]
        self.assertEqual(len(replans), 2, script.titles)
        self.assertNotIn("Nothing needs to change", replans[0])

    def test_replan_where_a_reminder_does_not_offer_it_says_why(self):
        reminder_lists.NOW = lambda: datetime.datetime(2030, 1, 15, 9, 0, tzinfo=datetime.timezone.utc)
        script = self.run_script(
            [("key", "Deep work starts", "p"), ("back",)],
            lambda: reminder_lists.due(self.session),
        )
        self.assertIn("That does not apply here.", script.said)

    # -- the background service, and turning it on -----------------------------------------

    def test_background_reminders_and_sync_are_the_apps_setting_turned_on_by_linking_the_unit_in(self):
        config = service.config()
        script = self.run_script(
            [
                ("menu", "General"),
                ("menu", "Background reminders and sync, Off"),
                ("choose", "Off"),
                ("menu", "Background reminders and sync, Off"),
                ("choose", "On"),
                ("menu", "Background reminders and sync, On"),
                ("choose", "On"),
                ("back",),
                ("back",),
            ],
            lambda: preferences.settings(self.session),
        )
        self.assertIn("Background reminders and sync are on. They start whenever the device does", script.said)
        self.assertTrue(service.enabled(config))
        where = service.paths(config)
        self.assertEqual(where["unit"].resolve(), service.UNIT)
        self.assertEqual(where["wanted"].resolve(), service.UNIT)
        self.assertEqual(where["profile"].read_text(), f'[Service]\nEnvironment="LUMENNA_PROFILE={self.profile}"\n')
        self.assertEqual(
            self.ran,
            [["systemctl", "--user", "daemon-reload"], ["systemctl", "--user", "start", "lumenna-background.service"]],
        )

    def test_turned_off_the_service_is_stopped_and_its_links_and_profile_are_gone(self):
        config = service.config()
        service.enable(self.profile, config)
        # Turned on elsewhere: the session tells the app as it would at start.
        self.session.send(self.session.background())
        self.ran.clear()
        script = self.run_script(
            [("menu", "General"), ("menu", "Background reminders and sync, On"), ("choose", "Off"), ("back",), ("back",)],
            lambda: preferences.settings(self.session),
        )
        self.assertIn("Background reminders and sync are off", script.said)
        self.assertEqual(
            self.ran,
            [["systemctl", "--user", "stop", "lumenna-background.service"], ["systemctl", "--user", "daemon-reload"]],
        )
        for path in service.paths(config).values():
            self.assertFalse(path.is_symlink() or path.exists(), path)
        self.assertFalse(service.paths(config)["profile"].parent.exists())

    def test_the_background_service_keeps_the_device_in_sync_and_serves_the_command_surface(self):
        device = L.Device(str(self.profile))
        try:
            self.assertTrue(device.keep_in_sync(background.PROCESS, "test device", "btspeak"), "holds the endpoint")
            self.assertTrue(device.keep_in_sync(background.PROCESS, "test device", "btspeak"), "asked again, unchanged")
            self.assertEqual(answering(self.profile, within=10), background.PROCESS)
        finally:
            device.stop_syncing()
        self.assertIsNone(answering(self.profile), "the address given up once stopped")

    def test_the_unit_runs_the_background_service_beside_it_as_the_person_once_the_device_is_ready(self):
        unit = service.UNIT.read_text()
        self.assertIn("ExecStart=/usr/bin/python3 %Y/../background.py\n", unit)
        self.assertTrue((service.UNIT.parent.parent / "background.py").exists())
        self.assertIn("is-active --quiet btspeak-ready.service", unit)
        self.assertIn("WantedBy=default.target\n", unit)
        self.assertNotIn("User=", unit)
        self.assertNotIn("/BTSpeak", unit)

    def test_reminders_on_this_device_is_on_the_reminders_page_not_with_the_backups(self):
        with preferences.opened(self.session):
            backups = [item.get_title() for item in preferences.setting_items(self.session, "backups")]
        self.assertFalse(any(title.startswith("Reminders on this device") for title in backups), backups)
        self.run_script(
            [("menu", "Reminders on this device, On"), ("choose", "Off"), ("back",)],
            lambda: preferences.setting_page(self.session, "reminders"),
        )
        self.assertEqual(self.client.call("config.get", key="reminders")["settings"][0]["value"], "false")

    def test_a_default_reminder_the_core_refuses_is_asked_again_with_its_sentence(self):
        script = self.run_script(
            [
                ("menu", "Reminders for a block, 5m before start"),
                ("input", "at the end of time"),
                ("input", "at end"),
                ("back",),
            ],
            lambda: preferences.setting_page(self.session, "reminders"),
        )
        self.assertTrue(script.prompts[1].startswith("'at the end of time' is not a reminder"), script.prompts)
        value = self.client.call("config.get", key="default-block-reminders")["settings"][0]["value"]
        self.assertEqual(value, "at end")


if __name__ == "__main__":
    unittest.main()
