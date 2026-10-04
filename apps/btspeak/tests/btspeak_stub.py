"""Stands in for the device's `BTSpeak` package when the tests run anywhere else.

The app imports `BTSpeak.dialogs` at module load, and that package exists only on a BTSpeak
or BTBraille device. The logic worth testing off the device — the tree, offsets, the client —
touches nothing in it beyond constructing menu items, so a stub that records its arguments is
enough. On the device the real package is found first and this does nothing.
"""

import sys
import types


class _Item:
    """Records what a `DynamicMenuItem` was built with."""

    def __init__(self, **fields):
        self.__dict__.update(fields)


def install() -> None:
    try:
        import BTSpeak  # noqa: F401
    except ImportError:
        dialogs = types.ModuleType("BTSpeak.dialogs")
        dialogs.DynamicMenuItem = _Item
        package = types.ModuleType("BTSpeak")
        package.dialogs = dialogs
        sys.modules["BTSpeak"] = package
        sys.modules["BTSpeak.dialogs"] = dialogs
