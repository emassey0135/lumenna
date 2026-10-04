"""What every menu shares: the client, whether the store moved, and how a result is said.

Kept apart from `menus.py` so that the menus for tasks, the day, organising and settings can
each use it without importing one another.
"""

from __future__ import annotations

from BTSpeak import dialogs

from client import LumennaError


#: How often a menu wakes to notice that the store changed underneath it.
#:
#: §16.11 warns that live refresh costs battery, and prefers pushed updates. This is the
#: cheap half of both: the push has already arrived and set a flag, and this only decides how
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

        Every message the server sends is written to be spoken — §6.3 says the text is the
        only channel, there being no squiggle to point at — so it is passed through rather
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


def live_menu(session: Session, build, title, default: int = 0, moved=None) -> None:
    """A menu rebuilt whenever the store moves, reopened where the cursor was.

    `build()` returns the items, or a string to say instead when there is nothing to show.
    `title` is a string or a function of nothing, read on each rebuild. `moved`, when given,
    is another reason to rebuild — the day changing under the planner — and is cleared here.
    Losing your place in a list when it refreshes is the sort of thing that makes a UI
    unusable without sight, so the selection is kept by position.
    """
    selection = default
    while True:
        items = build()
        if isinstance(items, str):
            if items:
                dialogs.show_message(items)
            return
        heading = title() if callable(title) else title
        choice = dialogs.dynamic_menu(
            items,
            title=heading,
            exit_condition=lambda: session.restless() or bool(moved and moved()),
            refresh_interval=REFRESH,
            default=min(selection, max(len(items) - 1, 0)),
        )
        nudged = bool(moved and moved(clear=True))
        if not session.settle() and not nudged:
            # The menu closed because the user left it, not because anything moved.
            return
        selection = choice.key if choice else 0


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
