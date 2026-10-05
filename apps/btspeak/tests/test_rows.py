"""The tree, which is the only real logic on this side.

Everything else here is a call into `lum rpc`; this is the one place the app decides
something for itself, so it is the one place worth testing without a server.
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import btspeak_stub  # noqa: E402

btspeak_stub.install()

from tasks import char_offset  # noqa: E402
from rows import Tree, describe  # noqa: E402


def row(identifier, title, depth=0, **extra):
    return dict({"id": identifier, "title": title, "depth": depth, "state": []}, **extra)


class DescribingARow(unittest.TestCase):
    def test_the_title_comes_first_and_is_never_abbreviated(self):
        line = describe(row("a", "write the chapter", value="due 2026-09-01"))
        self.assertTrue(line.startswith("write the chapter"))

    def test_states_follow_the_value(self):
        line = describe(row("a", "ship it", value="due 2026-01-01", state=["overdue"]))
        self.assertEqual(line, "ship it, due 2026-01-01, overdue")

    def test_a_completed_row_says_so(self):
        self.assertIn("done", describe(row("a", "ship it", checked=True)))


class ReadingAFlatListAsATree(unittest.TestCase):
    def setUp(self):
        self.tree = Tree(
            [
                row("parent", "write the chapter"),
                row("child", "outline it", depth=1),
                row("grandchild", "list the sections", depth=2),
                row("other", "unrelated"),
            ]
        )

    def test_a_parent_is_the_nearest_shallower_row_above(self):
        self.assertEqual(self.tree.parent_of(1), 0)
        self.assertEqual(self.tree.parent_of(2), 1)
        self.assertIsNone(self.tree.parent_of(3))

    def test_collapsing_hides_every_descendant_not_just_the_children(self):
        self.tree.collapse(0)
        self.assertTrue(self.tree.visible(0))
        self.assertFalse(self.tree.visible(1))
        self.assertFalse(self.tree.visible(2), "a grandchild goes too")
        self.assertTrue(self.tree.visible(3), "a sibling stays")

    def test_expanding_puts_it_back(self):
        self.tree.collapse(0)
        self.tree.expand(0)
        self.assertTrue(self.tree.visible(2))

    def test_a_leaf_cannot_be_folded(self):
        self.assertEqual(self.tree.collapse(3), "")
        self.assertTrue(self.tree.visible(3))


class AnnouncingTheLevel(unittest.TestCase):
    """Depth is never indentation, and the level is announced when it changes."""

    def setUp(self):
        self.tree = Tree(
            [
                row("parent", "write the chapter"),
                row("first", "outline it", depth=1),
                row("second", "draft it", depth=1),
                row("other", "unrelated"),
            ]
        )

    def test_a_top_level_row_never_mentions_a_level(self):
        self.assertNotIn("level", self.tree.title_for(0))

    def test_the_first_row_of_a_new_level_says_which(self):
        self.assertIn("level 2", self.tree.title_for(1))

    def test_a_row_at_the_same_level_as_the_one_above_stays_quiet(self):
        self.assertNotIn("level", self.tree.title_for(2))

    def test_collapsing_recomputes_it_against_what_is_actually_read(self):
        # With the branch folded, "unrelated" follows the parent directly, and both are top
        # level — so nothing should announce a level at all.
        self.tree.collapse(0)
        self.assertNotIn("level", self.tree.title_for(3))

    def test_a_foldable_row_always_says_whether_it_is_folded(self):
        # It is the one thing the user cannot discover without pressing a key.
        self.assertIn("expanded", self.tree.title_for(0))
        self.tree.collapse(0)
        self.assertIn("collapsed", self.tree.title_for(0))


if __name__ == "__main__":
    unittest.main()


class ReadingServerOffsets(unittest.TestCase):
    """The server's spans count UTF-8 bytes; a Python string counts characters."""

    def test_ascii_offsets_are_unchanged(self):
        self.assertEqual(char_offset("call #wo", 6), 6)

    def test_an_accent_before_the_span_moves_it(self):
        text = "café #wo"
        start = len("café ".encode("utf-8"))
        self.assertEqual(text[char_offset(text, start):], "#wo")

    def test_an_offset_inside_a_character_does_not_split_it(self):
        self.assertEqual(char_offset("é", 1), 0)


class ScalingToALongList(unittest.TestCase):
    """A redraw asks every row for its title, so each title has to be cheap."""

    def test_a_large_folded_branch_does_not_slow_every_row_after_it(self):
        # The worst case for walking the list: every row after a big folded branch had to
        # scan back across all of it to find the row read before it.
        import time

        rows = [row("big", "big project")]
        for i in range(600):
            rows.append(row(f"c{i}", "child", depth=1))
            rows.append(row(f"g{i}", "grandchild", depth=2))
        for top in range(300):
            rows.append(row(f"t{top}", f"task {top}"))
        tree = Tree(rows)
        tree.collapse(0)

        started = time.perf_counter()
        titles = [tree.title_for(i) for i in range(len(rows)) if tree.visible(i)]
        elapsed = time.perf_counter() - started

        self.assertEqual(len(titles), 301, "the folded branch is hidden, the rest shown")
        self.assertFalse(any(t.startswith("level") for t in titles))
        self.assertLess(elapsed, 0.05, f"a redraw of 1500 rows took {elapsed:.3f}s")

    def test_changing_the_folded_set_directly_still_takes_effect(self):
        tree = Tree([row("a", "a"), row("b", "b", depth=1)])
        self.assertTrue(tree.visible(1))
        tree.collapsed.add("a")
        self.assertFalse(tree.visible(1))

