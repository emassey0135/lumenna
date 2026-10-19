"""The tests' own way into the store beside the app's: `lum rpc` over stdio, or the daemon's
socket when one answers. The tests seed and check the store through it, so what the app wrote
is read back by another process, as the device's daemon would read it."""

from __future__ import annotations

import os
import shutil
import socket
import subprocess
from pathlib import Path

from client import Client, LumennaError
from core import profile_directory


#: Where the daemon listens. One socket per profile, beside the store itself, so a second
#: profile is a second daemon rather than a collision.
SOCKET_NAME = "lumenna.sock"


def connect(profile: Path | None = None) -> Client:
    """Opens a conversation, however one can be had."""
    profile = profile or profile_directory()
    return _over_socket(profile) or _over_stdio(profile)


def _over_socket(profile: Path) -> Client | None:
    """The daemon, if it is running."""
    path = profile / SOCKET_NAME
    if not path.exists():
        return None
    try:
        connection = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        connection.connect(str(path))
    except OSError:
        # A socket file left behind by a daemon that died is not a reason to fail; spawning
        # our own server is exactly the right thing to do next.
        return None
    reader = connection.makefile("r", encoding="utf-8")
    writer = connection.makefile("w", encoding="utf-8")
    return Client(reader, writer, on_close=connection.close)


def find_lum() -> str | None:
    """The `lum` to run: the one on PATH, or else the one built in the checkout this app is
    running from — release first — so that a menu item starting the app from a git clone
    needs no PATH of its own."""
    found = shutil.which("lum")
    if found:
        return found
    checkout = Path(__file__).resolve().parents[3]
    for build in ("release", "debug"):
        candidate = checkout / "target" / build / "lum"
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


def _over_stdio(profile: Path) -> Client:
    """Our own `lum rpc`, which lives as long as this app does."""
    binary = find_lum()
    if binary is None:
        raise LumennaError(
            "cannot find `lum`: not on PATH, and not built in this checkout. Build it with "
            "cargo build, or install it on PATH"
        )
    environment = dict(os.environ, LUMENNA_PROFILE=str(profile))
    # The server's stderr goes to a file beside the store, for two reasons. It would
    # otherwise land on the terminal and scribble over whatever dialog is drawn — the app
    # owns that screen — and swallowing it would leave a server that refuses to start with
    # no way to say why.
    profile.mkdir(parents=True, exist_ok=True)
    log = open(profile / "rpc.log", "a", encoding="utf-8")  # noqa: SIM115
    server = subprocess.Popen(
        [binary, "rpc"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=log,
        text=True,
        encoding="utf-8",
        # Line buffering. Without it our writes sit in Python's buffer and the server waits
        # for a request that was never actually sent.
        bufsize=1,
        env=environment,
    )

    def stop() -> None:
        try:
            server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            server.kill()
        server.stdout.close()
        log.close()

    return Client(server.stdout, server.stdin, on_close=stop)
