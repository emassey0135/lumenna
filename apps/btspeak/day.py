"""The day and its blocks.

The timeline has no visual metaphor here, and blocks as a time-ordered list lose nothing.
What a timeline shows by empty space is a row of the app's: free time between blocks, and now.
Sittings sit under their block as a second level, folded as the app folds them. Every line is
the core's, in this device's clock; the day shown, moving between days and today opening on
now are the app's.
"""

from __future__ import annotations

import actions
from actions import L
import core
from rows import Row, items
from session import Command, Session, empty_then, heading, live_menu, screen
import proposals  # noqa: F401 - Replan from now's and Plan the day's screen
import tasks


#: A block form's yes-or-no fields, as a choice.


# ---------------------------------------------------------------------------------------
# The planner
# ---------------------------------------------------------------------------------------


def day_rows(session: Session) -> list[Row]:
    """The day's rows, not folded away, each with its line: blocks with their sittings, free
    time, now, what was cancelled for the day. The day's heading is the screen's title."""
    view, lines = session.view(), session.lines()
    if view.day is None:
        return []
    rows = []
    for index, item in enumerate(view.day.items):
        if item.hidden or item.kind == L.DayKind.SUMMARY:
            continue
        line = lines.day[index] if index < len(lines.day) else None
        rows.append(Row(
            item.key, line.text if line else "", item.offered, item, level=item.level, expanded=item.expanded,
            braille=line.braille if line else "",
        ))
    return rows


def day_heading(session: Session) -> str:
    """The day as every app heads it: "<day>. <summary>", the core's line."""
    view, lines = session.view(), session.lines()
    if view.day is None:
        return L.place_title(L.PlaceToday())
    return next(
        (lines.day[i].text for i, item in enumerate(view.day.items)
         if item.kind == L.DayKind.SUMMARY and i < len(lines.day)),
        lines.day_title,
    )


def day_heading_braille(session: Session) -> str:
    """The day's heading as the display shows it: "<day>. 4 blk, 4:00 wrk, 3 tsk"."""
    view, lines = session.view(), session.lines()
    if view.day is None:
        return ""
    return next(
        (lines.day[i].braille for i, item in enumerate(view.day.items)
         if item.kind == L.DayKind.SUMMARY and i < len(lines.day)),
        "",
    )


def day_plan(session: Session) -> str:
    """A day as it is lived, opening on now.

    Enter does a row's main thing, the shared table's Open — edits a block or a sitting's task,
    adds a block in free time, puts a cancelled day back. The rest is on the row's context menu (M-Chord with
    Dot 7); the day's own actions, then what Today offers of its own (the core's: a new block,
    moving between days, undo and redo) are on the main menu (M-Chord), and on their keys.
    """

    def commands():
        own = day_commands(session)
        # The day's What should I work on? is the screen's too: offered once, as the day's.
        return [*own, *tasks.place_commands(session, skip={command.key for command in own})]

    def build():
        session.show(L.PlaceToday())
        return items(day_rows(session), fold=lambda row, open_: actions.fold(session, row, open_))

    with screen("lumenna-day"):
        live_menu(
            session, build, lambda: day_heading(session),
            braille_title=lambda: day_heading_braille(session),
            # Enter is the shared table's Open: edits a block or a sitting's task, adds a block in
            # free time, puts a cancelled day back.
            main=lambda row: actions.press(session, "open", row),
            context=lambda rows: actions.commands(session, rows),
            app=commands,
            bar=(L.PlaceToday(), False),
            empty=core.words().nothing_planned,
            app_title=L.menu_of(L.place_title(L.PlaceToday())),
            focus=lambda: session.view().focus,
        )
    return ""


def day_commands(session: Session) -> list[Command]:
    """What can be done to the day shown itself, the day's own actions in the core's order,
    first on the Day menu: Plan the day, Replan from now, What should I work on?, End the day,
    Clear the rest of the day."""
    day = session.view().day
    return [
        Command(actions.named(action), lambda _, action=action: actions.act(session, action, on=L.ActingOnDay()),
                key=L.action_letter(action.kind) or "", through=True)
        for action in (day.offered if day is not None else [])
    ]


# ---------------------------------------------------------------------------------------
# Blocks
# ---------------------------------------------------------------------------------------


def blocks(session: Session) -> str:
    """Every block series: Enter edits one, the delete keys delete it; the main menu is what the
    Blocks list offers of its own, the core's (New block, undo and redo)."""
    state = {"heading": L.place_title(L.PlaceBlocks())}

    def build():
        session.show(L.PlaceBlocks())
        rows = tasks.list_rows(session)
        state["heading"] = heading(L.place_title(L.PlaceBlocks()), tasks.readback(session), bool(rows))
        return items(rows)

    with screen("lumenna-day"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: actions.press(session, "open", row),
            context=lambda rows: actions.commands(session, rows),
            app=lambda: tasks.place_commands(session),
            bar=(L.PlaceBlocks(), False),
            empty=empty_then(lambda: tasks.readback(session), L.press_to_add(L.command_letter("new-block"))),
            app_title=L.menu_of(L.place_title(L.PlaceBlocks())),
            focus=lambda: session.view().focus,
        )
    return ""


# ---------------------------------------------------------------------------------------
# The block form: the app's (`ViewModel::block_form`), described as a dialog
# (`ScreenView::dialog`) and filled in with the device's form.
# ---------------------------------------------------------------------------------------


def block_form(session: Session) -> str:
    """The block form the app has open — a new block, a block changed for one day or every
    day, a block in free time — asked with the device's form (`forms.form`), again while saving
    is refused, the app's sentence first. What was said."""
    import forms

    return forms.form_until_closed(session, "block")
