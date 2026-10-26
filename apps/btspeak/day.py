"""The day and its blocks.

The timeline has no visual metaphor here, and blocks as a time-ordered list lose nothing.
What a timeline shows by empty space is a row of the app's: free time between blocks, and now.
Sittings sit under their block as a second level, folded as the app folds them. Every line is
the core's, in this device's clock; the day shown, moving between days and today opening on
now are the app's.
"""

from __future__ import annotations

from BTSpeak import dialogs

import actions
from actions import L
from rows import Row, items
from session import Command, Session, empty_then, heading, live_menu, screen
import proposals  # noqa: F401 - Replan from now's and Plan the day's screen
import tasks


#: A block form's yes-or-no fields, as a choice.
YES_NO = {"yes": "Yes", "no": "No"}


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
    Dot 7); the day's own actions, moving between days and adding a block are on the main
    menu (M-Chord), and on their keys.
    """

    def move(step) -> str:
        said = session.send(L.ShellEventDay(step))
        # Go to Day asks which; the app's question.
        return actions.joined(said, actions.follow(session))

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
            app=lambda: [
                *day_commands(session),
                Command("Add a block", lambda _: add_block(session), key="a"),
                Command("Previous day", lambda _: move(L.DayMove.PREVIOUS), key="p"),
                Command("Next day", lambda _: move(L.DayMove.NEXT), key="n"),
                Command("Go to a day", lambda _: move(L.DayMove.ASK), key="g"),
                Command("Today", lambda _: move(L.DayMove.TODAY), key="t"),
                *tasks.undo_commands(session),
            ],
            empty="Nothing planned.",
            app_title="Day menu",
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
                key=actions.DAY_KEYS.get(action.kind, ""))
        for action in (day.offered if day is not None else [])
    ]


def add_block(session: Session) -> str:
    """One block, once or repeating, in the app's block form: it starts when the core says for
    its day — today the next whole hour, another day when the day starts."""
    session.send(L.ShellEventBlock(L.BlockEventNew(None, None)))
    return actions.follow(session)


# ---------------------------------------------------------------------------------------
# Blocks
# ---------------------------------------------------------------------------------------


def blocks(session: Session) -> str:
    """Every block series: Enter edits one, the delete keys delete it, a adds one."""
    state = {"heading": "Blocks"}

    def build():
        session.show(L.PlaceBlocks())
        rows = tasks.list_rows(session)
        state["heading"] = heading("Blocks", tasks.readback(session), bool(rows))
        return items(rows)

    with screen("lumenna-day"):
        live_menu(
            session, build, lambda: state["heading"],
            main=lambda row: actions.press(session, "open", row),
            context=lambda rows: actions.commands(session, rows),
            app=[Command("Add a block", lambda _: add_block(session), key="a"), *tasks.undo_commands(session)],
            empty=empty_then(lambda: tasks.readback(session), "Press a to add one."),
            app_title="Blocks menu",
            focus=lambda: session.view().focus,
        )
    return ""


# ---------------------------------------------------------------------------------------
# The block form: the app's (`ViewModel::block_form`), filled in with the device's form.
# ---------------------------------------------------------------------------------------


def value_of(form, key: str):
    """What the form shows in field `key`: the day it is for, or one of its values."""
    return form.date if key == "date" else getattr(form.values, key, "")


def block_form(session: Session) -> str:
    """The block form the app has open — a new block, a block changed for one day or every
    day, a block in free time — asked with the device's form. Each field changed is sent as
    typed, then saved; the app sends only what changed, brings a kind's own settings with it,
    and moves a start left as offered to the day typed. A refused form is asked again with the
    app's sentence. What was said."""
    while True:
        form = session.view().block_form
        if form is None:
            return ""
        if form.refusal:
            dialogs.show_message(form.refusal)
        fields = []
        for field in form.fields:
            value = value_of(form, field.key)
            if field.kind == L.FieldKind.TOGGLE:
                fields.append(dialogs.InputField(
                    key=field.key, prompt=field.label, field_type="choice", choices=YES_NO,
                    default_text="yes" if value else "no",
                ))
            else:
                fields.append(tasks.form_field(field, str(value)))
        answers = dialogs.request_form(fields)
        if answers is None:
            return session.send(L.ShellEventBlock(L.BlockEventCancel()))
        for field in form.fields:
            typed = answers.get(field.key)
            before = value_of(form, field.key)
            if field.kind == L.FieldKind.TOGGLE:
                # Sent as the app takes a toggle.
                typed = None if typed is None else "true" if typed == "yes" else "false"
                shown = "true" if before else "false"
            else:
                shown = str(before)
            if typed is not None and typed != shown:
                session.send(L.ShellEventBlock(L.BlockEventField(field.key, str(typed))))
        said = session.send(L.ShellEventBlock(L.BlockEventSave()))
        if session.view().block_form is None:
            return said
