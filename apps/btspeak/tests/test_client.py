"""The client, against a real `lum rpc`.

§8 makes this transport the path for every client that cannot link Rust, so the parts worth
proving are the ones a mock would hide: that a reply finds the call waiting for it, that a
write from another process arrives unasked, and that a refusal is an exception rather than a
silent wrong answer.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import btspeak_stub  # noqa: E402

btspeak_stub.install()

from client import Disconnected, LumennaError  # noqa: E402
from connect import connect  # noqa: E402


def find_lum() -> str | None:
    """The binary, from the build tree or from PATH."""
    root = Path(__file__).resolve().parents[3]
    for build in ("debug", "release"):
        candidate = root / "target" / build / "lum"
        if candidate.exists():
            return str(candidate)
    return shutil.which("lum")


LUM = find_lum()


@unittest.skipIf(LUM is None, "`lum` has not been built")
class TalkingToTheServer(unittest.TestCase):
    def setUp(self):
        self.profile = Path(tempfile.mkdtemp())
        os.environ["PATH"] = f"{Path(LUM).parent}{os.pathsep}{os.environ['PATH']}"
        # The server and the CLI both take automatic backups; keep them in the scratch area.
        os.environ["LUMENNA_BACKUP_DIR"] = str(self.profile / "backups")
        self.client = connect(self.profile)

    def tearDown(self):
        self.client.close()
        shutil.rmtree(self.profile, ignore_errors=True)

    def cli(self, *args):
        """A second process, writing to the same store."""
        done = subprocess.run(
            [LUM, *args],
env=dict(os.environ, LUMENNA_PROFILE=str(self.profile)),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_the_server_names_a_contract_this_app_can_read(self):
        from menus import CONTRACT

        self.assertEqual(self.client.call("initialize")["contract"], CONTRACT)

    def test_a_created_task_comes_back_whole(self):
        # §12's surface is `add_task(text) -> Task`: capture is the one place a round trip
        # hurts.
        self.client.call("project.add", name="Work")
        result = self.client.call("task.add", text="write the chapter tomorrow p1 #Work")
        self.assertEqual(result["task"]["title"], "write the chapter")
        self.assertEqual(result["task"]["project"], "Work")
        self.assertEqual(result["task"]["priority"], 1)

    def test_replies_find_the_calls_that_are_waiting_for_them(self):
        first = self.client.call("task.add", text="one")["task"]["id"]
        second = self.client.call("task.add", text="two")["task"]["id"]
        self.assertNotEqual(first, second)
        self.assertEqual(self.client.call("task.show", id=first)["title"], "one")
        self.assertEqual(self.client.call("task.show", id=second)["title"], "two")

    def test_a_write_from_another_process_arrives_without_being_asked_for(self):
        self.client.call("task.list")
        self.cli("task", "add", "from elsewhere", "--quiet")
        for _ in range(40):
            if self.client.changed.is_set():
                break
            time.sleep(0.1)
        self.assertTrue(self.client.take_changed(), "no push arrived")
        self.assertFalse(self.client.take_changed(), "and it is only reported once")

    def test_our_own_writes_do_not_announce_themselves(self):
        # `data_version` does not move for the connection that wrote, which is why a menu
        # tracks its own edits rather than waiting to be told about them.
        self.client.call("task.add", text="ours")
        time.sleep(2)
        self.assertFalse(self.client.changed.is_set())

    def test_a_refusal_is_an_exception_and_not_a_wrong_answer(self):
        with self.assertRaises(LumennaError) as refused:
            self.client.call("task.show", id="nothing-like-this")
        self.assertIn("nothing here matches", refused.exception.message)

    def test_row_numbers_are_refused_because_they_belong_to_the_terminal(self):
        self.cli("task", "add", "review PR", "--quiet")
        self.cli("task", "list")
        with self.assertRaises(LumennaError) as refused:
            self.client.call("task.done", id="1")
        self.assertIn("identifier", refused.exception.message)

    def test_completion_is_reachable_and_is_why_this_speaks_a_protocol(self):
        self.client.call("label.add", name="deep")
        found = self.client.call("complete", text="review @de", cursor=10, syntax="quick-add")
        self.assertEqual([c["text"] for c in found["candidates"]], ["@deep"])
        self.assertEqual(found["candidates"][0]["kind"], "label")

    def test_a_preview_writes_nothing(self):
        preview = self.client.call("preview", text="something #Nope")
        self.assertTrue(preview["has_errors"])
        self.assertEqual(self.client.call("task.list")["count"], 0)

    def test_a_dead_server_wakes_every_waiting_call(self):
        self.client.close()
        with self.assertRaises(Disconnected):
            self.client.call("task.list")


if __name__ == "__main__":
    unittest.main()


@unittest.skipIf(LUM is None, "`lum` has not been built")
class TalkingToTheDaemon(unittest.TestCase):
    """§8: the socket is the expected path on this device, and spawning is the fallback."""

    def setUp(self):
        self.profile = Path(tempfile.mkdtemp())
        os.environ["LUMENNA_BACKUP_DIR"] = str(self.profile / "backups")
        self.daemon = subprocess.Popen(
            [LUM, "sync-daemon", "--local-only"],
            env=dict(os.environ, LUMENNA_PROFILE=str(self.profile)),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        socket = self.profile / "lumenna.sock"
        for _ in range(100):
            if socket.exists():
                break
            time.sleep(0.1)
        self.assertTrue(socket.exists(), "the daemon made its socket")

    def tearDown(self):
        self.daemon.terminate()
        self.daemon.wait(timeout=10)
        shutil.rmtree(self.profile, ignore_errors=True)

    def test_the_app_reaches_the_store_through_the_daemon(self):
        client = connect(self.profile)
        try:
            # A spawned server is stopped by a function of connect's own; the socket is just
            # closed.
            self.assertEqual(client._on_close.__name__, "close", "it used the socket")
            result = client.call("task.add", text="said to the daemon")
            self.assertEqual(result["task"]["title"], "said to the daemon")
            listed = client.call("task.list")
            self.assertEqual(listed["count"], 1)
        finally:
            client.close()
        # It was the socket, not a spawned `lum rpc`: closing the client left the daemon up.
        self.assertIsNone(self.daemon.poll())


@unittest.skipIf(LUM is None, "`lum` has not been built")
class FindingAServer(unittest.TestCase):
    """§8's rule for this kind of client: the daemon's socket when it answers, else a
    `lum rpc` of its own — and a socket left behind by a daemon that died is not an answer."""

    def setUp(self):
        self.profile = Path(tempfile.mkdtemp())
        os.environ["PATH"] = f"{Path(LUM).parent}{os.pathsep}{os.environ['PATH']}"
        os.environ["LUMENNA_BACKUP_DIR"] = str(self.profile / "backups")

    def tearDown(self):
        shutil.rmtree(self.profile, ignore_errors=True)

    def test_a_running_daemon_is_used_over_its_socket(self):
        daemon = subprocess.Popen(
            [LUM, "--profile", str(self.profile), "sync-daemon", "--local-only"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            socket_path = self.profile / "lumenna.sock"
            deadline = time.monotonic() + 20
            while not socket_path.exists() and time.monotonic() < deadline:
                time.sleep(0.1)
            self.assertTrue(socket_path.exists(), "the daemon made no socket")
            client = connect(self.profile)
            try:
                self.assertIn("socket", repr(client._on_close), "connected some other way")
                client.call("task.add", text="through the daemon")
                titles = [row["title"] for row in client.call("task.list")["rows"]]
                self.assertEqual(titles, ["through the daemon"])
            finally:
                client.close()
        finally:
            daemon.terminate()
            daemon.wait(timeout=10)

    def test_a_socket_left_by_a_dead_daemon_falls_back_to_lum_rpc(self):
        import socket as sockets

        left = sockets.socket(sockets.AF_UNIX, sockets.SOCK_STREAM)
        left.bind(str(self.profile / "lumenna.sock"))
        left.close()  # bound and closed: a file nobody listens on, as a crash leaves it
        client = connect(self.profile)
        try:
            self.assertNotIn("socket", repr(client._on_close))
            self.assertEqual(client.call("initialize")["name"], "lumenna")
        finally:
            client.close()
