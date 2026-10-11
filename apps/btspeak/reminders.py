#!/usr/bin/env python3
"""Lumenna's reminders on BTSpeak: a small resident service, apart from the sync daemon.

It holds the right to give this device's reminders (`reminder.deliver`, over `lum rpc` or the
daemon's socket), asks every twenty seconds what came due, and gives each: the device's
notification sound, then the words spoken (`host.say`) and on the braille display
(`brl.push_message`). Everything else is the core's — when each fires, what it says, that a
restart does not say the day again (it remembers when it last gave reminders), that what came
while it was stopped is one summary, and that nothing is given while another process on the
device gives them. There is no reminder logic here.

It runs as a system service (`service.py` writes it, only when asked from the app's Settings),
and is turned off either by the device's own setting, `lumenna-reminders` (off), which lets go
of the right so another process may take it, or by the app's Reminders on this device, which
the core honours.

Dismissing and snoozing are the app's Reminders screen: a service has no keys of its own.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from client import Disconnected, LumennaError  # noqa: E402
from connect import connect  # noqa: E402

#: How often to ask what came due, in seconds.
INTERVAL = 20

#: The device setting that turns the service off, with its `.values` file in Settings.
SETTING = "lumenna-reminders"


def enabled() -> bool:
    """Whether the device's own setting leaves the service on. Absent, it is on."""
    try:
        from BTSpeak import settings

        return str(settings.getValue(SETTING) or "on").strip().lower() not in {"off", "false", "no"}
    except Exception:  # noqa: BLE001 - off the device, or before the setting exists
        return True


def chime() -> None:
    """The device's notification sound, before the words, as its own reminders have it."""
    try:
        from BTSpeak import host

        play = getattr(host, "play_sound", None)
        if play is not None:
            play("notification")
    except Exception:  # noqa: BLE001 - a sound that cannot play must not lose the words
        pass


def say(text: str) -> None:
    """Spoken by the device's own speech."""
    from BTSpeak import host

    host.say(text)


def braille(text: str) -> None:
    """Shown on the braille display as a message, as the device shows its own."""
    from BTSpeak import brl

    brl.push_message(text)


class Giver:
    """One process's giving of reminders: a connection holding the right, and what it says.

    `now`, when given, is a function of nothing giving the moment to deliver at (RFC 3339),
    for tests; the service leaves it out and the core takes now.
    """

    def __init__(self, open_client=connect, on=enabled, sound=chime, speak=say, show=braille, now=None) -> None:
        self._open = open_client
        self._on = on
        self._sound = sound
        self._speak = speak
        self._show = show
        self._now = now
        self.client = None

    def let_go(self) -> None:
        """Closes the connection, which lets go of the right to give reminders."""
        if self.client is not None:
            self.client.close()
            self.client = None

    def once(self) -> list[str]:
        """Asks what came due and gives it; what was given, in order."""
        if not self._on():
            self.let_go()
            return []
        if self.client is None:
            self.client = self._open()
        params = {"now": self._now()} if self._now else {}
        try:
            found = self.client.call("reminder.deliver", **params)
        except Disconnected:
            self.client = None
            return []
        given = []
        if found.get("missed_summary"):
            given.append(found["missed_summary"])
        given.extend(reminder["text"] for reminder in found.get("due", []))
        for text in given:
            self._sound()
            self._speak(text)
            self._show(text)
        return given

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
                time.sleep(INTERVAL)
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
