#!/usr/bin/env python3
"""Lumenna's reminders on BTSpeak: a small resident service, apart from the sync daemon.

It opens the store itself, through the core the app runs (`core.py`), holds the right to give
this device's reminders (`Device::deliver_reminders`), asks every twenty seconds what came due, and gives each: the device's
notification sound, then the words spoken (`host.say`) and on the braille display
(`brl.push_message`). Everything else is the core's — when each fires, what it says, that a
restart does not say the day again (it remembers when it last gave reminders), that what came
while it was stopped is one summary, and that nothing is given while another process on the
device gives them. There is no reminder logic here.

It runs as a systemd user service (`system/lumenna-reminders.service`, which `service.py` links
in only when it is turned on from the app's Settings), and is turned off there, which stops it
and lets go of the right so another process may take it, or by the app's Reminders on this
device, which the core honours.

Dismissing, snoozing and carrying forward what a block's end left short (the reminder's
`carry`, whose question ends its text) are the app's Reminders screen: a service has no keys
of its own.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from core import LumennaError, native, profile_directory, refused  # noqa: E402

#: How often to ask what came due, in seconds.
INTERVAL = 20

#: How often to ask while another process gives reminders, to take over soon after it stops.
WAITING = 5

def play(path) -> None:
    """Plays a sound file as the device's own services do, with ffplay, waiting for it."""
    import subprocess

    subprocess.run(["ffplay", "-nodisp", "-autoexit", str(path)], check=False,
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def chime() -> None:
    """The device's notification sound, before the words, as its calendar reminders play it:
    `notification.ogg` in its data directory."""
    try:
        from BTSpeak import host

        play(Path(host.getDataDirectory()) / "notification.ogg")
    except Exception:  # noqa: BLE001 - a sound that cannot play must not lose the words
        pass


def say(text: str) -> None:
    """Spoken, and pushed to the braille display when one is attached: the device's own
    `host.say_brl`, so braille follows the device's rules for whether there is a display."""
    from BTSpeak import host

    host.say_brl(text)


def braille(text: str) -> None:
    """Nothing more: `say` has shown it on the display already."""


def open_store():
    """This device's store, through the core."""
    try:
        return native().Device(str(profile_directory()))
    except Exception as error:  # noqa: BLE001 - the core's refusal
        raise LumennaError(refused(error)) from error


class Giver:
    """One process's giving of reminders: the store, holding the right, and what it says.

    `now`, when given, is a function of nothing giving the moment to deliver at (RFC 3339),
    for tests; the service leaves it out and the core takes now.
    """

    def __init__(self, open_store=open_store, sound=chime, speak=say, show=braille, now=None) -> None:
        self._open = open_store
        self._sound = sound
        self._speak = speak
        self._show = show
        self._now = now
        self.device = None
        #: Whether another process on the device gave reminders when last asked.
        self.elsewhere = False

    def let_go(self) -> None:
        """Lets go of the right to give reminders, for another process to take."""
        if self.device is not None:
            self.device.let_go_of_reminders()

    def once(self) -> list[str]:
        """Asks what came due and gives it; what was given, in order."""
        if self.device is None:
            self.device = self._open()
        try:
            found = self.device.deliver_reminders(self._now() if self._now else None)
        except Exception as error:  # noqa: BLE001 - the core's refusal
            raise LumennaError(refused(error)) from error
        # Said to no one: the other process gives them, and this one waits its turn.
        self.elsewhere = bool(found.elsewhere)
        given = []
        if found.missed_summary:
            given.append(found.missed_summary)
        given.extend(reminder.text for reminder in found.due)
        for text in given:
            self._sound()
            self._speak(text)
            self._show(text)
        return given

    def pause(self) -> float:
        """How long to wait before asking again: shorter while another process gives them."""
        return WAITING if self.elsewhere else INTERVAL

    def run(self) -> None:
        """Gives reminders until stopped."""
        try:
            while True:
                try:
                    self.once()
                except LumennaError as error:
                    # Logged where the unit's journal finds it; the next round tries again.
                    print(f"lumenna reminders: {error.message}", file=sys.stderr, flush=True)
                    self.let_go()
                time.sleep(self.pause())
        finally:
            self.let_go()


def main() -> int:
    try:
        Giver().run()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
