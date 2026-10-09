"""Turning rows into menu items.

The server sends components — `role`, `depth`, `index`, `count`, `state`, `title`, `value` —
and never a sentence, because speech and braille compose them differently. This file is
where the composing happens for this device.

**One caveat shapes everything here.** `DynamicMenuDialog.draw()` sets `content_text` and
`content_braille` from the same string, so with the stock library speech and braille cannot
diverge. There are two ways round it: put a compact level marker in the title, or
subclass the dialog and override the selection branch of `draw()`. This uses the marker. If the marker turns out to grate in speech, the subclass is about
twenty lines and `InteractiveSearchDialog` extends `ChoiceDialog`, so it is an intended
pattern rather than a hack.

**Depth is never indentation**, which does not work in speech. The
level is announced *when it changes*, which is what a screen-reader tree view does, and
costs nothing here because titles may be callables — so the marker is computed against the
row above at the moment it is drawn, and stays right as branches collapse.
"""

from __future__ import annotations

import locale
import time

from BTSpeak import dialogs, settings

#: The device's clock setting, read again once this many seconds have passed: a redraw
#: describes every row, and a Pi feels a file read per row.
_CLOCK_FOR = 5.0
_clock_read: tuple[float, str] = (float("-inf"), "")


def _time_format() -> str:
    """The device's own time format, as its say-time command chooses it: the 12- or 24-hour
    clock if the Time Format setting says one, else the locale's, never with seconds."""
    global _clock_read
    read_at, chosen = _clock_read
    now = time.monotonic()
    if now - read_at > _CLOCK_FOR:
        try:
            chosen = settings.getValue("time-format")
        except Exception:  # noqa: BLE001 - a setting the device cannot read is the locale's
            chosen = ""
        _clock_read = (now, chosen)
    if chosen == "12-hour":
        return "%l:%M %p"
    if chosen == "24-hour":
        return "%H:%M"
    try:
        local = locale.nl_langinfo(locale.T_FMT)
    except (AttributeError, ValueError):
        local = "%H:%M:%S"
    local = local.replace("%r", "%l:%M:%S %p").replace("%T", "%H:%M:%S").replace("%I", "%l")
    return local.replace(":%S", "") or "%H:%M"


def clock(hhmm: str) -> str:
    """A time of day, `HH:MM` as the server sends it, as this device says times."""
    try:
        hour, minute = (int(part) for part in hhmm.split(":"))
    except (AttributeError, ValueError):
        return hhmm
    return time.strftime(_time_format(), (2000, 1, 1, hour, minute, 0, 5, 1, -1)).strip()


def due(row: dict) -> str:
    """When a row is due, its time in the device's clock: "due tomorrow at 3:00 PM"."""
    words = row.get("due")
    if not words:
        return ""
    at = row.get("due_time")
    return f"{words} at {clock(at)}" if at else words


def describe(row: dict, with_role: bool = False) -> str:
    """One row, as a line to speak and to braille.

    The title is never abbreviated — braille users get everything speech users get.
    Only the surroundings are compressed.
    """
    parts = [row.get("title", "")]
    if with_role:
        parts.append(row.get("role", ""))
    if row.get("checked") is True:
        parts.append("done")
    parts.append(due(row))
    value = row.get("value")
    if value:
        parts.append(value)
    parts.extend(row.get("state", []))
    return ", ".join(part for part in parts if part)


class Tree:
    """A flat list of rows, read as a tree.

    The projection flattens depth-first and carries `depth`, so a row's parent is the nearest
    row above it that is shallower. That is enough to collapse a branch, and it means the
    client needs no second shape for what the server already sent.

    A task whose parent was filtered out appears at top level rather than vanishing, which is
    the projection's choice and the right one — so a tree here may be several trees.

    **Every row's title is recomputed on every redraw**, because the level marker depends on
    what is folded. Walking up the list to find parents and visible neighbours each time made
    a redraw cost grow with the cube of the list, which a Raspberry Pi feels at a few hundred
    tasks. So parents are found once, in one pass, and what is visible is worked out once per
    set of folded rows and reused until that set changes.
    """

    def __init__(self, rows: list[dict]) -> None:
        self.rows = rows
        self.collapsed: set[str] = set()
        self._parents = self._find_parents(rows)
        self._layout_for: frozenset[str] | None = None
        self._visible: list[bool] = []
        self._previous: list[int | None] = []

    @staticmethod
    def _find_parents(rows: list[dict]) -> list[int | None]:
        """Each row's parent, by keeping the chain of open ancestors as the list is read."""
        parents: list[int | None] = []
        chain: list[int] = []
        for index, row in enumerate(rows):
            depth = row.get("depth", 0)
            while chain and rows[chain[-1]].get("depth", 0) >= depth:
                chain.pop()
            parents.append(chain[-1] if chain else None)
            chain.append(index)
        return parents

    def _layout(self) -> None:
        """Works out visibility and reading order for the current set of folded rows.

        Keyed on the set itself rather than on calls to `collapse` and `expand`, so that
        changing `collapsed` directly still takes effect.
        """
        key = frozenset(self.collapsed)
        if key == self._layout_for:
            return
        self._layout_for = key
        visible: list[bool] = []
        previous: list[int | None] = []
        last_visible: int | None = None
        for index in range(len(self.rows)):
            parent = self._parents[index]
            # Parents come first in the list, so theirs is already known.
            shown = parent is None or (visible[parent] and not self.is_collapsed(parent))
            visible.append(shown)
            previous.append(last_visible)
            if shown:
                last_visible = index
        self._visible = visible
        self._previous = previous

    def parent_of(self, index: int) -> int | None:
        """The row this one sits under, if any."""
        return self._parents[index]

    def has_children(self, index: int) -> bool:
        """Whether anything sits under this row."""
        below = index + 1
        if below >= len(self.rows):
            return False
        return self.rows[below].get("depth", 0) > self.rows[index].get("depth", 0)

    def is_collapsed(self, index: int) -> bool:
        return self.rows[index].get("id", "") in self.collapsed

    def visible(self, index: int) -> bool:
        """Whether every ancestor is expanded."""
        self._layout()
        return self._visible[index]

    def collapse(self, index: int) -> str:
        """Folds a branch away, or steps out to the parent if there is nothing to fold."""
        if self.has_children(index) and not self.is_collapsed(index):
            self.collapsed.add(self.rows[index]["id"])
            return f"collapsed, {self.rows[index].get('title', '')}"
        return ""

    def expand(self, index: int) -> str:
        """Unfolds a branch."""
        if self.has_children(index) and self.is_collapsed(index):
            self.collapsed.discard(self.rows[index]["id"])
            return f"expanded, {self.rows[index].get('title', '')}"
        return ""

    def toggle(self, index: int) -> str:
        return self.expand(index) or self.collapse(index)

    def previous_visible(self, index: int) -> int | None:
        """The row that will be read before this one."""
        self._layout()
        return self._previous[index]

    def title_for(self, index: int) -> str:
        """The line for one row, level marker and all.

        The marker appears only where the level differs from the row above, so a flat list
        never mentions levels at all and a deep one says "level 3" once rather than on every
        line. Whether a branch is foldable is worth saying every time: it is the one thing
        the user cannot discover without pressing a key.
        """
        row = self.rows[index]
        line = describe(row)
        if self.has_children(index):
            line = f"{line}, {'collapsed' if self.is_collapsed(index) else 'expanded'}"

        depth = row.get("depth", 0)
        above = self.previous_visible(index)
        if depth and (above is None or self.rows[above].get("depth", 0) != depth):
            line = f"level {depth + 1}, {line}"
        return line

    def items(self, on_select=None) -> list[dialogs.DynamicMenuItem]:
        """Menu items for every row, foldable where a row has children, each carrying its row.

        `left` and `right` are already bound to left-arrow/Dot7 and right-arrow/Dot8 on this
        device, so folding needs no key handling of its own. `+` and `-` are offered
        as well, because that is what a tree view is expected to answer to. First-letter
        navigation goes by the title alone, so a level marker in front of it does not hide it.
        """
        items = []
        for index, row in enumerate(self.rows):
            item = dialogs.DynamicMenuItem(
                title=(lambda index=index: self.title_for(index)),
                action=(lambda index=index: on_select(self.rows[index])) if on_select else None,
                left=(lambda index=index: self.collapse(index)),
                right=(lambda index=index: self.expand(index)),
                dependency=(lambda index=index: self.visible(index)),
                hotkeys={
                    "+": (lambda index=index: self.expand(index)),
                    "-": (lambda index=index: self.collapse(index)),
                },
                hint=row.get("hint"),
                navigation_title=row.get("title", ""),
            )
            item.row = row
            items.append(item)
        return items
