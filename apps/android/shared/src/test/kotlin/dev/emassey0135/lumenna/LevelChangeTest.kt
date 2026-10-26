package dev.emassey0135.lumenna

import org.junit.Assert.assertEquals
import org.junit.Test

class LevelChangeTest {
    private val shown = listOf(Option("a", "Work"), Option("b", "Reports", depth = 1), Option("c", "Q3", depth = 1), Option("d", "Home"))

    @Test
    fun theLevelIsSaidOnlyWhereItChangesFromTheOptionBefore() {
        assertEquals(listOf(null, "level 2", null, "level 1"), shown.indices.map { levelChange(shown, it) })
    }

    @Test
    fun anOptionsDetailComesBeforeItsLevel() {
        val detailed = shown.toMutableList().apply { this[1] = this[1].copy(detail = "project") }
        assertEquals("project, level 2", detailOf(detailed, 1))
    }
}
