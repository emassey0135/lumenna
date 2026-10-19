"""Lumenna's core, in this process: the store and the Crux app beside it, through BoltFFI.

The core is a CPython extension (`lumenna_app`) that `build-core.sh` builds from
`crates/mobile` and unpacks into `native/` beside this file, the same core Android runs. The
app holds everything a screen used to decide — which row is in hand, what is folded, which
question is open, what a form starts from and what saving sends, the proposals, pairing — and
words every line in this device's clock. A screen here draws what it shows and sends what the
person did.

Each change the app makes arrives as a turn, on a thread of the app's own: the view, its lines
and what to say. They queue here, and the menus, which block in the device's dialogs, take
them in when they wake (`Session`).
"""

from __future__ import annotations

import os
import queue
import sys
import threading
import time
from pathlib import Path

NATIVE = Path(__file__).resolve().parent / "native"
sys.path.insert(0, str(NATIVE))

try:
    import lumenna_app as core  # noqa: E402
except ImportError as error:  # pragma: no cover - a checkout not yet built
    core = None
    _missing = str(error)
else:
    _missing = ""


class LumennaError(Exception):
    """Something the core refused, or a core that is not there: its sentence, to be said."""

    def __init__(self, message: str) -> None:
        super().__init__(message)
        self.message = message


def refused(error: Exception) -> str:
    """The core's sentence for what it refused: a `LumennaError`'s `reason`, written to be said."""
    return getattr(error, "reason", None) or getattr(error, "message", None) or str(error)


def profile_directory() -> Path:
    """Where the store lives, by the same rule `lum` uses.

    `LUMENNA_PROFILE` first, then the platform data directory, which on Linux is
    `$XDG_DATA_HOME/lumenna`, or `~/.local/share/lumenna` when that is unset; as the
    `directories` crate does, a relative `XDG_DATA_HOME` is ignored. It has to match `lum`
    exactly: the daemon syncing this device opens the same store.
    """
    explicit = os.environ.get("LUMENNA_PROFILE")
    if explicit:
        return Path(explicit)
    data_home = os.environ.get("XDG_DATA_HOME", "")
    base = Path(data_home) if os.path.isabs(data_home) else Path.home() / ".local" / "share"
    return base / "lumenna"


def native():
    """The core's module, or why it cannot be loaded."""
    if core is None:
        raise LumennaError(
            f"Lumenna's core is not built for this Python: run apps/btspeak/build-core.sh. ({_missing})"
        )
    return core


class _Shell:
    """Where the app's turns arrive, on its own thread: queued for the menus."""

    def __init__(self, turns: queue.Queue, arrived: threading.Event) -> None:
        self._turns = turns
        self._arrived = arrived

    def turn(self, turn) -> None:
        self._turns.put(turn)
        self._arrived.set()


class _Locale:
    """This device's clock and dates, for the lines the core words: the Time Format setting,
    and the day as a person says it. "Today" and when to give the year are the core's."""

    def time(self, hour: int, minute: int) -> str:
        import rows  # here: rows reads the device's settings

        return rows.clock(f"{hour:02d}:{minute:02d}")

    def date(self, year: int, month: int, day: int, with_year: bool) -> str:
        import datetime

        when = datetime.date(year, month, day)
        said = f"{when:%A} {when.day} {when:%B}"
        return f"{said} {year}" if with_year else said


class Core:
    """One open store and the app over it."""

    #: What this device says it runs, in pairing.
    PLATFORM = "btspeak"

    def __init__(self, profile: Path | None = None) -> None:
        module = native()
        self.profile = Path(profile or profile_directory())
        self.profile.mkdir(parents=True, exist_ok=True)
        try:
            self.device = module.Device(str(self.profile))
        except Exception as error:  # noqa: BLE001 - the core's own refusal
            raise LumennaError(f"The store could not be opened. {refused(error)}") from error
        self.turns: queue.Queue = queue.Queue()
        self.arrived = threading.Event()
        self._locale = _Locale()
        self._shell = _Shell(self.turns, self.arrived)
        self.app = module.App(self.device, self.PLATFORM, self._locale, self._shell)

    #: How often an automatic backup is looked for, in seconds: whether one is due is the
    #: core's, by the device's `backup-every`.
    BACKUP_EVERY = 3600

    def back_up_while_running(self) -> None:
        """Takes the automatic backups, as `lum` does before a command: one at start if one is
        due, then looked for every hour, off the menus' thread since a Pi takes a while."""

        def run() -> None:
            while True:
                try:
                    self.device.back_up_if_due()
                except Exception:  # noqa: BLE001 - a backup that fails is tried again next hour
                    pass
                time.sleep(self.BACKUP_EVERY)

        threading.Thread(target=run, name="lumenna-backups", daemon=True).start()

    def still(self, timeout: float = 30.0) -> None:
        """Waits until the app has done everything sent it and handed every turn on. What
        reaches other devices — a sync, a pairing — is not waited for."""
        deadline = time.monotonic() + timeout
        while not self.app.is_still():
            if time.monotonic() > deadline:
                raise LumennaError("Lumenna stopped responding")
            time.sleep(0.002)
