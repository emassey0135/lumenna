"""What every test of the menus shares: a fresh store, the app over it in this process, and
`lum rpc` beside it to seed the store and read back what the app wrote, as another process
on the device would."""

from __future__ import annotations

import os
import shutil
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

# Every test runs in UTC: the core reads the zone once, so one test cannot change it for itself,
# and fixed moments are written in UTC.
os.environ["TZ"] = "UTC"
time.tzset()

from connect import connect, find_lum  # noqa: E402
from core import Core  # noqa: E402
from session import Session  # noqa: E402

LUM = find_lum()


class Store:
    """A store of its own in a scratch directory, the app over it and `lum rpc` beside it."""

    #: The environment a test changes, put back afterwards.
    KEYS = ("PATH", "LUMENNA_PROFILE", "LUMENNA_BACKUP_DIR", "XDG_CONFIG_HOME")

    def __init__(self) -> None:
        self.profile = Path(tempfile.mkdtemp())
        self.saved = {key: os.environ.get(key) for key in self.KEYS}
        os.environ["PATH"] = f"{Path(LUM).parent}{os.pathsep}{os.environ['PATH']}"
        os.environ["LUMENNA_PROFILE"] = str(self.profile)
        os.environ["LUMENNA_BACKUP_DIR"] = str(self.profile / "backups")
        # This app's own preferences, kept out of the real ones.
        os.environ["XDG_CONFIG_HOME"] = str(self.profile / "config")
        self.client = connect(self.profile)
        self.core = Core(self.profile)
        self.session = Session(self.core)
        self.session.ready()

    def caught_up(self) -> None:
        """The app takes in what `lum rpc` wrote, as it would on its next tick."""
        self.session.tick()

    def close(self) -> None:
        self.client.close()
        for key, value in self.saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        shutil.rmtree(self.profile, ignore_errors=True)
