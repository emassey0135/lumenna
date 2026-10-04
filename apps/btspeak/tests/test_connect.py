"""Finding the store: the app has to land on the same directory `lum` does."""

import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from connect import profile_directory  # noqa: E402


class FindingTheProfile(unittest.TestCase):
    def directory(self, **environment):
        with mock.patch.dict(os.environ, environment, clear=False):
            for key in ("LUMENNA_PROFILE", "XDG_DATA_HOME"):
                if key not in environment:
                    os.environ.pop(key, None)
            return profile_directory()

    def test_an_explicit_profile_wins(self):
        self.assertEqual(
            self.directory(LUMENNA_PROFILE="/srv/tasks", XDG_DATA_HOME="/data"),
            Path("/srv/tasks"),
        )

    def test_xdg_data_home_is_honoured_as_lum_honours_it(self):
        self.assertEqual(self.directory(XDG_DATA_HOME="/data"), Path("/data/lumenna"))

    def test_without_it_the_store_is_under_local_share(self):
        self.assertEqual(self.directory(), Path.home() / ".local" / "share" / "lumenna")

    def test_a_relative_xdg_data_home_is_ignored_as_the_specification_says(self):
        self.assertEqual(
            self.directory(XDG_DATA_HOME="relative/path"),
            Path.home() / ".local" / "share" / "lumenna",
        )


if __name__ == "__main__":
    unittest.main()
