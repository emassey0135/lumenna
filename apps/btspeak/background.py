#!/usr/bin/env python3
"""Lumenna's background service on BTSpeak: it keeps this device in sync and gives its
reminders while the app is closed, so nobody needs a terminal for either.

One process, as Blazie asks of a background service, doing what the daemon and a resident
desktop app do: it keeps the device in sync (`Device::keep_in_sync`), holding its endpoint, or
waiting its turn while another process holds it, and serving the command surface at the
profile's address while it holds it, so the app's Sync now runs its round here; it gives the
reminders as they come (`reminders.Giver`); and it takes the automatic backups when they are
due. While the app runs, the app waits its turn for the endpoint and for the right to give
reminders, which the core gives one process at a time.

It runs as a systemd user service (`system/lumenna-background.service`, which `service.py`
links in only when it is turned on from the app's Settings), and is turned off there, which
stops it: it lets go of the endpoint and of the right to give reminders, for another process
to take. The app's Reminders on this device, which the core honours, turns reminders off alone.
"""

from __future__ import annotations

import signal
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import reminders  # noqa: E402
from core import Core, LumennaError, device_name  # noqa: E402

#: What a client of the command surface is told is answering.
PROCESS = "Lumenna background service"

#: How often an automatic backup is looked for, in seconds, as the app does: whether one is
#: due is the core's.
BACKUP_EVERY = Core.BACKUP_EVERY


def log(message: str) -> None:
    """Said where the unit's journal finds it."""
    print(f"lumenna background: {message}", file=sys.stderr, flush=True)


def keep_in_sync(device) -> None:
    """Keeps the device in sync from here; a failure is logged and leaves the reminders."""
    try:
        holding = device.keep_in_sync(PROCESS, device_name(), Core.PLATFORM)
    except Exception as error:  # noqa: BLE001 - the core's refusal
        log(f"cannot keep this device in sync: {reminders.refused(error)}")
        return
    if not holding:
        log("another process keeps this device in sync; this service takes over when it stops")


def run(device, giver: reminders.Giver) -> None:
    """Gives reminders and takes the backups until stopped, keeping the device in sync."""
    keep_in_sync(device)
    backed_up = None
    try:
        while True:
            if backed_up is None or time.monotonic() - backed_up >= BACKUP_EVERY:
                try:
                    device.back_up_if_due()
                except Exception as error:  # noqa: BLE001 - tried again next hour
                    log(f"the automatic backup failed: {reminders.refused(error)}")
                backed_up = time.monotonic()
            try:
                giver.once()
            except LumennaError as error:
                log(error.message)
                giver.let_go()
            time.sleep(giver.pause())
    finally:
        giver.let_go()
        device.stop_syncing()


def main() -> int:
    # systemd stops the service with SIGTERM: leaving through `run`'s `finally` gives up the
    # profile's address and the endpoint at once, rather than when the process is gone.
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
    try:
        device = reminders.open_store()
    except LumennaError as error:
        log(error.message)
        return 1
    try:
        run(device, reminders.Giver(open_store=lambda: device))
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    sys.exit(main())
