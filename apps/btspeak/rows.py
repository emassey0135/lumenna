"""Turning rows into menu items.

The server sends components — `role`, `depth`, `index`, `count`, `state`, `title`, `value` —
and never a sentence, because speech and braille compose them differently (§13). This file is
where the composing happens for this device.

**One caveat shapes everything here.** `DynamicMenuDialog.draw()` sets `content_text` and
`content_braille` from the same string, so with the stock library speech and braille cannot
diverge. §16.11 offers two ways round it: put a compact level marker in the title, or
subclass the dialog and override the selection branch of `draw()`. It says to start with the
marker, and this does. If the marker turns out to grate in speech, the subclass is about
twenty lines and `InteractiveSearchDialog` extends `ChoiceDialog`, so it is an intended
pattern rather than a hack.

**Depth is never indentation.** §16.11 is explicit: indentation does not work in speech. The
level is announced *when it changes*, which is what a screen-reader tree view does, and
costs nothing here because titles may be callables — so the marker is computed against the
row above at the moment it is drawn, and stays right as branches collapse.
"""

from __future__ import annotations

from BTSpeak import dialogs


def describe(row: dict, with_role: bool = False) -> str:
    """One row, as a line to speak and to braille.

    The title is never abbreviated — braille users get everything speech users get (§13).
    Only the surroundings are compressed.
    """
    parts = [row.get("title", "")]
    if with_role:
        parts.append(row.get("role", ""))
    if row.get("checked") is True:
        parts.append("done")
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
    """

    def __init__(self, rows: list[dict]) -> None:
        self.rows = rows
        self.collapsed: set[str] = set()

    def parent_of(self, index: int) -> int | None:
        """The row this one sits under, if any."""
        depth = self.rows[index].get("depth", 0)
        for above in range(index - 1, -1, -1):
            if self.rows[above].get("depth", 0) < depth:
                return above
        return None

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
        parent = self.parent_of(index)
        while parent is not None:
            if self.is_collapsed(parent):
                return False
            parent = self.parent_of(parent)
        return True

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
        for above in range(index - 1, -1, -1):
            if self.visible(above):
                return above
        return None

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

    def items(self, on_select) -> list[dialogs.DynamicMenuItem]:
        """Menu items for every row, foldable where a row has children.

        `left` and `right` are already bound to left-arrow/Dot7 and right-arrow/Dot8 on this
        device, so folding needs no key handling of its own (§16.11). `+` and `-` are offered
        as well, because that is what a tree view is expected to answer to.
        """
        items = []
        for index, row in enumerate(self.rows):
            items.append(
                dialogs.DynamicMenuItem(
                    title=(lambda index=index: self.title_for(index)),
                    action=(lambda index=index: on_select(self.rows[index])),
                    left=(lambda index=index: self.collapse(index)),
                    right=(lambda index=index: self.expand(index)),
                    dependency=(lambda index=index: self.visible(index)),
                    hotkeys={
                        "+": (lambda index=index: self.expand(index)),
                        "-": (lambda index=index: self.collapse(index)),
                    },
                    hint=row.get("hint"),
                )
            )
        return items
