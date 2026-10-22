"""What every menu shares: the app, what it last showed, and what it said.

Kept apart from `menus.py` so that the menus for tasks, the day, organising and settings can
each use it without importing one another.
"""

from __future__ import annotations

import contextlib
import queue
import time
from pathlib import Path

from BTSpeak import dialogs, host

from core import Core, LumennaError, native


#: How often a menu wakes to notice that the app has something new to show.
#:
#: The app's turns arrive on a thread of its own and set a flag; this only decides how long a
#: stale list can sit on screen. One second is what the dialog library itself uses whenever any
#: item has an idle or hint handler, so it is the device's own normal.
REFRESH = 1

#: How often the store is asked whether another process wrote, or a minute turned: what the
#: daemon's sync brings in, `lum` from a shell, the day's "now".
TICK = 5.0


def sentences(*parts: str) -> str:
    """What several things said, as one thing to say: each its own sentence, ended once."""
    parts = [part.strip() for part in parts if part and part.strip()]
    if len(parts) == 1:
        return parts[0]
    return " ".join(part if part.endswith((".", "?", "!")) else f"{part}." for part in parts)


class Session:
    """The app, as every menu reaches it.

    `send` runs what the person did and waits until the app is still, then hands back what it
    said, for the menu to say; the view it left is `view()`, its lines `lines()`. A turn the
    app leaves at any other time — a sync's arrivals, a pairing's words — marks the screen
    restless, so the menu shown is rebuilt from it.
    """

    def __init__(self, core: Core) -> None:
        self.core = core
        self.app = core.app
        self.device = core.device
        self.lumenna = native()
        self._view = None
        self._lines = None
        #: Files the app made for the person to keep — an export — not yet saved.
        self.files: list = []
        self._shown = None
        #: What the app said that nothing has said yet.
        self._unsaid: list[str] = []
        #: Whether a reminder list is open, so an action run from it does not open it again.
        self.reminders_open = False
        self._ticked = time.monotonic()
        import rows  # here: rows reads the device's settings

        self.send(
            self.lumenna.ShellEventClock(
                self.lumenna.TimeStyle.TWELVE_HOUR if rows.twelve_hour() else self.lumenna.TimeStyle.TWENTY_FOUR_HOUR
            ),
            # Every capital costs a braille cell, and the device's own menus are in sentence case.
            self.lumenna.ShellEventCapitals(self.lumenna.Capitals.SENTENCE),
            # What pairing's sentences call this device, which finds others on its network.
            self.lumenna.ShellEventPairingAs("this device", True),
            # A place to show from the start: the main menu shows none, and without one the
            # screen stood on every task, the whole list worked out and handed over for nothing
            # (two seconds of a Pi's start on 2,000 tasks). Today is the cheapest, and first used.
            self.lumenna.ShellEventOpen(self.lumenna.PlaceToday()),
        )

    # -- turns ------------------------------------------------------------------------

    def _take(self) -> None:
        """Takes in what has arrived since last asked: the view and lines it leaves, files to
        save, and what it said, kept until it is said."""
        while True:
            try:
                turn = self.core.turns.get_nowait()
            except queue.Empty:
                break
            self._view, self._lines = turn.view, turn.lines
            self.files.extend(turn.files)
            self._unsaid.extend(turn.said)

    def _said_since(self, mark: int) -> str:
        """What the app said after `mark`, taken off what is left to say."""
        said = self._unsaid[mark:]
        del self._unsaid[mark:]
        return sentences(*said)

    def send(self, *events) -> str:
        """Runs `events` through the app in order, and says what it said, as one sentence each."""
        self._take()
        mark = len(self._unsaid)
        for event in events:
            self.app.send(event)
        self.core.still()
        self._take()
        return self._said_since(mark)

    def wait_said(self, timeout: float) -> str:
        """Waits up to `timeout` seconds for the app to say something unprompted — what reaches
        other devices answers later than `send` waits — and says it."""
        deadline = time.monotonic() + timeout
        self._take()
        while not self._unsaid and time.monotonic() < deadline:
            time.sleep(0.05)
            self._take()
        return self._said_since(0)

    def view(self):
        """What the app shows now."""
        self._take()
        if self._view is None:
            self._view, self._lines = self.app.view(), self.app.lines()
        return self._view

    def lines(self):
        """The view's lines, in this device's clock."""
        self.view()
        return self._lines

    def ready(self) -> None:
        """Waits for the store to arrive: there is nothing to show until it has."""
        deadline = time.monotonic() + 30
        while not self.view().ready:
            if time.monotonic() > deadline:
                raise LumennaError("Lumenna's store did not open")
            self.core.still()
            time.sleep(0.01)

    def tick(self) -> str:
        """Takes in what another process wrote, or a new minute, and says what it said."""
        self.app.tick()
        return self.send()

    def show(self, place, query: str | None = None) -> None:
        """Makes `place` the one the app shows, with `query` typed over it: a screen opening, or
        coming back from one opened from it."""
        wanted = (place, query)
        if self._shown != wanted or self.view().place != place:
            self.send(self.lumenna.ShellEventOpen(place))
            if query is not None:
                self.send(self.lumenna.ShellEventFilter(query))
            self._shown = wanted

    def sentence(self, text: str) -> str:
        """Fixed text — a button, an answer — in sentence case, as the core writes it."""
        return self.lumenna.sentence_case(text) if text else text

    # -- the screen's freshness ----------------------------------------------------------

    def restless(self) -> bool:
        """Whether the app has shown something new since the screen was drawn."""
        if time.monotonic() - self._ticked > TICK:
            self._ticked = time.monotonic()
            self.app.tick()
        return self.core.arrived.is_set()

    def settle(self) -> bool:
        """Whether to rebuild, clearing the reason. What the app said unprompted meanwhile — a
        sync's report, a pairing's end — is said now."""
        changed = self.core.arrived.is_set()
        self.core.arrived.clear()
        self._take()
        for text in self._unsaid:
            announce(text)
        self._unsaid.clear()
        return changed


def announce(text: str) -> None:
    """Something the app said that no action of the person's asked for: spoken and brailled
    as the device's own services say things, without a dialog in the way."""
    if text:
        host.say_brl(text)


class Command:
    """Something that can be done: to the row under the cursor, offered on M-Chord with Dot 7,
    or to the whole screen, offered on M-Chord — the device's two menus.

    `key` is a lowercase letter that does the same from the list, as the device's own apps
    have them; the menus say it after the label. A capital letter, Dot 7 with the letter, still
    moves to the next row starting with it. `applies` says which rows a context command is for;
    `deletes` makes it what the delete keys do — Control-D and the D chord. `by_key`, when
    given, is what the letter does instead of `run`: asking which, where a row has several.
    `refuse(row)`, when given, says why the letter does nothing on a row it does not apply to.
    """

    def __init__(self, label, run, key: str = "", applies=None, deletes: bool = False, by_key=None,
                 refuse=None) -> None:
        self.label = label
        self.run = run
        self.key = key
        self.applies = applies or (lambda row: True)
        self.deletes = deletes
        self.by_key = by_key or run
        self.refuse = refuse

    def label_for(self, row) -> str:
        return self.label(row) if callable(self.label) else self.label


def row_of(item):
    """The data a menu row stands for, or None for a row that stands for nothing — the line
    an empty list shows in place of rows."""
    return getattr(item, "row", None)


def _menu(items, braille_title: str = "", **arguments):
    """The device's menu, its T-Chord showing `braille_title` on the display where it is given:
    the dialog takes one title, said as the menu opens and drawn at the top of the screen, and
    T-Chord says it again and pushes it to the display, so the shorter braille needs the key
    answered here, as the device's file manager answers it."""
    if not braille_title:
        return dialogs.dynamic_menu(items, **arguments)

    class BrailledTitle(dialogs.DynamicMenuDialog):
        def on_TitleRead(self) -> bool:
            host.say(self.title or "", immediate=True, as_ui=True)
            try:
                from BTSpeak import brl
                brl.push_message(braille_title)
            except Exception:  # noqa: BLE001 - no display, or a library without the call: said only
                pass
            return True

    return BrailledTitle(menu=items, **arguments).run()


def live_menu(
    session: Session, build, title, default: int = 0, moved=None, *,
    main=None, context=(), app=(), empty="Nothing here", app_title: str = "", focus=None, until=None,
    braille_title=None,
) -> None:
    """A menu rebuilt whenever the store moves, reopened where the cursor was.

    `build()` returns the rows (menu items carrying a `row`, `rows.items`), or a string to say instead of opening at all.
    `main(row)` is what Enter does on a row; `context` and `app` are `Command`s for the two
    menus, and `context` may be a function of the rows giving them, for commands that are the
    rows' own actions (`actions.commands`); `app` may be a function of nothing, read after
    `build`, for a screen's own actions that come with what it shows (the day's). `title` is a string or a function of nothing, read on each rebuild, and so is
    `braille_title`, what T-Chord shows on the display where it is shorter than what it says; and
    so is `empty`, what an empty list says: the listing's own `empty` (`Rows::empty`), read
    after `build`. `moved`, when
    given, is another reason to rebuild — the day changing under the planner — and is cleared
    here. Losing your place in a list when it refreshes is the sort of thing that makes a UI
    unusable without sight: `focus()`, when given, is the key of the row the app has in hand,
    which the menu reopens on — the same row after a change, else the one now in its place, as
    the app keeps it — and otherwise the selection is kept by position. `until()`, when given,
    closes the menu once it holds: the app has put away what the menu shows.
    """
    selection = default
    while True:
        items = build()
        # What arrived before this draw, building it included, is in what it draws.
        session.settle()
        if focus is not None and not isinstance(items, str):
            held = focus()
            selection = next((i for i, item in enumerate(items) if getattr(row_of(item), "key", None) == held), selection)
        if isinstance(items, str):
            if items:
                dialogs.show_message(items)
            return
        here = context([row_of(item) for item in items if row_of(item) is not None]) if callable(context) else context
        offered = app() if callable(app) else app
        wire(items, main, here, offered)
        heading = title() if callable(title) else title
        brailled = braille_title() if callable(braille_title) else braille_title
        choice = _menu(
            items,
            braille_title=brailled if brailled and brailled != heading else "",
            title=heading,
            exit_condition=lambda: session.restless() or bool(moved and moved()) or bool(until and until()),
            refresh_interval=REFRESH,
            default=min(selection, max(len(items) - 1, 0)),
            context_menu=context_commands(here),
            app_menu=app_commands(offered),
            app_menu_title=app_title or f"{heading.split(',')[0]} menu",
            global_keys=keys(here, offered),
            empty_message=empty() if callable(empty) else empty,
        )
        nudged = bool(moved and moved(clear=True))
        if until is not None and until():
            session.settle()
            return
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
    row under the cursor first, then one for the screen. A letter that does neither says why,
    rather than leave the person guessing."""
    letters = {command.key for command in [*context, *app] if command.key}

    def run(key, item):
        row = row_of(item)
        for command in context:
            if command.key == key and row is not None and command.applies(row):
                return command.by_key(row)
        for command in app:
            if command.key == key:
                return command.run(None)
        for command in context:
            if command.key == key and command.refuse is not None:
                return command.refuse(row)
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


def choose(options: dict, prompt: str, default=None, hints=None):
    """One choice from `options`, key to label; the key, or None if cancelled.

    `hints`, key to text, is why a choice is offered ("Suggested because ..."): its
    description, never its name. A choice dialog has no description, so where any choice has
    one the choices are a menu instead, each row's hint said after it, as the device says a
    menu's hints."""
    if not options:
        return None
    hints = {key: hint for key, hint in (hints or {}).items() if hint and key in options}
    if hints:
        picked = []

        def pick(key):
            def run(menu) -> None:
                picked.append(key)
                menu.close()
            return run

        dialogs.dynamic_menu(
            [dialogs.DynamicMenuItem(title=label, action=pick(key), hint=hints.get(key)) for key, label in options.items()],
            title=prompt,
            default=list(options).index(default) if default in options else 0,
        )
        return picked[0] if picked else None
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


def heading(title: str, readback: str, shown: bool) -> str:
    """A list's heading: its title, then what the list says of itself (the core's readback:
    the query understood, the count, its notices), unless nothing is listed, which the empty
    list says instead, once."""
    return ", ".join(part for part in (title, readback if shown else "") if part)


def empty_then(said, then: str = ""):
    """What an empty list says: the list's own words (`said()`, the core's readback), then
    what this screen adds, such as the key that adds one."""
    return lambda: sentences(said(), then)
