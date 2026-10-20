"""Reminders in the app: a task's or block's own, what came due, and the device's settings.

A task with a due date and every block offer Reminders (`r`): the app's list of them, with Add
reminder (`a`), a phrase the core reads, asked again with the core's sentence when it cannot be
read, and Remove reminder on each of its own (the delete keys). The main menu's Reminders lists
what came due over the last day and was not dealt with, whatever gave it — this app's service,
or another device's dismissal not yet here — to dismiss (the delete keys) or snooze (`z`), at a
block's end that left work short to carry it forward (`f`), and on an overrun to replan (`p`).
Settings has Reminders on this device, the core's own setting, and the reminder service,
installed only when asked.
"""

from __future__ import annotations

import datetime
import subprocess
from pathlib import Path

from BTSpeak import dialogs

import actions
from actions import K, L
from core import profile_directory
from rows import Row, items
import service
from session import Command, Session, confirm, live_menu, screen
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


def now() -> datetime.datetime:
    """Now, or the moment a test has fixed."""
    return NOW() if NOW else datetime.datetime.now(datetime.timezone.utc)


# ---------------------------------------------------------------------------------------
# A task's or block's own
# ---------------------------------------------------------------------------------------


def reminder_rows(session: Session) -> list[Row]:
    """The open list's reminders, each said as the core lines it: when it fires, and whether
    it is a default."""
    listing, lines = session.view().reminders, session.lines().reminders
    if listing is None:
        return []
    rows = []
    for index, reminder in enumerate(listing.reminders):
        line = lines[index].text if index < len(lines) else reminder.words
        row = Row(reminder.id, line, reminder.actions, reminder, title=reminder.words)
        row.side = L.Side.REMINDERS
        rows.append(row)
    return rows


def reminders_of(session: Session, row=None) -> str:
    """The list a Reminders action opened, until the person leaves it: the core's
    announcement as its heading, each reminder a row. Leaving closes it in the app."""
    name = getattr(row, "title", "") if row is not None else ""
    state = {"heading": "Reminders"}

    def build():
        listing = session.view().reminders
        if listing is None:
            return []
        said = listing.announcement
        state["heading"] = f"Reminders for {name}. {said}" if name else said
        return items(reminder_rows(session))

    def adding(_row) -> str:
        listing = session.view().reminders
        found = [action for action in (listing.actions if listing else []) if action.kind == K.NEW]
        return actions.act(session, found[0]) if found else ""

    session.reminders_open = True
    try:
        with screen("lumenna-reminders"):
            live_menu(
                session, build, lambda: state["heading"],
                context=lambda rows: actions.commands(session, rows),
                app=[Command("Add reminder", adding, key="a"), *undo_commands(session)],
                empty="Press a to add one.",
                app_title="Reminders menu",
                focus=lambda: session.view().reminders_held.focus,
                until=lambda: session.view().reminders is None,
            )
    finally:
        session.reminders_open = False
    if session.view().reminders is not None:
        session.send(L.ShellEventCloseReminders())
    return ""


# ---------------------------------------------------------------------------------------
# What came due
# ---------------------------------------------------------------------------------------


def due(session: Session) -> str:
    """What came due over the last day and was not dismissed: the delete keys dismiss one,
    z snoozes it, on every device."""
    state = {"heading": "Reminders"}
    looked = now()
    # What a look finds is said as it arrives; here the screen says it, as its heading.
    session.send(L.ShellEventDue(L.DueEventLook(_moment(looked - LOOK_BACK), _moment(looked))))

    def build():
        given = session.view().due
        made = []
        for reminder in given:
            row = Row(reminder.id, reminder.text, (), reminder, title=reminder.title)
            made.append(row)
        return items(made)

    def deal(event):
        def run(row) -> str:
            return session.send(L.ShellEventDue(event(row.key)))
        return run

    def replan(row) -> str:
        import proposals  # here: proposals imports actions, which imports this module

        proposals.pin(session)
        session.send(L.ShellEventDue(L.DueEventReplan(row.key)))
        return actions.follow(session)

    def no(event):
        # The app refuses a button the reminder lacks, in the core's words.
        return lambda row: session.send(L.ShellEventDue(event(row.key)))

    with screen("lumenna-reminders"):
        live_menu(
            session, build, lambda: state["heading"],
            context=lambda rows: [
                Command(lambda row: row.record.dismiss, deal(L.DueEventDismiss), deletes=True),
                Command(lambda row: row.record.snooze or "", deal(L.DueEventSnooze), key="z",
                        applies=lambda row: bool(row.record.snooze)),
                # A block's end that left work short: its text ends in the question this answers.
                Command(lambda row: session.sentence(row.record.carry or ""), deal(L.DueEventCarry), key="f",
                        applies=lambda row: bool(row.record.carry), refuse=no(L.DueEventCarry)),
                # A block's work outrunning its time: Replan from now.
                Command(lambda row: session.sentence(row.record.replan or ""), replan, key="p",
                        applies=lambda row: bool(row.record.replan), refuse=no(L.DueEventReplan)),
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
        return service.install(profile_directory(), ROOT)
    except (OSError, subprocess.CalledProcessError) as error:
        detail = getattr(error, "stderr", "") or str(error)
        return f"The reminder service could not be set up: {detail.strip()}"


def settings_page(session: Session) -> str:
    """Reminders on this device, which the core honours, and the service."""
    import preferences  # here, since preferences imports this module

    def build():
        made = preferences.setting_items(session, lambda setting: setting.key in KEYS)
        made.append(dialogs.DynamicMenuItem(title=lambda *_: service_title(), action=lambda: toggle_service(session)))
        return made

    with screen("lumenna-reminders"), preferences.opened(session):
        live_menu(session, build, "Reminders", app=undo_commands(session), app_title="Reminders menu")
    return ""
