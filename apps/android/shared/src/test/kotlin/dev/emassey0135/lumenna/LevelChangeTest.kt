package dev.emassey0135.lumenna

import org.junit.Assert.assertEquals
import org.junit.Test

class LevelChangeTest {
    // The core's `levels`, as its own tests have it, without the native library.
    private val levels = { depths: List<UInt> ->
        depths.mapIndexed { index, depth -> if (depth != (depths.getOrNull(index - 1) ?: 0u)) "level ${depth + 1u}" else null }
    }
    private val shown = listOf(Option("a", "Work"), Option("b", "Reports", depth = 1), Option("c", "Q3", depth = 1), Option("d", "Home"))

    @Test
    fun anOptionsLevelIsTheCoresForTheOptionsShownInOrder() {
        assertEquals(listOf(null, "level 2", null, "level 1"), shown.leveled(levels).map { it.level })
        // Narrowed to Q3 alone, it is the first shown, a level down from nothing.
        assertEquals(listOf("level 2"), listOf(shown[2]).leveled(levels).map { it.level })
    }

    @Test
    fun anOptionsDetailIsWhatItIsThenItsLevel() {
        assertEquals("project, level 2", detailOf(Option("b", "Reports", "project", depth = 1, level = "level 2")))
        assertEquals("", detailOf(Option("a", "Work")))
    }
}
