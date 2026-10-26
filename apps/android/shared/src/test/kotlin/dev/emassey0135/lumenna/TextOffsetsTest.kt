package dev.emassey0135.lumenna

import org.junit.Assert.assertEquals
import org.junit.Test

class TextOffsetsTest {
    @Test
    fun anOffsetCountsBytesForTheCoreAndUnitsForKotlin() {
        val text = "café 📅 x"
        // "café " is six bytes and five units; the calendar is four bytes and two units.
        assertEquals(6u, TextOffsets.bytes(5, text))
        assertEquals(10u, TextOffsets.bytes(7, text))
        assertEquals(5, TextOffsets.utf16(6u, text))
        assertEquals(7, TextOffsets.utf16(10u, text))
    }

    @Test
    fun anOffsetOutsideTheTextIsKeptToItsEnds() {
        assertEquals(0u, TextOffsets.bytes(-3, "abc"))
        assertEquals(3u, TextOffsets.bytes(99, "abc"))
        assertEquals(3, TextOffsets.utf16(99u, "abc"))
    }
}
