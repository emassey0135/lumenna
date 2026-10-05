"""The app: the main menu, and the check that the server is one this app understands.

Everything reachable here goes through `lum rpc`'s typed surface. Nothing parses prose,
nothing computes a due date, nothing decides what completing a task does to its subtasks —
that is all core's, so that every client behaves alike and none can drift.

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

from client import Disconnected
from session import Session
import day
import organise
import preferences
import tasks


#: The JSON shapes this app was written against. The server reports its own; if they
#: disagree, one of the two is guessing, and a client that does not know the number should
#: refuse rather than guess.
CONTRACT = 1

#: Methods this app calls that a server from before them would not answer. Asked for at the
#: start, so an old `lum` is a sentence then rather than a failure halfway through a pairing.
NEEDS = ("pair", "pair.confirm", "block.show", "length")


def run(client) -> int:
    """Checks the server is one we understand, then hands over to the main menu."""
    try:
        server = client.call("initialize")
    except Disconnected as error:
        dialogs.show_message(f"Could not start Lumenna. {error.message}")
        return 1

    if server.get("contract") != CONTRACT:
        dialogs.show_message(
            f"This app speaks version {CONTRACT} of Lumenna's data format and the installed "
            f"`lum` speaks version {server.get('contract')}. Update whichever is older; "
            "guessing at the difference would be worse than stopping."
        )
        return 1
    missing = [method for method in NEEDS if method not in server.get("methods", NEEDS)]
    if missing:
        dialogs.show_message(
            f"The installed `lum` is older than this app and cannot {', '.join(missing)}. "
            "Update it to use everything here."
        )

    session = Session(client)
    try:
        main_menu(session)
    except Disconnected as error:
        dialogs.show_message(error.message)
        return 1
    return 0


def main_menu(session: Session) -> None:
    """Everything else, one level down."""
    item = dialogs.DynamicMenuItem
    dialogs.dynamic_menu(
        [
            item(title="Today", shortcut="t", action=lambda: day.day_plan(session)),
            item(title="Tasks", shortcut="k", action=lambda: tasks.task_list(session)),
            item(title="Add a task", shortcut="a", action=lambda: tasks.add_task(session)),
            item(title="Filters and search", shortcut="f", action=lambda: organise.saved_filters(session)),
            item(title="Projects", shortcut="p", action=lambda: organise.projects(session)),
            item(title="Labels", shortcut="l", action=lambda: organise.labels(session)),
            item(title="Blocks", shortcut="b", action=lambda: day.blocks(session)),
            item(title="Trash", shortcut="x", action=lambda: tasks.trash(session)),
            item(title="Undo", shortcut="u", action=lambda: session.write("undo")),
            item(title="Redo", shortcut="y", action=lambda: session.write("redo")),
            item(title="Sync now", shortcut="n", action=lambda: preferences.sync_now(session)),
            item(title="Settings", shortcut="s", action=lambda: preferences.settings(session)),
        ],
        title="Lumenna",
    )
