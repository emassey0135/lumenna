"""Reminders in the app: a task's or block's own, what came due, and the device's settings.

A task with a due date and every block offer Reminders (`r`): the app's list of them, with Add
reminder (`a`), a phrase the core reads, asked again with the core's sentence when it cannot be
read, and Remove reminder on each of its own (the delete keys). The main menu's Reminders lists
what came due over the last day and was not dealt with, whatever gave it — this app's service,
or another device's dismissal not yet here — to dismiss (the delete keys) or snooze (`z`), at a
block's end that left work short to carry it forward (`f`), and on an overrun to replan (`p`).
Settings' Reminders page, the core's, has Reminders on this device and the defaults; the background
service that gives them while the app is closed is on its General page (`preferences.py`).
"""

from __future__ import annotations

import datetime

import actions
from actions import K, L
from rows import Row, items
from session import Command, Session, live_menu, screen, sentences
from tasks import undo_commands


#: For tests: a function of nothing giving now, as an aware datetime. None is now.
NOW = None

#: How far back the Reminders screen looks for what came due: as far as the service ever
#: goes back after being stopped.
LOOK_BACK = datetime.timedelta(hours=24)


def _moment(when: datetime.datetime) -> str:
    return when.astimezone(datetime.timezone.utc).isoformat().replace("+00:00", "Z")


#: This app's letter for each of a reminder's buttons (`DueButton.id`); Dismiss is the delete
#: keys'.
DUE_KEYS = {"carry": "f", "replan": "p", "snooze": "z"}


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
        row = Row(reminder.id, line, reminder.offered, reminder, title=reminder.words)
        row.side = L.Side.REMINDERS
        rows.append(row)
    return rows


def reminders_of(session: Session, row=None) -> str:
    """The list a Reminders action opened, until the person leaves it: the app's title for it
    ("Reminders for Pay rent") and the core's announcement as its heading, each reminder a row,
    and the list's own Add reminder. Leaving closes it in the app."""

    def title() -> str:
        view = session.view()
        listing = view.reminders
        return actions.prompt(view.reminders_title or "", listing.announcement if listing is not None else "")

    def build():
        return items(reminder_rows(session)) if session.view().reminders is not None else []

    def adding_action():
        listing = session.view().reminders
        return next((action for action in (listing.offered if listing else []) if action.kind == K.NEW), None)

    def adding(_row) -> str:
        action = adding_action()
        return actions.act(session, action, on=L.ActingOnReminders()) if action is not None else ""

    def own():
        action = adding_action()
        added = [Command(actions.named(action), adding, key="a")] if action is not None else []
        return [*added, *undo_commands(session)]

    session.reminders_open = True
    try:
        with screen("lumenna-reminders"):
            live_menu(
                session, build, title,
                context=lambda rows: actions.commands(session, rows),
                app=own,
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
    looked = now()
    # What a look finds is said as it arrives; here the screen says it, as its heading as it
    # opens, and only then: the rows say them after that.
    found = session.send(L.ShellEventDue(L.DueEventLook(_moment(looked - LOOK_BACK), _moment(looked))))
    state = {"heading": sentences("Reminders", found)}

    def title() -> str:
        shown, state["heading"] = state["heading"], "Reminders"
        return shown

    def build():
        given = session.view().due
        made = []
        for reminder in given:
            row = Row(reminder.id, reminder.text, (), reminder, title=reminder.title)
            made.append(row)
        return items(made)

    def replan(key: str) -> str:
        import proposals  # here: proposals imports actions, which imports this module

        proposals.pin(session)
        # A reminder without the button: the app says why, and opens nothing.
        said = session.send(L.ShellEventDue(L.DueEventReplan(key)))
        return actions.joined(said, actions.follow(session))

    #: What each button sends; Replan opens the app's proposal, answered here.
    runs = {
        "carry": lambda key: session.send(L.ShellEventDue(L.DueEventCarry(key))),
        "replan": replan,
        "dismiss": lambda key: session.send(L.ShellEventDue(L.DueEventDismiss(key))),
        "snooze": lambda key: session.send(L.ShellEventDue(L.DueEventSnooze(key))),
    }

    def button(row, which):
        return next((b for b in row.record.buttons if b.id == which), None)

    def commands(rows) -> list[Command]:
        """Each row's buttons, the core's, in its order: what a reminder offers of its own
        (Carry forward, Replan) first, then Dismiss, which the delete keys run, and Snooze. A
        letter a reminder lacks is sent all the same, and the app says why."""
        order: list[str] = []
        for row in rows:
            for offered in row.record.buttons:
                if offered.id not in order and offered.id in runs:
                    order.append(offered.id)
        # A letter no reminder here offers still runs, for the app to say why.
        order.extend(which for which in runs if which not in order)
        return [
            Command(
                lambda row, which=which: button(row, which).title,
                lambda row, which=which: runs[which](row.key),
                key=DUE_KEYS.get(which, ""),
                applies=lambda row, which=which: button(row, which) is not None,
                deletes=which == "dismiss",
                refuse=(lambda row, which=which: runs[which](row.key)) if which in ("carry", "replan") else None,
            )
            for which in order
        ]

    with screen("lumenna-reminders"):
        live_menu(
            session, build, title,
            context=commands,
            app=undo_commands(session),
            empty="Nothing has come due.",
            app_title="Reminders menu",
        )
    return ""
