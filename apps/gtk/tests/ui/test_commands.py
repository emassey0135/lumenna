"""The command surface, unpaired: the app keeps nothing in sync until another device is paired,
so it neither holds the device's endpoint nor serves the socket, and `lum rpc` answers itself."""

import json
import subprocess
import unittest
from pathlib import Path

from harness import LUM, Session, pump


class CommandsTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([("task", "add", "Buy milk")])
        self.socket = Path(self.session.environment["LUMENNA_PROFILE"]) / "lumenna.sock"
        pump(2)

    def tearDown(self):
        self.session.close()

    def test_an_unpaired_app_serves_no_socket_and_lum_rpc_answers_itself(self):
        self.assertFalse(self.socket.exists(), "an unpaired app serves no socket")
        self.session.press("Control+2", wait=1)
        requests = [
            {"jsonrpc": "2.0", "id": 1, "method": "initialize"},
            {"jsonrpc": "2.0", "id": 2, "method": "task.add", "params": {"text": "Call the bank"}},
        ]
        result = subprocess.run([str(LUM), "rpc"], env=self.session.environment, capture_output=True, text=True,
                                input="".join(json.dumps(request) + "\n" for request in requests), timeout=30)
        replies = {reply["id"]: reply["result"] for reply in map(json.loads, result.stdout.splitlines()) if "id" in reply}
        self.assertEqual(replies[1]["process"], "lum rpc")
        # Another process's write, which the app takes in within a second or so.
        pump(2)
        self.assertIn("Call the bank", [row.get_name() for row in self.session.find_all("tree item")])

    def test_an_unpaired_sync_says_it_is_not_paired(self):
        said = subprocess.run([str(LUM), "sync"], env=self.session.environment, capture_output=True, text=True, timeout=30)
        self.assertIn("not paired", said.stdout + said.stderr)
        self.assertFalse(self.socket.exists(), "the app holds no endpoint")


if __name__ == "__main__":
    unittest.main()
