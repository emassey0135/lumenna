"""Turning the app's rows into menu items.

The app sends each row's line already worded in this device's clock (`Lines`), the level where
it changes against the row shown before, and whether what is under a row is folded away; it
folds, too, through each row's Expand and Collapse. This file only joins those for the menu.

Speech and braille come from one string, but for a row whose line the app shows apart in
braille (`Line.braille`: its states as short forms, "ovd rec", as the device's own mail list
shows its statuses), which goes to the menu item's `braille_title`. The level is a compact
marker in the line, said only where it changes, which is what a screen-reader tree view does:
depth is never indentation, which says nothing in speech.
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


def twelve_hour() -> bool:
    """Whether the device says times in the 12-hour clock, for what the core words itself."""
    return "%p" in _time_format()


def clock(hhmm: str) -> str:
    """A time of day, `HH:MM` as the server sends it, as this device says times."""
    try:
        hour, minute = (int(part) for part in hhmm.split(":"))
    except (AttributeError, ValueError):
        return hhmm
    return time.strftime(_time_format(), (2000, 1, 1, hour, minute, 0, 5, 1, -1)).strip()


class Row:
    """One row a menu shows: its key in the app, its line, what can be done to it, and the
    record it stands for (a task's `RowView`, a day's `DayItem`, a sidebar entry, a device)."""

    def __init__(self, key: str, line: str, actions=(), record=None, *, level=None, expanded=None,
                 title: str = "", hint=None, braille: str = "") -> None:
        self.key = key
        self.line = line
        #: The line on the braille display, where it is not the line said.
        self.braille = braille or line
        self.actions = list(actions)
        self.record = record
        self.level = level
        self.expanded = expanded
        self.title = title or line
        self.hint = hint

    def said(self) -> str:
        """The line as read: the level where it changes, the line, and whether what is under
        it is folded away — the one thing that cannot be found out without pressing a key."""
        return self._with(self.line)

    def brailled(self) -> str:
        """The line on the braille display: as said, the states short."""
        return self._with(self.braille)

    def _with(self, line: str) -> str:
        parts = [self.level or "", line]
        if self.expanded is not None:
            parts.append("expanded" if self.expanded else "collapsed")
        return ", ".join(part for part in parts if part)


def items(rows, on_select=None, fold=None) -> list:
    """Menu items for `rows`, each carrying its row.

    `fold(row, open)` runs the row's own Expand or Collapse, the app's: left and right are
    already bound to left-arrow/Dot7 and right-arrow/Dot8 on this device, so folding needs no
    key handling of its own, and `+` and `-` are offered as well, because that is what a tree
    view is expected to answer to. First-letter navigation goes by the title alone, so a level
    marker in front of it does not hide it.
    """
    made = []
    for row in rows:
        foldable = fold is not None and row.expanded is not None
        item = dialogs.DynamicMenuItem(
            title=row.said(),
            action=(lambda row=row: on_select(row)) if on_select else None,
            left=(lambda row=row: fold(row, False)) if foldable else None,
            right=(lambda row=row: fold(row, True)) if foldable else None,
            hotkeys={"+": (lambda row=row: fold(row, True)), "-": (lambda row=row: fold(row, False))} if foldable else {},
            hint=row.hint,
            navigation_title=row.title,
            braille_title=row.brailled() if row.braille != row.line else None,
        )
        item.row = row
        made.append(item)
    return made
