package dev.emassey0135.braillesense

import dev.emassey0135.braillesense.edit.CellEcho
import org.junit.Assert.assertEquals
import org.junit.Test

class CellEchoTest {
    private fun cells(vararg word: IntArray): ByteArray =
        word.map { dots -> dots.fold(0) { bits, dot -> bits or (1 shl (dot - 1)) }.toByte() }.toByteArray()

    private fun dots(vararg dot: Int) = dot

    @Test fun aCellIsSaidByItsLettersNeverAsTheWordItStandsForAlone() {
        val still = cells(dots(3, 4))
        assertEquals("dots 3-4 alone are \"still\", but are said as their letters", "s t", CellEcho.cell(still, 0, true))
        val test = cells(dots(6), dots(2, 3, 4, 5), dots(1, 5), dots(3, 4))
        assertEquals("the capital sign is said by its dots", "dot six", CellEcho.cell(test, 0, true))
        assertEquals("t", CellEcho.cell(test, 1, true))
        assertEquals("e", CellEcho.cell(test, 2, true))
        assertEquals("s t", CellEcho.cell(test, 3, true))
        assertEquals("the", CellEcho.cell(cells(dots(2, 3, 4, 6)), 0, true))
        assertEquals("underscore", CellEcho.cell(cells(dots(4, 5, 6)), 0, true))
        assertEquals("four six", CellEcho.cell(cells(dots(4, 6)), 0, true))
    }

    @Test fun aLowerCellIsPunctuationOnlyAtTheEndOfAContractedWord() {
        val period = cells(dots(1, 2, 3, 4), dots(2, 5, 6))
        assertEquals("period", CellEcho.cell(period, 1, true))
        val inside = cells(dots(2, 5, 6), dots(1, 2, 3, 4))
        assertEquals("inside a word it is a lower cell", "lower d", CellEcho.cell(inside, 0, true))
        assertEquals("uncontracted it is always the punctuation", "period", CellEcho.cell(inside, 0, false))
        assertEquals("e n", CellEcho.cell(cells(dots(2, 6)), 0, true))
    }

    @Test fun afterANumberSignAToJAreDigitsUntilSomethingElse() {
        val number = cells(dots(3, 4, 5, 6), dots(1), dots(2, 4, 5), dots(1, 2))
        assertEquals("number sign", CellEcho.cell(number, 0, true))
        assertEquals("1", CellEcho.cell(number, 1, true))
        assertEquals("0", CellEcho.cell(number, 2, true))
        val letters = cells(dots(3, 4, 5, 6), dots(1), dots(1, 3, 4), dots(1, 2))
        assertEquals("a cell that is no digit ends the number", "b", CellEcho.cell(letters, 3, true))
    }

    @Test fun dots7And8AloneAreSaidAsDots() {
        assertEquals("dot seven", CellEcho.cell(byteArrayOf(0x40), 0, true))
        assertEquals("seven eight", CellEcho.cell(byteArrayOf(0xc0.toByte()), 0, true))
        assertEquals("a", CellEcho.cell(byteArrayOf(0x41), 0, true))
        assertEquals("", CellEcho.cell(ByteArray(0), 0, true))
    }

    @Test fun computerBraillePunctuationIsSaidByItsName() {
        assertEquals("comma", CellEcho.character(","))
        assertEquals("period", CellEcho.character("."))
        assertEquals("back slash", CellEcho.character("\\"))
        assertEquals("tilde", CellEcho.character("~"))
        assertEquals("space", CellEcho.character(" "))
        assertEquals("a", CellEcho.character("a"))
        assertEquals("only a character by itself is named", "ab", CellEcho.character("ab"))
    }
}
