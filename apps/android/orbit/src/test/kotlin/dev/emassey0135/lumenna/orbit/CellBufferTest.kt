package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CellBufferTest {
    private val H = 0b10011
    private val I = 0b1010
    private val NUMBER_SIGN = 0b111100
    private val K = 0b101

    /** A word of letters, each a cell; a sign alone makes nothing yet; 132 commits "hi" as "HI!". */
    private val buffer = CellBuffer { cells, mode ->
        val text = cells.joinToString("") { cell -> when (cell) { H -> "h"; I -> "i"; else -> "" } }
        if (mode == CellBuffer.MODE_COMMIT && text == "hi") "HI!" else text
    }

    @Test fun aCellGivesItsWordSoFarAndWhatItMakes() {
        assertEquals(CellBuffer.Typed(listOf(H), 0, "h"), buffer.type(H))
        assertEquals(CellBuffer.Typed(listOf(H, I), 1, "hi"), buffer.type(I))
    }

    @Test fun aSignMakesNothingYet() {
        assertEquals("", buffer.type(NUMBER_SIGN).live)
        assertEquals("3 4 5 6", CellBuffer.dotNumbers(NUMBER_SIGN))
    }

    @Test fun spaceCommitsTheWordWithTheBlankAfterItAndGivesItInPrint() {
        val seen = mutableListOf<List<Int>>()
        val buffer = CellBuffer { cells, mode ->
            if (mode == CellBuffer.MODE_COMMIT) seen += cells
            if (mode == CellBuffer.MODE_COMMIT && cells == listOf(K, CellBuffer.BLANK)) "knowledge " else "k"
        }
        buffer.type(K)
        assertEquals("knowledge", buffer.space())
        assertEquals(listOf(listOf(K, CellBuffer.BLANK)), seen)
        assertEquals("knowledge ", buffer.display())
        assertEquals("a space after a space commits no word", "", buffer.space())
    }

    @Test fun selectionModeRunsFromWhereItBeganToTheCharacterAtTheCaret() {
        buffer.set("one two")
        buffer.moveEdge(start = true)
        assertTrue(buffer.toggleSelection())
        buffer.moveWord(1)
        buffer.moveCharacter(-1)
        assertFalse(buffer.toggleSelection())
        assertEquals("one ", buffer.selected())
        buffer.deleteSelection()
        assertEquals("two", buffer.display())
        assertEquals(0, buffer.caret)
    }

    @Test fun pasteReplacesTheSelectionAndMovingClearsAFinishedOne() {
        buffer.set("one two")
        buffer.selectAll()
        buffer.paste("three")
        assertEquals("three", buffer.display())
        buffer.selectAll()
        buffer.moveCharacter(-1)
        assertNull(buffer.selected())
    }

    @Test fun theWordIsShownLiveAndCommittedAtSpaceInCommitMode() {
        buffer.type(H)
        buffer.type(I)
        assertEquals("hi", buffer.display())
        assertEquals("HI!", buffer.space())
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
