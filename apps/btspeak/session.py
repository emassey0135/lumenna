"""What every menu shares: the client, whether the store moved, and how a result is said.

Kept apart from `menus.py` so that the menus for tasks, the day, organising and settings can
each use it without importing one another.
"""

from __future__ import annotations

import contextlib
from pathlib import Path

from BTSpeak import dialogs, host

from client import LumennaError


#: How often a menu wakes to notice that the store changed underneath it.
#:
#: Polling the store would cost battery, so changes are pushed instead. This is the cheap
#: half of both: the push has already arrived and set a flag, and this only decides how
#: long a stale list can sit on screen. One second is what the dialog library itself uses
#: whenever any item has an idle or hint handler, so it is the device's own normal.
REFRESH = 1


def spoken(result: dict) -> str:
    """A reply as one thing to say: the announcement, then anything worth adding.

    Notices are part of the answer — "new label errands", "kept the title, changed since" —
    so they are said with it rather than dropped.
    """
    parts = [result.get("announcement", "")] + list(result.get("notices", []))
    return ". ".join(part for part in parts if part)


class Session:
    """A client, plus the bookkeeping the menus share."""

    def __init__(self, client) -> None:
        self.client = client
        self.dirty = False

    def call(self, method: str, **params):
        """Calls, and turns a refusal into something worth hearing.

        Every message the server sends is written to be spoken — the text is the only
        channel, there being no squiggle to point at — so it is passed through rather
        than wrapped in something of ours.
        """
        return self.client.call(method, **params)

    def write(self, method: str, **params) -> str:
        """Calls something that changes the store, and says what it did — or why not."""
        try:
            result = self.call(method, **params)
        except LumennaError as error:
            return error.message
        self.wrote()
        return spoken(result)

    def wrote(self) -> None:
        """Records that this app changed something.

        Our own writes do not come back as `lumenna/changed`: `data_version` does not move
        for the connection that wrote, which is the point of that pragma. So a list refreshes
        after our edits because we say so, and after anyone else's because the server does.
        """
        self.dirty = True

    def restless(self) -> bool:
        """Whether anything has changed under whatever is on screen."""
        return self.dirty or self.client.changed.is_set()

    def settle(self) -> bool:
        """Whether to rebuild, clearing the reason."""
        changed = self.dirty or self.client.take_changed()
        self.dirty = False
        return changed


class Command:
    """Something that can be done: to the row under the cursor, offered on M-Chord with Dot 7,
    or to the whole screen, offered on M-Chord — the device's two menus.

    `key` is a lowercase letter that does the same from the list, as the device's own apps
    have them; the menus say it after the label. A capital letter, Dot 7 with the letter, still
    moves to the next row starting with it. `applies` says which rows a context command is for;
    `deletes` makes it what the delete keys do — Control-D and the D chord. `by_key`, when
    given, is what the letter does instead of `run`: asking which, where a row has several.
    """

    def __init__(self, label, run, key: str = "", applies=None, deletes: bool = False, by_key=None) -> None:
        self.label = label
        self.run = run
        self.key = key
        self.applies = applies or (lambda row: True)
        self.deletes = deletes
        self.by_key = by_key or run

    def label_for(self, row) -> str:
        return self.label(row) if callable(self.label) else self.label


def row_item(row, title=None, action=None, **fields) -> dialogs.DynamicMenuItem:
    """A menu row carrying the data it stands for, which the commands are given."""
    item = dialogs.DynamicMenuItem(title=title if title is not None else row["title"], action=action, **fields)
    item.row = row
    return item


def row_of(item):
    """The data a menu row stands for, or None for a row that stands for nothing — the line
    an empty list shows in place of rows."""
    return getattr(item, "row", None)


def live_menu(
    session: Session, build, title, default: int = 0, moved=None, *,
    main=None, context=(), app=(), empty: str = "Nothing here", app_title: str = "",
) -> None:
    """A menu rebuilt whenever the store moves, reopened where the cursor was.

    `build()` returns the rows (`row_item`s), or a string to say instead of opening at all.
    `main(row)` is what Enter does on a row; `context` and `app` are `Command`s for the two
    menus, and `context` may be a function of the rows giving them, for commands that are the
    rows' own actions (`actions.commands`). `title` is a string or a function of nothing, read on each rebuild. `moved`, when
    given, is another reason to rebuild — the day changing under the planner — and is cleared
    here. Losing your place in a list when it refreshes is the sort of thing that makes a UI
    unusable without sight, so the selection is kept by position.
    """
    selection = default
    while True:
        items = build()
        if isinstance(items, str):
            if items:
                dialogs.show_message(items)
            return
        here = context([row_of(item) for item in items if row_of(item) is not None]) if callable(context) else context
        wire(items, main, here, app)
        heading = title() if callable(title) else title
        choice = dialogs.dynamic_menu(
            items,
            title=heading,
            exit_condition=lambda: session.restless() or bool(moved and moved()),
            refresh_interval=REFRESH,
            default=min(selection, max(len(items) - 1, 0)),
            context_menu=context_commands(here),
            app_menu=app_commands(app),
            app_menu_title=app_title or f"{heading.split(',')[0]} menu",
            global_keys=keys(here, app),
            empty_message=empty,
        )
        nudged = bool(moved and moved(clear=True))
        if not session.settle() and not nudged:
            # The menu closed because the user left it, not because anything moved.
            return
        selection = choice.key if choice else 0


def wire(items, main, context, app) -> None:
    """Gives each row its Enter and its delete, from `main` and the deleting command."""
    deleting = [command for command in context if command.deletes]
    for item in items:
        row = row_of(item)
        if row is None:
            continue
        if main is not None and item.action is None:
            item.action = lambda row=row: main(row)
        for command in deleting:
            if command.applies(row):
                item.delete = lambda row=row, command=command: command.run(row)
                break


def context_commands(context):
    """The `Command`s as the device's context menu takes them: each for the rows it applies to."""
    return [
        dialogs.MenuCommand(
            name=f"context-{index}",
            label=lambda dialog, command=command: command.label_for(selected_row(dialog)),
            action=lambda item, command=command: command.run(row_of(item)),
            key=key_label(command.key),
            dependency=lambda dialog, command=command: (
                selected_row(dialog) is not None and command.applies(selected_row(dialog))
            ),
            on_delete_key=command.deletes,
        )
        for index, command in enumerate(context)
    ]


def app_commands(app):
    return [
        dialogs.MenuCommand(
            name=f"app-{index}",
            label=command.label_for(None),
            action=lambda command=command: command.run(None),
            key=key_label(command.key),
        )
        for index, command in enumerate(app)
    ]


def key_label(key: str) -> str:
    """The key as the menus announce it."""
    return {"/": "slash", ",": "comma", ".": "period"}.get(key, key)


def selected_row(dialog):
    if 0 <= dialog.selection < len(dialog.menu):
        return row_of(dialog.menu[dialog.selection])
    return None


def keys(context, app):
    """The letters, each running the first command it names that applies here: one for the
    row under the cursor first, then one for the screen."""
    letters = {command.key for command in [*context, *app] if command.key}

    def run(key, item):
        row = row_of(item)
        for command in context:
            if command.key == key and row is not None and command.applies(row):
                return command.by_key(row)
        for command in app:
            if command.key == key:
                return command.run(None)
        return ""

    return {key: (lambda item, key=key: run(key, item)) for key in letters}


@contextlib.contextmanager
def screen(topic: str):
    """H-Chord's help, about the screen in front: the app's own topic for it, from `help/`."""
    host.push_app_context("lumenna", topic, help_dir=HELP)
    try:
        yield
    finally:
        host.pop_app_context()


#: The app's own help topics, which H-Chord finds ahead of the device's.
HELP = str(Path(__file__).resolve().parent / "help")


def choose(options: dict, prompt: str, default=None):
    """One choice from `options`, key to label; the key, or None if cancelled."""
    if not options:
        return None
    choice = dialogs.request_choice(options, prompt, default=default)
    return None if choice is None else choice.key


def confirm(question: str) -> bool:
    """A yes or no whose default is no, for anything that cannot be taken back lightly."""
    return dialogs.request_confirmation(question, default=False)


def ask(prompt: str, default: str = "") -> str | None:
    """One line of text; None if cancelled or left empty."""
    text = dialogs.request_input(prompt, default_text=default)
    if text is None or not text.strip():
        return None
    return text.strip()


class Flag:
    """A reason to rebuild a menu that is not the store moving: the planner turning to
    another day, say. Called with `clear=True` it answers and resets."""

    def __init__(self) -> None:
        self._set = False

    def set(self) -> None:
        self._set = True

    def __call__(self, clear: bool = False) -> bool:
        was = self._set
        if clear:
            self._set = False
        return was
