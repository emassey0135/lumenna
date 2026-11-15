package dev.emassey0135.orbit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CellNamesTest {
    private val names = CellNames()

    @Test fun theOrbitNamesItsSignsAndTheLowerCellsAsItsOwnFieldsDo() {
        assertEquals("capital sign", names.name(listOf(0b10_0000), 0))
        assertEquals("capital word sign", names.name(listOf(0b10_0000, 0b10_0000), 1))
        assertEquals("number sign", names.name(listOf(0b11_1100), 0))
        assertEquals("low b", names.name(listOf(0b00_0110), 0))
        assertEquals("not the device's \"greek sign\"", "hyphen", names.name(listOf(0b10_0100), 0))
    }

    @Test fun aLetterIsTranslatedNotNamed() {
        assertNull(names.name(listOf(0b00_0001), 0))
        assertNull(names.name(emptyList(), 0))
    }
}
