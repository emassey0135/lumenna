"""A JSON-RPC client for `lum rpc`.

Stdlib only: the protocol is a request object, a reply object and a notification object, and
a library that supplied those would leave the framing and the demultiplexing — which is all
the work — to us anyway.

Two things shape this file:

**It takes streams, not a process.** §8 says the sync daemon will serve the same protocol
over a Unix socket, and that an RPC client's normal path is the socket with a spawned
`lum rpc` as the fallback. A client built around `Popen` would have to be rewritten for that;
one built around a reader and a writer needs a different constructor. See `connect.py`.

**Replies and notifications share one pipe.** A reader thread demultiplexes: replies go to
the call that is waiting for them, and `lumenna/changed` sets a flag. The thread must never
touch curses, so it sets an `Event` and the menus notice — see `menus.py`.
"""

from __future__ import annotations

import itertools
import json
import queue
import threading


#: How long to wait for a reply before giving up. Every operation is local and in-memory, so
#: this is a deadlock guard rather than a real deadline: if it ever fires, something is wrong
#: rather than slow, and a dialog that never returns is unrecoverable on a device with no way
#: to kill a process.
TIMEOUT = 20.0

#: The notification the server sends when another process wrote to the store.
CHANGED = "lumenna/changed"


class LumennaError(Exception):
    """A request the server refused."""

    def __init__(self, message: str, code: int = 0) -> None:
        super().__init__(message)
        self.message = message
        self.code = code


class Disconnected(LumennaError):
    """The server went away."""

    def __init__(self, message: str = "Lumenna stopped responding") -> None:
        super().__init__(message)


class Client:
    """One conversation with one server."""

    def __init__(self, reader, writer, on_close=None) -> None:
        self._reader = reader
        self._writer = writer
        self._on_close = on_close
        self._pending: dict[int, queue.Queue] = {}
        self._lock = threading.Lock()
        self._ids = itertools.count(1)
        self._alive = True

        #: Set when another process wrote to the store. Menus poll it; nothing else may.
        self.changed = threading.Event()

        self._thread = threading.Thread(target=self._read_loop, daemon=True)
        self._thread.start()

    # -- calling ------------------------------------------------------------------------

    def call(self, method: str, **params):
        """Sends a request and waits for its reply.

        Raises LumennaError if the server refused, Disconnected if it went away.
        """
        ident = next(self._ids)
        slot: queue.Queue = queue.Queue(maxsize=1)
        with self._lock:
            if not self._alive:
                raise Disconnected()
            self._pending[ident] = slot
        self._send({"jsonrpc": "2.0", "id": ident, "method": method, "params": params})

        try:
            reply = slot.get(timeout=TIMEOUT)
        except queue.Empty:
            with self._lock:
                self._pending.pop(ident, None)
            raise Disconnected(f"no reply to {method}") from None

        if reply is None:
            raise Disconnected()
        if "error" in reply:
            error = reply["error"]
            raise LumennaError(error.get("message", "unknown error"), error.get("code", 0))
        return reply.get("result", {})

    def notify(self, method: str, **params) -> None:
        """Sends a request that wants no reply."""
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def take_changed(self) -> bool:
        """Whether the store changed since this was last asked, clearing the flag."""
        if self.changed.is_set():
            self.changed.clear()
            return True
        return False

    def close(self) -> None:
        """Asks the server to stop, then lets go of it."""
        with self._lock:
            if not self._alive:
                return
            self._alive = False
        try:
            self._send({"jsonrpc": "2.0", "method": "shutdown"})
            self._writer.close()
        except OSError:
            pass
        if self._on_close is not None:
            self._on_close()

    # -- the wire -----------------------------------------------------------------------

    def _send(self, message: dict) -> None:
        """Writes one message, newline-framed.

        The server accepts newline-delimited JSON and `Content-Length` headers and answers in
        whichever arrived, so the simpler framing costs nothing here.
        """
        try:
            self._writer.write(json.dumps(message) + "\n")
            self._writer.flush()
        except (OSError, ValueError) as error:
            raise Disconnected(str(error)) from error

    def _read_loop(self) -> None:
        """Sorts replies from notifications until the pipe ends.

        Runs on its own thread and touches no UI: on this device the UI is curses, speech and
        a braille display, none of which are safe to drive from here.
        """
        try:
            for line in self._reader:
                line = line.strip()
                if not line:
                    continue
                try:
                    message = json.loads(line)
                except ValueError:
                    continue
                if "id" in message and message["id"] is not None:
                    with self._lock:
                        slot = self._pending.pop(message["id"], None)
                    if slot is not None:
                        slot.put(message)
                elif message.get("method") == CHANGED:
                    self.changed.set()
        except (OSError, ValueError):
            pass
        finally:
            self._fail_everything()

    def _fail_everything(self) -> None:
        """Wakes every waiting call, so a dead server is an error rather than a hang."""
        with self._lock:
            self._alive = False
            waiting = list(self._pending.values())
            self._pending.clear()
        for slot in waiting:
            try:
                slot.put_nowait(None)
            except queue.Full:
                pass
