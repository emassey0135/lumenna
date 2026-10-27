"""The app: the main menu.

Everything reachable here is the Crux app's, run in this process beside the store (`core.py`).
Nothing parses prose, nothing computes a due date, nothing decides what completing a task does
to its subtasks, which row is in hand after a change or what a question offers: that is all the
core's and the app's, so that every client behaves alike and none can drift.

The menus themselves live beside this file: `tasks.py`, `day.py` for the planner and blocks,
`organise.py` for projects, labels and filters, and `preferences.py` for settings, devices
and data. They are laid out as the phone's are, so the two can be described in one breath.

The device's conventions, followed rather than invented:

- **Enter does a row's main thing**: shows a project's tasks, a task's details, edits a block.
- **M-Chord with Dot 7 is the row's context menu**, with everything else that can be done to
  it; **M-Chord is the screen's main menu**: adding, moving between days, undo and redo.
  Both are `Command`s (`session.py`), and each command's lowercase letter does the same from
  the list — the same letter for the same thing on every screen. A capital, Dot 7 with the
  letter, still moves by first letter.
- **The delete keys delete.** Control-D and the D chord act on the row under the cursor.
- **H-Chord is help about the screen in front**: the app's own topics, in `help/`.
- **Left and right fold a branch.** They are already bound to left-arrow/Dot7 and
  right-arrow/Dot8, so a tree needs no key handling of its own.
- **Nothing is announced twice.** A menu action's return value is spoken by the menu itself,
  so an action that has something to say returns it rather than opening a dialog.
"""

from __future__ import annotations

from BTSpeak import dialogs

from actions import L
from core import Core, LumennaError
from session import Session, announce, live_menu
import day
import organise
import preferences
import reminder_lists
import reminders
import tasks


def run(core: Core) -> int:
    """Waits for the store, then hands over to the main menu."""
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
        main_menu(session)
    finally:
        giving()
        core.app.in_front(False)
    return 0


def opens(entry):
    """The letter and screen for one of the sidebar's top-level rows, or None for one this app
    does not show."""
    kind = entry.kind
    if isinstance(kind, L.SidebarKindGroup):
        return {
            L.SidebarGroup.PROJECTS: ("p", organise.projects),
            L.SidebarGroup.LABELS: ("l", organise.labels),
            L.SidebarGroup.FILTERS: ("f", organise.saved_filters),
        }.get(kind.field_0)
    place = kind.field_0
    if isinstance(place, L.PlaceToday):
        return "t", day.day_plan
    if isinstance(place, L.PlaceTasks):
        return "k", tasks.task_list
    if isinstance(place, L.PlaceBlocks):
        return "b", day.blocks
    if isinstance(place, L.PlaceHistory):
        return "h", tasks.history
    if isinstance(place, L.PlaceTrash):
        return "x", tasks.trash
    return None


def main_menu(session: Session) -> None:
    """The app's places, as every app's sidebar lists them, one level down — each project,
    label and filter is in its own list — then what this app adds: adding a task, undo and
    redo, reminders, syncing, settings."""
    item = dialogs.DynamicMenuItem

    def build():
        places = []
        for entry in session.view().sidebar:
            found = opens(entry) if entry.depth == 0 else None
            if found is not None:
                key, screen = found
                places.append(item(title=entry.text, shortcut=key, action=lambda screen=screen: screen(session)))
        # The task list sits beside adding a task, as it always has here.
        at = next((i + 1 for i, it in enumerate(places) if it.shortcut == "k"), len(places))
        places.insert(at, item(title="Add a task", shortcut="a", action=lambda: add_task(session)))
        return places + [
            item(title="Undo", shortcut="u", action=lambda: session.send(L.ShellEventUndo())),
            item(title="Redo", shortcut="y", action=lambda: session.send(L.ShellEventRedo())),
            item(title="Reminders", shortcut="r", action=lambda: reminder_lists.due(session)),
            item(title="Sync now", shortcut="n", action=lambda: preferences.sync_now(session)),
            item(title="Settings", shortcut="s", action=lambda: preferences.settings(session)),
        ]

    live_menu(session, build, "Lumenna", app_title=L.menu_of("Lumenna"))


def add_task(session: Session) -> str:
    """A task added from the main menu goes in the Inbox, as the task list's own would."""
    session.show(L.PlaceTasks())
    return tasks.add_task(session, session.view().quick_add)
