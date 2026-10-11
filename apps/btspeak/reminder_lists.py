"""Reminders in the app: a task's or block's own, what came due, and the device's settings.

A task with a due date and every block offer Reminders (`r`): their list, from
`reminder.list`, with Add reminder (`a`), a phrase the core reads, asked again with the core's
sentence when it cannot be read, and Remove reminder on each of its own (the delete keys).
The main menu's Reminders lists what came due over the last day and was not dealt with,
whatever gave it — this app's service, or another device's dismissal not yet here — to
dismiss (the delete keys) or snooze (`z`), and, at a block's end that left work short, to
carry it forward (`f`, the reminder's own Carry forward, `carry_rest`). Settings has Reminders on this device, the core's
own setting, and the reminder service, installed only when asked.
"""

from __future__ import annotations

import datetime
import subprocess
from pathlib import Path

from BTSpeak import dialogs

import actions
import connect
import service
from client import LumennaError
from session import Command, Session, confirm, live_menu, row_item, screen, spoken
from tasks import undo_commands


#: For tests: a function of nothing giving now, as an aware datetime. None is now.
NOW = None

#: Where the reminder service is installed: the device's root, or a test's directory.
ROOT = Path("/")

#: How far back the Reminders screen looks for what came due: as far as the service ever
#: goes back after being stopped.
LOOK_BACK = datetime.timedelta(hours=24)


def _moment(when: datetime.datetime) -> str:
    return when.astimezone(datetime.timezone.utc).isoformat().replace("+00:00", "Z")


# ---------------------------------------------------------------------------------------
# A task's or block's own
# ---------------------------------------------------------------------------------------


def target_name(row) -> str:
    """What the list is of, by name: a block's title, else the row's own."""
    row = row or {}
    return (row.get("block") or {}).get("title") or row.get("title", "")


def add_reminder(session: Session, action: dict) -> str:
    """Asks for a phrase and adds it; a phrase the core cannot read is asked again, the
    core's sentence first and the phrase as typed, as a refused field stays in its dialog."""
    question = action.get("question", {})
    wrong, typed = "", question.get("initial", "")
    while True:
        text = dialogs.request_input(actions.prompt(wrong, actions.text_prompt(question)), default_text=typed)
        if text is None:
            return ""
        try:
            result = session.call("act", action=action, answer={"answer": "text", "text": text})
        except LumennaError as error:
            wrong, typed = error.message, text
            continue
        session.wrote()
        return spoken(result)


def reminders_of(session: Session, action: dict, row=None) -> str:
    """The list a Reminders action opens: the core's announcement, then each reminder."""
    target = action["target"]
    name = target_name(row)
    state = {"heading": "Reminders", "list": {}}

    def build():
        try:
            listing = session.call("reminder.list", target=target)
        except LumennaError as error:
            return error.message
        state["list"] = listing
        said = listing.get("announcement", "")
        state["heading"] = f"Reminders for {name}. {said}" if name else said
        return [
            row_item(reminder, reminder["words"] if reminder.get("own") else f"{reminder['words']}, a default")
            for reminder in listing.get("reminders", [])
        ]

    def adding(_row) -> str:
        found = state["list"].get("actions", [])
        return add_reminder(session, found[0]) if found else ""

    with screen("lumenna-reminders"):
        live_menu(
            session, build, lambda: state["heading"],
            context=lambda rows: actions.commands(session, rows),
            app=[Command("Add reminder", adding, key="a"), *undo_commands(session)],
            empty="Press a to add one.",
            app_title="Reminders menu",
        )
    return ""


actions.FORMS[("task", "reminders")] = reminders_of
actions.FORMS[("series", "reminders")] = reminders_of


# ---------------------------------------------------------------------------------------
# What came due
# ---------------------------------------------------------------------------------------


def due(session: Session) -> str:
    """What came due over the last day and was not dismissed: the delete keys dismiss one,
    z snoozes it, on every device."""
    state = {"heading": "Reminders"}

    def build():
        now = NOW() if NOW else datetime.datetime.now(datetime.timezone.utc)
        try:
            found = session.call("reminder.due", since=_moment(now - LOOK_BACK), now=_moment(now))
        except LumennaError as error:
            return error.message
        given = [*found.get("missed", []), *found.get("due", [])]
        state["heading"] = f"Reminders, {found.get('announcement', '')}" if given else "Reminders"
        return [
            row_item(reminder, reminder["text"], navigation_title=reminder["title"])
            for reminder in given
        ]

    def deal(method: str):
        def run(reminder) -> str:
            return session.write(method, key=reminder["key"], occurrence=reminder["occurrence"])
        return run

    def carry(reminder) -> str:
        # At a block's end with work left short: its text ends in the question this answers.
        return session.write("carry_rest", block=f"{reminder['target']}@{reminder['occurrence']}")

    def no_carry(_reminder) -> str:
        try:
            return session.call("form.not_offered", kind="carry_rest", subject="reminder").get("value", "")
        except LumennaError as error:
            return error.message

    with screen("lumenna-reminders"):
        live_menu(
            session, build, lambda: state["heading"],
            context=lambda rows: [
                Command(lambda row: (row or {}).get("dismiss", "Dismiss"), deal("reminder.dismiss"), deletes=True),
                Command(lambda row: (row or {}).get("snooze", "Snooze"), deal("reminder.snooze"), key="z"),
                Command(lambda row: session.sentence((row or {}).get("carry", "")), carry, key="f",
                        applies=lambda row: bool((row or {}).get("carry")), refuse=no_carry),
            ],
            app=undo_commands(session),
            empty="Nothing has come due.",
            app_title="Reminders menu",
        )
    return ""


# ---------------------------------------------------------------------------------------
# Settings: reminders on this device, and the service that gives them
# ---------------------------------------------------------------------------------------


#: The device settings shown on this page rather than with the backups.
KEYS = {"reminders"}


def service_title() -> str:
    return f"Reminder service, {'installed' if service.installed(ROOT) else 'not installed'}"


def toggle_service(session: Session) -> str:
    """Installs the service, or removes it, each only once the person says yes."""
    try:
        if service.installed(ROOT):
            if not confirm("Remove the reminder service? Reminders are then given only while another app runs"):
                return ""
            return service.uninstall(ROOT)
        if not confirm(
            "Install the reminder service? It runs all the time, apart from this app, and gives "
            "reminders with the notification sound, speech and braille. Installing it needs an administrator"
        ):
            return ""
        return service.install(connect.profile_directory(), ROOT)
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        return f"The reminder service could not be set up: {detail.strip()}"


def settings_page(session: Session) -> str:
    """Reminders on this device, which the core honours, and the service."""
    import preferences  # here, since preferences imports this module

    def items():
        rows = preferences.setting_items(session, lambda setting: setting["key"] in KEYS)
        rows.append(dialogs.DynamicMenuItem(title=lambda *_: service_title(), action=lambda: toggle_service(session)))
        return rows

    with screen("lumenna-reminders"):
        live_menu(session, items, "Reminders", app=undo_commands(session), app_title="Reminders menu")
    return ""
