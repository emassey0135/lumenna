"""Installing the reminder service, only when asked from the app's Settings.

The device's own resident services live in `/BTSpeak`, as `btspeak-calendar-reminders` does:
the code in `Services/`, its systemd unit in `Systemd/`, and the toggle the device's settings
offer for it in `Settings/<name>.values`. This writes those three, then links and starts the
unit. Writing under `/BTSpeak` and enabling a system unit need an administrator, so on the
device each file is staged and put in place with `sudo install`; under a directory the person
can write to (a test's), it is written directly.

Nothing here runs unless the person chooses Install in Settings, Reminders, and answers yes:
a service on the device is theirs to agree to.
"""

from __future__ import annotations

import getpass
import os
import subprocess
import sys
import tempfile
from pathlib import Path


#: The service's name: its unit, its code and its setting are all called this.
NAME = "lumenna-reminders"

#: Where this checkout's app is, which the service's code runs.
APP = Path(__file__).resolve().parent


def paths(root: Path) -> dict:
    """Where each file goes under `root`, `/` on the device."""
    base = root / "BTSpeak"
    return {
        "code": base / "Services" / f"{NAME}.py",
        "unit": base / "Systemd" / f"{NAME}.service",
        "values": base / "Settings" / f"{NAME}.values",
    }


#: The unit and the toggle as they are kept in this checkout, for the device's /BTSpeak.
SYSTEM = APP / "system"


def code_text(app: Path = APP) -> str:
    """The service's code: this checkout's `reminders.py`, run from where the device keeps
    its services, so the app and the service are one version."""
    return (
        "#!/usr/bin/env python3\n"
        "# Written by Lumenna's BTSpeak app (Settings, Reminders); removing it there takes it away.\n"
        '"""Lumenna\'s reminders: the service in the Lumenna checkout, run from here."""\n'
        "import runpy\n"
        "import sys\n"
        f"sys.path.insert(0, {str(app)!r})\n"
        f"runpy.run_path({str(app / 'reminders.py')!r}, run_name='__main__')\n"
    )


def unit_text(user: str, profile: Path, python: str = "/usr/bin/python3", code: Path | None = None) -> str:
    """The systemd unit kept in `system/`, for this person and profile: a system unit
    running as the person, the device's own convention, the profile given outright."""
    text = (SYSTEM / f"{NAME}.service").read_text(encoding="utf-8")
    text = text.replace("@USER@", user).replace("@PROFILE@", str(profile)).replace("@PYTHON@", python)
    if code is not None:
        text = text.replace(str(paths(Path("/"))["code"]), str(code))
    return text


def values_text() -> str:
    """The toggle the device's settings offer, as kept in `system/`: on, the default, or off."""
    return (SYSTEM / f"{NAME}.values").read_text(encoding="utf-8")


def installed(root: Path = Path("/")) -> bool:
    """Whether the service's unit is in place."""
    return paths(root)["unit"].exists()


def _run(command: list[str]) -> None:
    subprocess.run(command, check=True, capture_output=True, text=True)


#: What runs a command. A test puts a recorder here, so nothing is ever run on the machine.
RUN = _run


def _place(path: Path, text: str, mode: int, run) -> None:
    """Writes `text` to `path`: directly where that is allowed, else through `sudo install`."""
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        path.chmod(mode)
        return
    except PermissionError:
        pass
    with tempfile.NamedTemporaryFile("w", delete=False, suffix=path.suffix, encoding="utf-8") as staged:
        staged.write(text)
    try:
        run(["sudo", "install", "-D", "-m", f"{mode:o}", staged.name, str(path)])
    finally:
        os.unlink(staged.name)


def install(profile: Path, root: Path = Path("/"), run=None, user: str | None = None) -> str:
    """Writes the service's code, unit and toggle under `root`, then links and starts the
    unit; what to say. `run` runs a command, `RUN` when absent."""
    run = run or RUN
    where = paths(root)
    _place(where["code"], code_text(), 0o755, run)
    python = sys.executable or "/usr/bin/python3"
    _place(where["unit"], unit_text(user or getpass.getuser(), profile, python, where["code"]), 0o644, run)
    _place(where["values"], values_text(), 0o644, run)
    unit = str(where["unit"])
    run(["sudo", "systemctl", "link", unit])
    run(["sudo", "systemctl", "daemon-reload"])
    run(["sudo", "systemctl", "enable", "--now", f"{NAME}.service"])
    return "The reminder service is installed and running. It starts whenever the device does"


def uninstall(root: Path = Path("/"), run=None) -> str:
    """Stops the service and takes away all three files; what to say."""
    run = run or RUN
    where = paths(root)
    run(["sudo", "systemctl", "disable", "--now", f"{NAME}.service"])
    for path in where.values():
        try:
            path.unlink()
        except FileNotFoundError:
            pass
        except PermissionError:
            run(["sudo", "rm", "-f", str(path)])
    run(["sudo", "systemctl", "daemon-reload"])
    return "The reminder service is stopped and removed"

