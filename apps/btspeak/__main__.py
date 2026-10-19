#!/usr/bin/env python3
# blazie-flags: self-voice
"""Lumenna on BTSpeak and BTBraille.

Run it directly — `python3 __main__.py` — or add it to the user menu from BT Code, which is
what puts it on the device's own menu tree. It does not write a menu entry itself: that is
BT Code's job and it already does it.

The core runs in this process: `build-core.sh` builds it from the checkout, as a Python
extension beside this file, and the device is aarch64 Linux, which builds it for itself.
"""

import sys

from BTSpeak import dialogs, host

from core import Core, LumennaError
from menus import run
from session import HELP


def main() -> int:
    """Makes Lumenna the app in front, runs it, and puts back whatever was there before.

    `push_app_context` is how the device's own apps say they are in front. Without it the
    device went on believing the app that launched this one was — BT Code, say — and took its
    braille table from that app's open file, so every menu came out in computer braille
    rather than the reader's own table. It also turns self-voice on: the dialog library speaks
    and brailles for itself, and with self-voice off brltty would read the screen as well and
    everything would be said twice. The flags comment at the top of this file does the same
    for the menu launcher, which reads it before starting the program.

    The pop is in a `finally` because what it restores is device-wide: the app in front, its
    help, and self-voice, which left on would stop brltty reading the screen for whatever
    runs next.
    """
    host.push_app_context("lumenna", help_dir=HELP, self_voice=True)
    try:
        try:
            core = Core()
        except LumennaError as error:
            # Said in a dialog rather than printed: with self-voice on, nothing is reading
            # the screen, so a print here would be silence. This is also the failure most
            # likely to happen on a first run, before the core is built.
            dialogs.show_message(f"Lumenna could not start. {error.message}")
            return 1
        return run(core)
    finally:
        host.pop_app_context()


if __name__ == "__main__":
    sys.exit(main())
