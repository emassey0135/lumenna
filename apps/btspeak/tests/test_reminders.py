"""Reminders: the resident service, the app's screens, and installing the service — against a
real `lum rpc`, at fixed moments in 2030 in UTC, so nothing depends on when the tests run."""

from __future__ import annotations

import datetime
import os
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import btspeak_stub  # noqa: E402

btspeak_stub.install()

from connect import connect  # noqa: E402
from session import Session  # noqa: E402
from test_client import LUM  # noqa: E402
import day  # noqa: E402
import preferences  # noqa: E402
import reminder_lists  # noqa: E402
import reminders  # noqa: E402
import service  # noqa: E402

play = btspeak_stub.play

STARTS = "Deep work starts in 5 minutes — nothing assigned yet"


@unittest.skipIf(LUM is None, "`lum` has not been built")
class Reminders(unittest.TestCase):
    def setUp(self):
        self.profile = Path(tempfile.mkdtemp())
        self.saved = {key: os.environ.get(key) for key in ("PATH", "TZ", "LUMENNA_PROFILE", "LUMENNA_BACKUP_DIR", "XDG_CONFIG_HOME")}
        os.environ["PATH"] = f"{Path(LUM).parent}{os.pathsep}{os.environ['PATH']}"
        os.environ["TZ"] = "UTC"
        os.environ["LUMENNA_PROFILE"] = str(self.profile)
        os.environ["LUMENNA_BACKUP_DIR"] = str(self.profile / "backups")
        os.environ["XDG_CONFIG_HOME"] = str(self.profile / "config")
        self.ran = []
        service.RUN = self.ran.append
        reminder_lists.ROOT = self.profile / "root"
        reminder_lists.NOW = None
        btspeak_stub.given.clear()
        self.client = connect(self.profile)
        self.session = Session(self.client)
        self.client.call("block.add", title="Deep work", at="9:00", minutes=60, date="2030-01-15")
        self.givers = []

    def tearDown(self):
        for giver in self.givers:
            giver.let_go()
        self.client.close()
        service.RUN = service._run
        reminder_lists.ROOT = Path("/")
        reminder_lists.NOW = None
        for key, value in self.saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        shutil.rmtree(self.profile, ignore_errors=True)

    def giver(self, at: dict, on=lambda: True):
        # The sound file the device plays, recorded rather than played.
        reminders.play = lambda path: btspeak_stub.given.append(("sound", Path(path).stem))
        giver = reminders.Giver(open_client=lambda: connect(self.profile), on=on, now=lambda: at["now"])
        self.givers.append(giver)
        return giver

    def run_script(self, steps, start):
        script = play(steps)
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

    def test_while_the_service_gives_them_no_other_client_does_and_turned_off_it_lets_go(self):
        at = {"now": "2030-01-15T08:50:00Z"}
        on = {"on": True}
        giver = self.giver(at, on=lambda: on["on"])
        giver.once()
        held = self.client.call("reminder.deliver", now="2030-01-15T08:51:00Z")
        self.assertEqual(held["announcement"], "Another app on this device gives reminders")
        other = self.giver(at)
        self.assertEqual(other.once(), [], "says nothing while the first gives them")
        self.assertEqual(other.pause(), reminders.WAITING)
        self.assertEqual(giver.pause(), reminders.INTERVAL)
        on["on"] = False
        self.assertEqual(giver.once(), [])
        self.assertIsNone(giver.client)
        free = self.client.call("reminder.deliver", now="2030-01-15T08:52:00Z")
        self.assertNotEqual(free["announcement"], "Another app on this device gives reminders")
        self.assertNotIn("elsewhere", free)
        self.assertTrue(held["elsewhere"])

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
        self.assertEqual(script.titles, ["Reminders, 1 reminder", "Reminders"])
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

    # -- settings and the service ---------------------------------------------------------

    def test_the_service_is_installed_only_when_asked_and_only_where_it_is_told(self):
        root = reminder_lists.ROOT
        self.run_script(
            [
                ("menu", "Reminder service, not installed"),
                ("confirm", False),
                ("menu", "Reminder service, not installed"),
                ("confirm", True),
                ("menu", "Reminder service, installed"),
                ("confirm", True),
                ("back",),
            ],
            lambda: reminder_lists.settings_page(self.session),
        )
        unit = root / "BTSpeak" / "Systemd" / "lumenna-reminders.service"
        self.assertEqual(
            self.ran,
            [
                ["sudo", "systemctl", "link", str(unit)],
                ["sudo", "systemctl", "daemon-reload"],
                ["sudo", "systemctl", "enable", "--now", "lumenna-reminders.service"],
                ["sudo", "systemctl", "disable", "--now", "lumenna-reminders.service"],
                ["sudo", "systemctl", "daemon-reload"],
            ],
        )
        self.assertFalse(unit.exists(), "removed again")

    def test_the_installed_files_are_the_checkouts_filled_in_for_this_person(self):
        root = reminder_lists.ROOT
        service.install(self.profile, root, user="pi")
        where = service.paths(root)
        unit = where["unit"].read_text()
        self.assertIn("User=pi\n", unit)
        self.assertIn(f'Environment="LUMENNA_PROFILE={self.profile}"\n', unit)
        self.assertIn(f"{where['code']}\n", unit)
        self.assertNotIn("@", unit.replace(str(self.profile), ""))
        self.assertEqual(where["values"].read_text(), (service.SYSTEM / "lumenna-reminders.values").read_text())
        self.assertIn("reminders.py", where["code"].read_text())
        self.assertTrue(os.access(where["code"], os.X_OK))

    def test_reminders_on_this_device_is_on_the_reminders_page_not_with_the_backups(self):
        backups = [item.get_title() for item in preferences.setting_items(self.session, preferences.on_this_device)]
        self.assertFalse(any(title.startswith("Reminders on this device") for title in backups), backups)
        self.run_script(
            [("menu", "Reminders on this device, On"), ("choose", "Off"), ("back",)],
            lambda: reminder_lists.settings_page(self.session),
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
            lambda: preferences.setting_page(self.session, "Planning", preferences.planning),
        )
        self.assertTrue(script.prompts[1].startswith("'at the end of time' is not a reminder"), script.prompts)
        value = self.client.call("config.get", key="default-block-reminders")["settings"][0]["value"]
        self.assertEqual(value, "at end")


if __name__ == "__main__":
    unittest.main()
