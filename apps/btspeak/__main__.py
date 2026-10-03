#!/usr/bin/env python3
# blazie-flags: self-voice
"""Lumenna on BTSpeak and BTBraille.

Run it directly — `python3 __main__.py` — or add it to the user menu from BT Code, which is
what puts it on the device's own menu tree. It does not write a menu entry itself: that is
BT Code's job and it already does it.

The device is aarch64 Linux, so the ordinary Linux build of `lum` runs here unmodified; this
needs it on `PATH` and nothing else. No third-party Python: the whole client is one file, and
a dependency to build a JSON-RPC envelope would be more surface than it saves.
"""

import sys

from BTSpeak import dialogs, host

from client import LumennaError
from connect import connect
from menus import run


def main() -> int:
    """Turns self-voice on, runs the app, and puts self-voice back.

    The flags comment at the top of this file is the usual mechanism: the launcher reads it
    before starting the program and restores the setting afterwards, so a program launched
    from a menu does not have to manage this. But it *is* only read at launch, and this app
    is also meant to be run straight from a shell and from BT Code's project runner, where
    nothing has read it. Setting it here costs nothing when the launcher already did.

    It matters because the dialog library speaks and brailles for itself: with self-voice
    off, brltty reads the screen as well and everything is said twice.

    The flag lives in `/run/BTSpeak/`, so it is device-wide rather than ours — which is why
    restoring it belongs in a `finally` and not at the end of a happy path. Leaving it on
    would stop brltty reading the screen for whatever runs next.
    """
    was_self_voicing = host.get_self_voice()
    host.set_self_voice(True)
    client = None
    try:
        try:
            client = connect()
        except LumennaError as error:
            # Said in a dialog rather than printed: with self-voice on, nothing is reading
            # the screen, so a print here would be silence. This is also the failure most
            # likely to happen on a first run, when `lum` is not on PATH yet.
            dialogs.show_message(f"Lumenna could not start. {error.message}")
            return 1
        return run(client)
    finally:
        if client is not None:
            client.close()
        host.set_self_voice(was_self_voicing)


if __name__ == "__main__":
    sys.exit(main())
