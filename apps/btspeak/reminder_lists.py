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

import actions
from actions import K, L
import core
from rows import Row, items
from session import Command, Session, live_menu, screen
from tasks import undo_commands


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
        # The core's heading: the list's title, then what it says of itself.
        return session.view().reminders_heading or ""

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
        added = [Command(actions.named(action), adding, key=L.action_letter(K.NEW) or "", through=True)] if action is not None else []
        return [*added, *undo_commands(session)]

    session.reminders_open = True
    try:
        with screen("lumenna-reminders"):
            live_menu(
                session, build, title,
                context=lambda rows: actions.commands(session, rows),
                app=own,
                empty=L.press_to_add(L.action_letter(K.NEW)),
                app_title=L.menu_of(session.view().reminders_title or core.words().reminders_due),
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


#: A reminder's buttons, by the ids the core gives them (`DueButton::id`), for their letters.
BUTTONS = ("carry", "replan", "dismiss", "snooze")


def due(session: Session) -> str:
    """What came due over the last day and was not dismissed: the delete keys dismiss one,
    z snoozes it, on every device. How far back to look is the app's, by its own clock
    (`DueEvent::LookBack`); what each button does is the app's too (`DueEvent::Press`)."""
    # What a look finds is said as it arrives; here the screen says it, as its heading as it
    # opens, and only then: the rows say them after that.
    found = session.send(L.ShellEventDue(L.DueEventLookBack()))
    named = core.words().reminders_due
    state = {"heading": actions.prompt(named, found)}

    def title() -> str:
        shown, state["heading"] = state["heading"], named
        return shown

    def build():
        given = session.view().due
        made = []
        for reminder in given:
            row = Row(reminder.id, reminder.text, (), reminder, title=reminder.title)
            row.side = L.Side.DUE
            made.append(row)
        return items(made)

    def press(row, which: str) -> str:
        """A reminder's button, handed back by its id: the app does what it is (Replan opens its
        proposal, answered here), or says why the reminder has no such button."""
        import proposals  # here: proposals imports actions, which imports this module

        proposals.pin(session)
        said = session.send(L.ShellEventDue(L.DueEventPress(row.key, which)))
        return actions.joined(said, actions.follow(session))

    def button(row, which):
        return next((b for b in row.record.buttons if b.id == which), None)

    def commands(rows) -> list[Command]:
        """Each row's buttons, the core's, in its order: what a reminder offers of its own
        (Carry forward, Replan) first, then Dismiss, which the delete keys run, and Snooze. A
        letter of a button some reminder here offers is pressed as the app's letter on every row
        (`Event::Letter`), the app saying why where a reminder lacks it."""
        order: list[str] = []
        for row in rows:
            for offered in row.record.buttons:
                if offered.id not in order:
                    order.append(offered.id)
        # A letter of a button no reminder here offers still goes to the app, which says why.
        unoffered = [
            Command("", lambda _: "", key=L.due_letter(which) or "", through=True, listed=False)
            for which in BUTTONS if which not in order and L.due_letter(which)
        ]
        return unoffered + [
            Command(
                lambda row, which=which: button(row, which).title,
                lambda row, which=which: press(row, which),
                key=L.due_letter(which) or "",
                applies=lambda row, which=which: button(row, which) is not None,
                deletes=which == "dismiss",
                through=True,
            )
            for which in order
        ]

    with screen("lumenna-reminders"):
        live_menu(
            session, build, title,
            context=commands,
            app=undo_commands(session),
            empty=core.words().nothing_due,
            app_title=L.menu_of(core.words().reminders_due),
        )
    return ""
