package com.extra.han.lumenna

import com.extra.han.lumenna.device.Cells
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CellsTest {
    private fun cells(vararg bytes: Int) = bytes.map { it.toByte() }.toByteArray()

    @Test fun aShortLineFillsOneWindow() {
        val display = Cells(40)
        display.set(cells(1, 2, 3), -1)
        assertArrayEquals(cells(1, 2, 3), display.shown())
        assertFalse(display.forward())
    }

    @Test fun windowsBreakAtSpaces() {
        val display = Cells(4)
        display.set(cells(1, 2, 0, 3, 4, 5, 0, 6), -1)
        assertArrayEquals(cells(1, 2, 0), display.shown())
        assertTrue(display.forward())
        assertArrayEquals(cells(3, 4, 5, 0), display.shown())
        assertTrue(display.forward())
        assertArrayEquals(cells(6), display.shown())
        assertFalse(display.forward())
        assertTrue(display.back())
    }

    @Test fun aWordLongerThanTheDisplayIsCutAtItsWidth() {
        val display = Cells(3)
        display.set(cells(1, 2, 3, 4, 5), -1)
        assertArrayEquals(cells(1, 2, 3), display.shown())
        assertTrue(display.forward())
        assertArrayEquals(cells(4, 5), display.shown())
    }

    @Test fun theCursorOpensOnItsWindowAndFallsOnItsCell() {
        val display = Cells(4)
        display.set(cells(1, 2, 0, 3, 4, 5, 0, 6), 5)
        assertArrayEquals(cells(3, 4, 5, 0), display.shown())
        assertEquals(2, display.cursorShown())
    }

    @Test fun aRoutingKeyNamesTheCellOfTheWholeLine() {
        val display = Cells(4)
        display.set(cells(1, 2, 0, 3, 4, 5, 0, 6), -1)
        display.forward()
        assertEquals(4, display.routed(1))
    }
}
