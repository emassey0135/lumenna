package dev.emassey0135.lumenna

import org.junit.Assert.assertEquals
import org.junit.Test

class LevelChangeTest {
    private val word = { depth: UInt -> "level ${depth + 1u}" }
    private val shown = listOf(Option("a", "Work"), Option("b", "Reports", depth = 1), Option("c", "Q3", depth = 1), Option("d", "Home"))

    @Test
    fun theLevelIsSaidOnlyWhereItChangesFromTheOptionShownBefore() {
        assertEquals(listOf(null, "level 2", null, "level 1"), shown.indices.map { levelChange(shown, it, word) })
    }

    @Test
    fun aWholeListSaysTheCoresLevelAndANarrowedOneWorksItOut() {
        val worded = shown.leveled(word).toMutableList().apply { this[1] = this[1].copy(detail = "project") }
        assertEquals("project, level 2", detailOf(worded, 1))
        // Narrowed to Q3 alone, it is the first shown, a level down from nothing.
        assertEquals("level 2", detailOf(listOf(shown[2]), 0, whole = false, word = word))
    }
}
