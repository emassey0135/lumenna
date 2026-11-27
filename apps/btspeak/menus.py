"""The app: Home, and the screen each place opens.

Everything reachable here is the Crux app's, run in this process beside the store (`core.py`).
Nothing parses prose, nothing computes a due date, nothing decides what completing a task does
to its subtasks, which row is in hand after a change or what a question offers: that is all the
core's and the app's, so that every client behaves alike and none can drift.

The menus themselves live beside this file: `tasks.py`, `day.py` for the planner and blocks,
and `preferences.py` for settings, devices and data. Projects, labels and saved filters are
Home's own rows, under their headings, each with its own actions.

The device's conventions, followed rather than invented:

- **The app opens on Home** (`ViewModel::home`): the places, as every app's sidebar lists them,
  each with its letter, then what can be done from anywhere on its main menu.
- **Enter does a row's main thing**, the app's `Key("open")` on every row: opens a place, shows a
  task's details, edits a block, restores a trashed task, renames a device.
- **M-Chord with Dot 7 is the row's context menu**, with everything else that can be done to
  it; **M-Chord is the screen's main menu**: what the screen offers of its own, the core's
  (`ViewModel::screen_actions`). Both are `Command`s (`session.py`), and each command's
  lowercase letter does the same from the list — the same letter for the same thing on every
  screen. A capital, Dot 7 with the letter, still moves by first letter.
- **The delete keys delete.** Control-D and the D chord act on the row under the cursor.
- **H-Chord is help about the screen in front**: the app's own topics, in `help/`.
- **Left and right fold a branch.** They are already bound to left-arrow/Dot7 and
  right-arrow/Dot8, so a tree needs no key handling of its own.
- **Nothing is announced twice.** A menu action's return value is spoken by the menu itself,
  so an action that has something to say returns it rather than opening a dialog.
"""

from __future__ import annotations

from BTSpeak import dialogs

import actions
from actions import L
from core import Core, LumennaError, words
from rows import Row, items
from session import Command, Session, announce, live_menu, screen
import day
import reminders
import tasks


def run(core: Core) -> int:
    """Waits for the store, then hands over to Home."""
    session = Session(core)
    try:
        session.ready()
    except LumennaError as error:
        dialogs.show_message(error.message)
        return 1
    core.back_up_while_running(failed=announce)
    # Syncing runs while the app does, beside the daemon or in its place; so does giving
    # reminders, unless the background service already gives them.
    core.app.in_front(True)
    giving = reminders.give_in_background(reminders.Giver(open_store=lambda: core.device))
    try:
        home(session)
    finally:
        giving()
        core.app.in_front(False)
    return 0


def place_screen(place):
    """The screen that shows `place`: the day, a list of tasks, the blocks."""
    if isinstance(place, L.PlaceToday):
        return day.day_plan
    if isinstance(place, L.PlaceBlocks):
        return day.blocks
    if isinstance(place, L.PlaceHistory):
        return tasks.history
    if isinstance(place, L.PlaceUpcoming):
        return tasks.upcoming
    if isinstance(place, L.PlaceReport):
        return tasks.report
    if isinstance(place, L.PlaceTrash):
        return tasks.trash
    return lambda session: tasks.task_list(session, place)


def home_rows(session: Session) -> list[Row]:
    """Home's places, the app's (`HomeView::places`), not folded away: each its line, its level
    where it changes, whether what is under it is folded, its letter, and its own actions."""
    view = session.view().home
    made = []
    for place in (view.places if view is not None else []):
        entry = place.entry
        if entry.hidden:
            continue
        row = Row(entry.key, entry.text, entry.offered, place, level=entry.level, expanded=entry.expanded,
                  title=entry.title)
        row.side = L.Side.SIDEBAR
        row.letter = place.letter
        made.append(row)
    return made


def home(session: Session) -> None:
    """Home: the places, each opened by Enter or its letter, then what can be done from anywhere,
    on the main menu (M-Chord). Opening a place leaves Home in the app; its screen is shown here
    until the person leaves it, and Home is shown again."""

    def build():
        if session.view().home is None:
            session.send(L.ShellEventHome())
        made = items(home_rows(session), fold=lambda row, open_: actions.fold(session, row, open_))
        for item in made:
            # The place's letter, said after it as the device says a shortcut; pressed, it runs
            # the row's Enter, which opens the place, as the letter does.
            item.shortcut = item.row.letter or None
        return made

    def went() -> bool:
        """Whether the app has left Home for a place, which is then shown."""
        view = session.view()
        return view.ready and view.home is None and view.place is not None

    def commands():
        view = session.view().home
        own = actions.screen_commands(session, tasks.own_commands(session))
        # Each place's letter runs as the app runs it (`Event::Letter`): never a menu entry.
        places = [
            Command("", lambda _: "", key=place.letter, through=True, listed=False)
            for place in (view.places if view is not None else []) if place.letter
        ]
        return [*own, *places]

    def context(rows):
        named = words()
        return actions.commands(session, rows, before=[
            Command(lambda row: added_title(row, named), lambda row: add_in(session, row),
                    applies=lambda row: added_title(row, named) != ""),
        ])

    name = words().metadata.name
    while True:
        with screen("lumenna"):
            live_menu(
                session, build, lambda: session.view().home.title if session.view().home else name,
                main=lambda row: actions.press(session, "open", row),
                context=context,
                app=commands,
                app_title=L.menu_of(name),
                focus=lambda: session.view().sidebar_held.focus,
                until=went,
                screen_first=True,
                bar=(None, True),
            )
        session.moved_on = False
        if session.leaving or not went():
            return
        # The place opened, then wherever the menu bar takes the app from there, until the person
        # leaves a place or the app goes Home.
        while went():
            place_screen(session.view().place)(session)
            if session.leaving:
                return
            if not session.moved_on:
                break
            session.moved_on = False


def added_title(row, named) -> str:
    """What adding a task to a place on Home is called: a project's or a filter's "Add task to
    it", a label's "Add task wearing it"; none for anything else."""
    kind = row.record.entry.kind
    place = kind.field_0 if isinstance(kind, L.SidebarKindPlace) else None
    if isinstance(place, L.PlaceLabel):
        return named.add_task_wearing_it
    if isinstance(place, (L.PlaceProject, L.PlaceFilter)):
        return named.add_task_to_it
    return ""


def add_in(session: Session, row) -> str:
    """Adds a task in the place on Home's `row`, starting the line as the app says a task added
    there does, then shows Home again."""
    place = row.record.entry.kind.field_0
    session.show(place)
    try:
        return tasks.add_task(session)
    finally:
        session.send(L.ShellEventHome())


def key_help(session: Session) -> str:
    """Key help, the core's for a BTSpeak over the screen in front (`App::key_help_on`): this
    device's own keys, Home's places, the screen's commands and the rows' actions, each by its
    letter, a line each to pan through. H-Chord's topics (`help/`) are the guide beside it."""
    session.view()
    shown = session.app.key_help_on(0, L.KeyDevice.BT_SPEAK, True)
    dialogs.view_lines(list(shown.lines), wrap=True)
    return ""


def about(session: Session) -> str:
    """About, the shell's own: what Lumenna is, from the one source every platform says
    (`words().metadata`), a line each to pan through."""
    metadata = words().metadata
    dialogs.view_lines(
        [line for line in (metadata.name, metadata.version, metadata.description, metadata.homepage,
                           metadata.publisher, metadata.licence) if line],
        wrap=True,
    )
    return ""
