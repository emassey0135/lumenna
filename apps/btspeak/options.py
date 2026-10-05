"""This app's own preferences: how it behaves on this device, not what the store holds, so
they live beside it rather than in it — in `$XDG_CONFIG_HOME/lumenna/btspeak.json`."""

from __future__ import annotations

import json
import os
from pathlib import Path

#: Read a task back, and ask, before adding it. Off by default: the announcement after
#: adding already says how it was read, and the extra step is for who wants it.
READ_BACK = "read-back-before-adding"


def path() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "lumenna" / "btspeak.json"


def load() -> dict:
    try:
        return json.loads(path().read_text())
    except (OSError, ValueError):
        return {}


def get(key: str, default=None):
    return load().get(key, default)


def put(key: str, value) -> None:
    values = load()
    values[key] = value
    path().parent.mkdir(parents=True, exist_ok=True)
    path().write_text(json.dumps(values, indent=2) + "\n")
