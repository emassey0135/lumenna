"""Turning the background service on and off, only when asked from the app's Settings: the one
process that keeps the device in sync and gives its reminders while the app is closed
(`background.py`).

The service is a systemd user unit, kept in `system/` and used where it is: turning it on links
it into the person's own units, as `systemctl --user enable` would, with the profile in a
drop-in beside it, then starts it; turning it off stops it and takes all three away. Everything
is in the person's home, so nothing needs an administrator, and nothing is written under
`/BTSpeak`, which the device replaces on every release. The links are the one record of whether
it is on.

Nothing here runs unless the person turns it on in Settings, Background reminders and sync: a
service on the device is theirs to agree to. The setting and its words are the app's
(`lumenna_records::background`); the session does what the app hands it (`Session.background`).
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path


#: The service's name: its unit is called this.
NAME = "lumenna-background"

#: The unit, in this app's own folder.
UNIT = Path(__file__).resolve().parent / "system" / f"{NAME}.service"


def config() -> Path:
    """The person's configuration directory, `$XDG_CONFIG_HOME` or `~/.config`, as systemd finds
    their units under it: a test's own, since the tests set `XDG_CONFIG_HOME`."""
    configured = os.environ.get("XDG_CONFIG_HOME", "")
    return Path(configured) if os.path.isabs(configured) else Path.home() / ".config"


def paths(config: Path) -> dict:
    """What turning it on writes under `config`: the unit's link, the link that starts it at
    login, and the drop-in naming the profile."""
    units = config / "systemd" / "user"
    return {
        "unit": units / f"{NAME}.service",
        "wanted": units / "default.target.wants" / f"{NAME}.service",
        "profile": units / f"{NAME}.service.d" / "profile.conf",
    }


def profile_text(profile: Path) -> str:
    """The drop-in giving the service the app's profile, so both open one store."""
    return f'[Service]\nEnvironment="LUMENNA_PROFILE={profile}"\n'


def enabled(config: Path) -> bool:
    """Whether the service is on: whether it starts at login."""
    return paths(config)["wanted"].is_symlink()


def _run(command: list[str]) -> None:
    subprocess.run(command, check=True, capture_output=True, text=True)


#: What runs a command. A test puts a recorder here, so nothing is ever run on the machine.
RUN = _run


def _link(path: Path, target: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_symlink() or path.exists():
        path.unlink()
    path.symlink_to(target)


def enable(profile: Path, config: Path, run=None) -> None:
    """Links the unit in, gives it the profile, and starts it. `run` runs a command, `RUN` when
    absent."""
    run = run or RUN
    where = paths(config)
    where["profile"].parent.mkdir(parents=True, exist_ok=True)
    where["profile"].write_text(profile_text(profile), encoding="utf-8")
    _link(where["unit"], UNIT)
    _link(where["wanted"], UNIT)
    run(["systemctl", "--user", "daemon-reload"])
    run(["systemctl", "--user", "start", f"{NAME}.service"])


def disable(config: Path, run=None) -> None:
    """Stops the service and takes its links and drop-in away."""
    run = run or RUN
    where = paths(config)
    run(["systemctl", "--user", "stop", f"{NAME}.service"])
    for path in where.values():
        if path.is_symlink() or path.exists():
            path.unlink()
    try:
        where["profile"].parent.rmdir()
    except OSError:
        pass
    run(["systemctl", "--user", "daemon-reload"])
