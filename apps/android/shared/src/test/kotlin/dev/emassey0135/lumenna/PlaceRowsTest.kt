package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.SidebarGroup
import org.junit.Assert.assertEquals
import org.junit.Test

class PlaceRowsTest {
    @Test
    fun aRowIsOpenWhileItOffersCollapseAndWhatIsUnderAFoldedOneIsHidden() {
        val rows = placeRowsWithin(
            listOf(
                group(SidebarGroup.PROJECTS),
                project("Work", 1u, folded = false),
                project("Reports", 2u),
                project("Home", 1u, folded = true),
                project("Garden", 2u),
            ),
            word = { "level ${it + 1u}" },
        )
        assertEquals(listOf(true, true, null, false, null), rows.map { it.expanded })
        assertEquals(listOf(false, false, false, false, true), rows.map { it.hidden })
        assertEquals(listOf(0, 1, 2, 1, 2), rows.map { it.depth })
        // Said where it changes among the rows shown: the hidden one says nothing.
        assertEquals(listOf(null, "level 2", "level 3", "level 2", null), rows.map { it.level })
    }

    @Test
    fun aFoldedHeadingHidesEverythingUnderItUntilTheNextHeading() {
        val rows = placeRowsWithin(
            listOf(
                group(SidebarGroup.PROJECTS, folded = true),
                project("Work", 1u, folded = false),
                project("Reports", 2u),
                group(SidebarGroup.LABELS),
            ),
            word = { "level ${it + 1u}" },
        )
        assertEquals(listOf(false, true, true, false), rows.map { it.hidden })
    }
}
