"""The desktop's search over D-Bus, as GNOME Shell and KRunner ask it: what is found and how each
result reads are the core's, and choosing one opens it in the app."""

import unittest

from gi.repository import Gio, GLib

from harness import Session, pump

PROVIDER = ("/dev/emassey0135/Lumenna/SearchProvider", "org.gnome.Shell.SearchProvider2")
RUNNER = ("/dev/emassey0135/Lumenna/KRunner", "org.kde.krunner1")


class SearchTest(unittest.TestCase):
    def setUp(self):
        self.session = Session([("task", "add", "Buy milk tomorrow"), ("project", "add", "Milkround"),
                                ("task", "add", "Call the bank")])
        pump(1)
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION)
        self.name = self._app_name()

    def tearDown(self):
        self.session.close()

    def _app_name(self):
        """The app's name on the bus: its identifier, tagged by this session's own profile."""
        names = self.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                   "ListNames", None, None, Gio.DBusCallFlags.NONE, -1).unpack()[0]
        for name in names:
            if name.startswith("dev.emassey0135.Lumenna"):
                pid = self.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                         "GetConnectionUnixProcessID", GLib.Variant("(s)", (name,)), None,
                                         Gio.DBusCallFlags.NONE, -1).unpack()[0]
                if pid == self.session.process.pid:
                    return name
        raise AssertionError(f"the app owns no name on the bus: {names}")

    def call(self, where, method, arguments=None):
        path, interface = where
        reply = self.bus.call_sync(self.name, path, interface, method, arguments, None, Gio.DBusCallFlags.NONE, 10000)
        pump(0.2)
        return reply.unpack() if reply else None

    def test_gnome_shell_finds_tasks_and_projects_and_reads_each_as_the_core_words_it(self):
        (found,) = self.call(PROVIDER, "GetInitialResultSet", GLib.Variant("(as)", (["milk"],)))
        self.assertTrue(found[0].startswith("task:"), found)
        self.assertIn("project:Milkround", found)
        self.assertFalse(any("bank" in result for result in found))
        (metas,) = self.call(PROVIDER, "GetResultMetas", GLib.Variant("(as)", (found,)))
        names = [meta["name"] for meta in metas]
        self.assertEqual(names[0], "Buy milk")
        self.assertIn("tomorrow", metas[0]["description"])
        (narrowed,) = self.call(PROVIDER, "GetSubsearchResultSet", GLib.Variant("(asas)", (found, ["milkr"])))
        self.assertEqual(narrowed, ["project:Milkround"])

    def test_activating_a_result_opens_it_in_the_app(self):
        self.session.press("Control+1", wait=1)
        self.call(PROVIDER, "ActivateResult", GLib.Variant("(sasu)", ("project:Milkround", ["milk"], 0)))
        self.session.wait_for_window("Milkround – Lumenna")

    def test_krunner_finds_the_same_and_runs_a_match(self):
        (matches,) = self.call(RUNNER, "Match", GLib.Variant("(s)", ("milk",)))
        ids = [match[0] for match in matches]
        self.assertTrue(ids[0].startswith("task:"), ids)
        self.assertEqual(matches[0][1], "Buy milk")
        self.assertIn("tomorrow", matches[0][5]["subtext"])
        relevance = [match[4] for match in matches]
        self.assertEqual(relevance, sorted(relevance, reverse=True), "most fitting first")
        (actions,) = self.call(RUNNER, "Actions")
        self.assertEqual(actions, [])
        self.call(RUNNER, "Run", GLib.Variant("(ss)", ("project:Milkround", "")))
        self.session.wait_for_window("Milkround – Lumenna")


if __name__ == "__main__":
    unittest.main()
