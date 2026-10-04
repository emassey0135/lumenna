"""The app: the main menu, and the check that the server is one this app understands.

Everything reachable here goes through `lum rpc`'s typed surface (§12). Nothing parses prose,
nothing computes a due date, nothing decides what completing a task does to its subtasks —
that is all core's, and a UI that reimplemented any of it would be the business-logic leak
principle 2 forbids.

The menus themselves live beside this file: `tasks.py`, `day.py` for the planner and blocks,
`organise.py` for projects, labels and filters, and `preferences.py` for settings, devices
and data. They are laid out as the phone's are, so the two can be described in one breath.

Three device conventions this follows rather than invents:

- **Left and right fold a branch.** They are already bound to left-arrow/Dot7 and
  right-arrow/Dot8, so a tree needs no key handling of its own (§16.11).
- **The delete keys delete.** Control-D and the D chord act on the row under the cursor, as
  they do in the device's own lists; Enter offers everything else.
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


#: The JSON shapes this app was written against (§15). The server reports its own; if they
#: disagree, one of the two is guessing, and §15 says a client that does not know the number
#: should refuse rather than guess.
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
            item(title="Redo", shortcut="r", action=lambda: session.write("redo")),
            item(title="Sync now", shortcut="y", action=lambda: preferences.sync_now(session)),
            item(title="Settings", shortcut="s", action=lambda: preferences.settings(session)),
        ],
        title="Lumenna",
    )
