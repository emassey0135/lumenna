"""The command surface the app serves while it holds the device's endpoint."""

import json
import os
import socket
import stat
import subprocess
import unittest
from pathlib import Path

from harness import LUM, Session, pump


class CommandsTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([("task", "add", "Buy milk")])
        self.socket = Path(self.session.environment["LUMENNA_PROFILE"]) / "lumenna.sock"
        for _ in range(50):
            if self.socket.exists():
                break
            pump(0.2)

    def tearDown(self):
        self.session.close()

    def ask(self, *requests):
        """Sends JSON-RPC requests over the socket, one per line, and returns their replies."""
        with socket.socket(socket.AF_UNIX) as connection:
            connection.connect(str(self.socket))
            stream = connection.makefile("rw")
            replies = []
            for number, (method, params) in enumerate(requests, 1):
                stream.write(json.dumps({"jsonrpc": "2.0", "id": number, "method": method, "params": params}) + "\n")
                stream.flush()
                while (reply := json.loads(stream.readline())).get("id") != number:
                    pass
                replies.append(reply["result"])
            return replies

    def test_the_socket_is_the_persons_alone(self):
        self.assertEqual(stat.S_IMODE(os.stat(self.socket).st_mode), 0o600)

    def test_a_client_reaches_the_app_and_its_writes_appear_there(self):
        self.session.press("Control+2", wait=1)
        info, _ = self.ask(("initialize", {}), ("task.add", {"text": "Call the bank"}))
        self.assertEqual(info["process"], "Lumenna for Linux")
        pump(2)
        self.assertIn("Call the bank", [row.get_name() for row in self.session.find_all("tree item")])

    def test_a_daemon_started_meanwhile_waits_and_takes_over_when_the_app_quits(self):
        log = Path(self.session.directory) / "daemon.log"
        daemon = subprocess.Popen([str(LUM), "sync-daemon"], env=self.session.environment,
                                  stdout=subprocess.DEVNULL, stderr=open(log, "w"))
        try:
            pump(4)
            self.assertIsNone(daemon.poll(), "it waits rather than refusing")
            self.assertIn("takes over when it stops", log.read_text())
            self.session.process.terminate()
            self.session.process.wait()
            pump(9)
            (info,) = self.ask(("initialize", {}))
            self.assertEqual(info["process"], "daemon")
        finally:
            daemon.terminate()
            daemon.wait()


if __name__ == "__main__":
    unittest.main()
