package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CellBufferTest {
    private val H = 0b10011
    private val I = 0b1010
    private val NUMBER_SIGN = 0b111100

    /** A word of letters, each a cell; a sign alone makes nothing yet; 132 commits "hi" as "HI!". */
    private val buffer = CellBuffer { cells, mode ->
        val text = cells.joinToString("") { cell -> when (cell) { H -> "h"; I -> "i"; else -> "" } }
        if (mode == CellBuffer.MODE_COMMIT && text == "hi") "HI!" else text
    }

    @Test fun aCellEchoesTheLastCharacterItsWordNowMakes() {
        assertEquals(CellBuffer.Echo.Character('h'), buffer.type(H))
        assertEquals(CellBuffer.Echo.Character('i'), buffer.type(I))
    }

    @Test fun aSignWithNoCharacterEchoesItsDots() {
        assertEquals(CellBuffer.Echo.Dots(NUMBER_SIGN), buffer.type(NUMBER_SIGN))
        assertEquals("3 4 5 6", CellBuffer.dotNumbers(NUMBER_SIGN))
    }

    @Test fun theWordIsShownLiveAndCommittedAtSpaceInCommitMode() {
        buffer.type(H)
        buffer.type(I)
        assertEquals("hi", buffer.display())
        buffer.space()
        assertEquals("HI! ", buffer.display())
        assertEquals(4, buffer.caret)
    }

    @Test fun backspaceTakesACellThenACharacter() {
        buffer.set("a")
        buffer.type(H)
        assertEquals(CellBuffer.Deleted.Cell(H, "h"), buffer.backspace())
        assertEquals(CellBuffer.Deleted.Character('a'), buffer.backspace())
        assertEquals(CellBuffer.Deleted.Nothing, buffer.backspace())
    }

    @Test fun movingCommitsTheWordFirst() {
        buffer.set("ab")
        buffer.type(H)
        assertEquals('h', buffer.moveCharacter(-1))
        assertEquals("abh", buffer.confirmed())
    }

    @Test fun movingPastAnEndIsNull() {
        buffer.set("a")
        assertNull(buffer.moveCharacter(1))
        buffer.moveCharacter(-1)
        assertNull(buffer.moveCharacter(-1))
    }

    @Test fun movingByWordsLandsOnTheNextWord() {
        buffer.set("one two")
        buffer.moveEdge(start = true)
        assertEquals("two", buffer.moveWord(1))
        assertEquals("", buffer.moveWord(1))
        assertNull(buffer.moveWord(1))
        assertEquals("two", buffer.moveWord(-1))
    }

    @Test fun t9InsertsAtItsPosition() {
        buffer.set("xy")
        buffer.insert('h', 1)
        assertEquals("xhy", buffer.display())
        assertEquals(2, buffer.caret)
    }
}
