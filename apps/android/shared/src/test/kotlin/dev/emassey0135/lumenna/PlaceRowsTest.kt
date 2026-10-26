package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.SidebarGroup
import org.junit.Assert.assertEquals
import org.junit.Test

class PlaceRowsTest {
    @Test
    fun aRowIsOpenWhileItOffersCollapseAndWhatIsUnderAFoldedOneIsHidden() {
        val rows = placeRows(
            listOf(
                group(SidebarGroup.PROJECTS),
                project("Work", 1u, folded = false),
                project("Reports", 2u),
                project("Home", 1u, folded = true),
                project("Garden", 2u),
            ),
        )
        assertEquals(listOf(true, true, null, false, null), rows.map { it.expanded })
        assertEquals(listOf(false, false, false, false, true), rows.map { it.hidden })
        assertEquals(listOf(0, 1, 2, 1, 2), rows.map { it.depth })
    }

    @Test
    fun aFoldedHeadingHidesEverythingUnderItUntilTheNextHeading() {
        val rows = placeRows(
            listOf(
                group(SidebarGroup.PROJECTS, folded = true),
                project("Work", 1u, folded = false),
                project("Reports", 2u),
                group(SidebarGroup.LABELS),
            ),
        )
        assertEquals(listOf(false, true, true, false), rows.map { it.hidden })
    }
}
