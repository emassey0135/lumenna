package dev.emassey0135.braillesense

import dev.emassey0135.braillesense.braille.Echo
import dev.emassey0135.braillesense.braille.Translated
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.edit.BoxEditing
import dev.emassey0135.braillesense.edit.Clipboard
import dev.emassey0135.braillesense.edit.EditDisplay
import dev.emassey0135.braillesense.edit.Edited
import dev.emassey0135.braillesense.edit.LineEditor
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.input.Key
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A table where a cell is its code point and a word is its letters back. */
internal object Plain : Translator {
    override fun computer(text: String): ByteArray = translate(text).cells

    override fun translate(text: String, cursor: Int): Translated {
        val cells = ByteArray(text.length) { text[it].code.toByte() }
        return Translated(cells, IntArray(text.length) { it }, IntArray(text.length) { it })
    }

    override fun back(cells: ByteArray): String = cells.joinToString("") { it.toInt().toChar().toString() }
}

/** One contraction: "the" is one cell. */
private const val THE = 0x2e

private object Contracted : Translator {
    override fun computer(text: String): ByteArray = Plain.computer(text)

    override fun translate(text: String, cursor: Int): Translated {
        val cells = ArrayList<Byte>()
        val textAt = ArrayList<Int>()
        var at = 0
        while (at < text.length) {
            textAt += at
            if (text.startsWith("the", at)) { cells += THE.toByte(); at += 3 } else { cells += text[at].code.toByte(); at++ }
        }
        return Translated(cells.toByteArray(), IntArray(text.length), textAt.toIntArray())
    }

    override fun back(cells: ByteArray): String = cells.joinToString("") { if (it.toInt() == THE) "the" else it.toInt().toChar().toString() }
}

/** Shows anything and reads back only lower case: what a round trip through a table can lose. */
private object Lossy : Translator {
    override fun computer(text: String): ByteArray = Plain.computer(text)

    override fun translate(text: String, cursor: Int): Translated = Plain.translate(text, cursor)

    override fun back(cells: ByteArray): String = Plain.back(cells).lowercase()
}

/** A clipboard of the test's own. */
private class Board(var held: String? = null) : Clipboard {
    override fun text() = held

    override fun set(text: String?) {
        held = text
    }
}

/** A display that knows nothing of where its lines break. */
private val DISPLAY = object : EditDisplay {
    override val width = 40

    override fun cursorLine(): IntRange? = null
}

/** Characters and words echoed, in computer braille: a cell is said as the character it is. */
private fun editing(echo: Echo = Echo.of(1, 0), clipboard: Clipboard = Board()) = BoxEditing(Plain, clipboard, { echo })

class EditTest {
    @Test fun aLineTakesAWordOfCellsAndReadsItBackAtSpace() {
        val editor = LineEditor("", Plain)
        "hi".forEach { editor.cell(it.code) }
        assertEquals("hi", editor.text)
        editor.space()
        assertEquals("hi ", editor.text)
        assertEquals("hi", editor.takeFinished())
    }

    @Test fun backspaceTakesTheLastCellThenTheLastCharacter() {
        val editor = LineEditor("a", Plain)
        editor.cell('b'.code)
        editor.backspace()
        assertEquals("a", editor.text)
        editor.backspace()
        assertEquals("", editor.text)
    }

    @Test fun theCaretMovesThroughTheLineAndAWordGoesInAtIt() {
        val editor = LineEditor("ad", Plain)
        editor.left()
        editor.type("b")
        editor.right()
        editor.start()
        editor.cell('c'.code)
        assertEquals("cabd", editor.text)
    }

    @Test fun aRoutingKeyPutsTheCaretOverItsCharacter() {
        val editor = LineEditor("abcd", Plain)
        editor.route(1)
        assertEquals(1, editor.caret)
    }

    @Test fun backspaceTakesOneCellOfAContraction() {
        val editor = LineEditor("", Contracted)
        editor.cell(THE)
        editor.cell('n'.code)
        assertEquals("then", editor.text)
        editor.backspace()
        assertEquals("the", editor.text)
        val removed = editor.takeRemoved()!!
        assertEquals('n'.code.toByte(), removed.cell)
        assertArrayEquals("the word it was in, to say it as it was", byteArrayOf(THE.toByte(), 'n'.code.toByte()), removed.word)
        assertEquals(1, removed.at)
    }

    @Test fun theCellJustTypedIsSaidWithItsWord() {
        val editor = LineEditor("", Plain)
        "ab".forEach { editor.cell(it.code) }
        val typed = editor.cellBeforeCaret()!!
        assertEquals('b'.code.toByte(), typed.cell)
        assertEquals(1, typed.at)
        assertNull("past the end of the line there is no cell", editor.cellAtCaret())
        editor.start()
        assertEquals('a'.code.toByte(), editor.cellAtCaret()!!.cell)
    }

    @Test fun theLineShowsWhatWasTyped() {
        val editor = LineEditor("", Contracted)
        editor.cell(THE)
        assertArrayEquals(byteArrayOf(THE.toByte()), editor.shown().cells)
        assertEquals(1, editor.shown().cursor)
    }

    @Test fun aWordNobodyTouchedKeepsItsTextWhenAnotherIsEdited() {
        // This translator could not read "Road" back from its cells; the edit never asks it to.
        val editor = LineEditor("Road milk", Lossy)
        editor.backspace()
        assertEquals("Road mil", editor.text)
    }

    @Test fun aRoutingKeyLandsOnTheCellUnderIt() {
        val editor = LineEditor("", Contracted)
        editor.cell(THE)
        editor.cell('n'.code)
        editor.route(1)
        assertEquals(1, editor.caret)
        editor.cell('x'.code)
        assertEquals("thexn", editor.text)
    }

    @Test fun spaceEndsAWordAndBackspaceJoinsItAgain() {
        val editor = LineEditor("", Plain)
        "ab".forEach { editor.cell(it.code) }
        editor.space()
        assertEquals("ab", editor.takeFinished())
        editor.cell('c'.code)
        assertEquals("ab c", editor.text)
        editor.backspace()
        editor.backspace()
        assertEquals(LineEditor.BLANK, editor.takeRemoved()!!.cell)
        assertEquals("ab", editor.text)
    }

    @Test fun typingOnTheKeyboardKeepsItsText() {
        val editor = LineEditor("", Contracted)
        editor.type("t")
        editor.type("h")
        editor.type("e")
        assertEquals("the", editor.text)
        assertArrayEquals(byteArrayOf(THE.toByte()), editor.shown().cells)
    }

    @Test fun aSpaceTypedOnTheKeyboardSplitsTheWord() {
        val editor = LineEditor("", Plain)
        editor.type("a b")
        assertEquals("a b", editor.text)
    }

    @Test fun aMultiLineBoxTakesEnterAsANewLineAndBackspaceJoinsItAgain() {
        val box = TextBox("ab", Plain, multiLine = true)
        val editing = editing()
        assertEquals("the word it finished, where the echo says words", Edited.Taken(true, "ab"), editing.press(box, Key.Enter, DISPLAY))
        editing.press(box, Key.Cell('c'.code), DISPLAY)
        assertEquals("ab\nc", box.text)
        assertEquals(1, box.row)
        editing.press(box, Key.Backspace, DISPLAY)
        assertEquals("backspace at a line's start joins it to the line before", Edited.Taken(true, "previous line"), editing.press(box, Key.Backspace, DISPLAY))
        assertEquals("ab", box.text)
        assertEquals("Tab is the app's", Edited.NotTaken, editing.press(box, Key.Tab, DISPLAY))
        assertEquals("so is Enter in a box of one line", Edited.NotTaken, editing.press(TextBox("x", Plain), Key.Enter, DISPLAY))
    }

    @Test fun aMultiLineBoxMovesByLineAndToItsTopAndBottom() {
        val box = TextBox("one\ntwo\nthree", Plain, multiLine = true)
        val editing = editing()
        assertEquals("three", box.line.text)
        assertEquals("the line it lands on", Edited.Taken(said = "two"), editing.press(box, Key.Up, DISPLAY))
        assertEquals("the cursor keeps as far along as the line allows", 3, box.line.caret)
        assertEquals("the top says the word it lands in", Edited.Taken(said = "one"), editing.press(box, Key.First, DISPLAY))
        assertEquals(0, box.row)
        assertEquals(0, box.line.caret)
        assertEquals(Edited.Taken(said = "three"), editing.press(box, Key.Last, DISPLAY))
        assertEquals(2, box.row)
        assertEquals(5, box.line.caret)
    }

    @Test fun aSingleLineBoxTakesANewLinePastedAsASpace() {
        val box = TextBox("", Plain)
        box.insert("a\nb")
        assertEquals("a b", box.text)
        assertEquals(1, box.count)
    }

    @Test fun aSelectionRunsFromWhereItStartedToTheCursor() {
        val box = TextBox("milk eggs", Plain)
        box.line.start()
        box.startSelection()
        box.line.wordRight()
        assertEquals("milk ", box.selected())
        assertTrue(box.deleteSelected())
        assertEquals("eggs", box.text)
        assertFalse(box.selecting)
        assertNull("with nothing selected there is nothing to copy", box.selected())
    }

    @Test fun aSelectionAcrossLinesKeepsItsNewLines() {
        val box = TextBox("one\ntwo", Plain, multiLine = true)
        box.top()
        box.line.right()
        box.startSelection()
        box.down()
        assertEquals("ne\nt", box.selected())
        box.deleteSelected()
        assertEquals("owo", box.text)
        box.selectAll()
        assertEquals("owo", box.selected())
    }

    @Test fun pastingAtTheCursorTypesTheTextIn() {
        val box = TextBox("ac", Plain, multiLine = true)
        box.line.left()
        box.insert("b\nx")
        assertEquals("ab\nxc", box.text)
    }

    @Test fun aNewTableWritesTheCellsAgainAndKeepsTheText() {
        var table: Translator = Contracted
        val switching = object : Translator {
            override fun computer(text: String) = table.computer(text)
            override fun translate(text: String, cursor: Int) = table.translate(text, cursor)
            override fun back(cells: ByteArray) = table.back(cells)
        }
        val box = TextBox("", switching)
        box.line.cell(THE)
        box.line.cell('n'.code)
        box.settle()
        table = Plain
        box.recell()
        assertEquals("then", box.text)
        assertArrayEquals("then".toByteArray(), box.line.shown().cells)
    }

    @Test fun backspaceTwoFiveTakesTheWordAtTheCursorAndTheSpacesAfterIt() {
        val box = TextBox("buy  milk now", Plain)
        box.line.start()
        box.line.wordRight()
        box.line.right()
        assertEquals(TextBox.WordRemoved.Word("milk"), box.deleteWord())
        assertEquals("buy  now", box.text)
        assertEquals("the cursor where the word began", 5, box.line.caret)
    }

    @Test fun backspaceTwoFiveAtTheEndTakesTheLastWordAndLeavesTheSpaceBeforeIt() {
        val box = TextBox("buy milk", Plain)
        assertEquals(TextBox.WordRemoved.Word("milk"), box.deleteWord())
        assertEquals("buy ", box.text)
        assertEquals("a space at the end goes by itself", TextBox.WordRemoved.Space, box.deleteWord())
        assertEquals("buy", box.text)
        assertEquals(TextBox.WordRemoved.Word("buy"), box.deleteWord())
        assertEquals("", box.text)
        assertNull("an empty line has nothing to take", box.deleteWord())
    }

    @Test fun backspaceTwoFiveAtTheEndOfALineJoinsTheNextOnAsANewParagraph() {
        val box = TextBox("one\ntwo", Plain, multiLine = true)
        box.up()
        box.line.end()
        assertEquals(TextBox.WordRemoved.LineBreak, box.deleteWord())
        assertEquals("onetwo", box.text)
    }

    @Test fun backspaceOneFourTakesThePartOfTheLineShownAndAnEmptiedLineGoes() {
        val box = TextBox("one two\nthree", Plain, multiLine = true)
        box.up()
        assertTrue(box.deleteLine(4, 7))
        assertEquals("one \nthree", box.text)
        assertEquals(4, box.line.caret)
        box.down()
        assertTrue(box.deleteLine(0, 5))
        assertEquals("the emptied last line goes, the cursor at the end of the one before", "one ", box.text)
        assertEquals(0, box.row)
        assertEquals(4, box.line.caret)
        assertTrue(box.deleteLine(0, 4))
        assertEquals("", box.text)
        assertFalse("nothing left to take", box.deleteLine(0, 0))
    }

    @Test fun deletingTheParagraphTakesTheWholeLine() {
        val box = TextBox("one\ntwo\nthree", Plain, multiLine = true)
        box.up()
        assertTrue(box.deleteParagraph())
        assertEquals("one\nthree", box.text)
        assertEquals("three", box.line.text)
        assertEquals(0, box.line.caret)
        val single = TextBox("milk", Plain)
        assertTrue(single.deleteParagraph())
        assertEquals("", single.text)
        assertFalse(single.deleteParagraph())
    }

    @Test fun enterUReadsASelectionInOneLineWholeAndOneAcrossLinesToTheEndOfItsFirst() {
        val box = TextBox("milk eggs", Plain)
        assertNull(box.selectionStart(40))
        box.line.start()
        box.startSelection()
        box.line.wordRight()
        assertEquals("milk ", box.selectionStart(40))
        val notes = TextBox("buy milk and eggs\nbread", Plain, multiLine = true)
        notes.top()
        notes.line.wordRight()
        notes.startSelection()
        notes.down()
        assertEquals("milk and eggs", notes.selectionStart(40))
        assertEquals("as a display of 10 cells breaks it", "milk ", notes.selectionStart(10))
    }

    @Test fun spaceDInAMultiLineBoxTakesTheCharacterAtTheCursorOrJoinsTheNextLine() {
        val box = TextBox("ab\nc", Plain, multiLine = true)
        val editing = editing()
        editing.press(box, Key.Up, DISPLAY)
        editing.press(box, Key.LineStart, DISPLAY)
        assertEquals("the character it took", Edited.Taken(true, "a"), editing.press(box, Key.Delete, DISPLAY))
        assertEquals("b\nc", box.text)
        editing.press(box, Key.LineEnd, DISPLAY)
        assertEquals("a line break taken says nothing", Edited.Taken(true, null), editing.press(box, Key.Delete, DISPLAY))
        assertEquals("bc", box.text)
    }

    @Test fun aCellIsEchoedAsTheDeviceSaysIt() {
        val box = TextBox("", Plain)
        assertEquals(Edited.Taken(said = "a"), editing().press(box, Key.Cell('a'.code), DISPLAY))
        assertEquals("punctuation by its name", Edited.Taken(said = "comma"), editing().press(box, Key.Cell(','.code), DISPLAY))
        assertEquals("nothing with the echo off", Edited.Taken(said = null), editing(Echo.of(0, 0)).press(box, Key.Cell('b'.code), DISPLAY))
        assertEquals("the word it ended at Space", Edited.Taken(true, "a,b"), editing().press(box, Key.Space, DISPLAY))
        assertEquals("Space where words are not echoed", Edited.Taken(true, "Space"), editing(Echo.of(2, 0)).press(box, Key.Space, DISPLAY))
    }

    @Test fun homeAndEndSayTheWordTheyLandInAndNothingWhereTheCursorWasAlready() {
        val box = TextBox("buy milk", Plain)
        val editing = editing()
        assertEquals(Edited.Taken(said = "buy"), editing.press(box, Key.LineStart, DISPLAY))
        assertEquals(Edited.Taken(said = null), editing.press(box, Key.LineStart, DISPLAY))
        assertEquals(Edited.Taken(said = "milk"), editing.press(box, Key.LineEnd, DISPLAY))
    }

    @Test fun copyAndPasteGoThroughTheClipboardInTheDevicesWords() {
        val board = Board()
        val editing = editing(clipboard = board)
        val box = TextBox("milk eggs", Plain)
        assertEquals(Edited.Taken(said = "Block is not set."), editing.press(box, Key.EnterLetter('c'), DISPLAY))
        box.line.start()
        assertEquals(Edited.Taken(said = "Start selection"), editing.press(box, Key.EnterDots(0x03), DISPLAY))
        box.line.wordRight()
        assertEquals("Enter-C", Edited.Taken(said = "Copy complete"), editing.press(box, Key.EnterDots(0x09), DISPLAY))
        assertEquals("milk ", board.held)
        assertFalse(box.selecting)
        box.line.end()
        assertEquals(Edited.Taken(true, "Paste"), editing.press(box, Key.EnterLetter('v'), DISPLAY))
        assertEquals("milk eggsmilk ", box.text)
        assertEquals(Edited.Taken(said = "Clear clipboard"), editing.press(box, Key.EnterLetter('d'), DISPLAY))
        assertNull(board.held)
    }

    @Test fun backspaceOverASelectionAsksFirst() {
        val box = TextBox("milk", Plain)
        val editing = editing()
        editing.press(box, Key.EnterLetter('a'), DISPLAY)
        val asked = editing.press(box, Key.Backspace, DISPLAY) as Edited.Ask
        assertEquals("Delete selected text?", asked.question)
        assertEquals("No reads the line again", Edited.Taken(said = "milk"), asked.answer(false))
        assertEquals("milk", box.text)
        assertEquals(Edited.Taken(true, "Delete complete."), asked.answer(true))
        assertEquals("", box.text)
    }

    @Test fun escapeLeavesASelectionAndOtherwiseIsTheApps() {
        val box = TextBox("milk", Plain)
        val editing = editing()
        assertEquals(Edited.NotTaken, editing.press(box, Key.Escape, DISPLAY))
        editing.press(box, Key.EnterLetter('a'), DISPLAY)
        assertEquals(Edited.Taken(said = "Selection canceled."), editing.press(box, Key.Escape, DISPLAY))
        assertFalse(box.selecting)
    }

    @Test fun backspaceOneFourTakesWhatTheDisplayShowsAtTheCursor() {
        val box = TextBox("one two", Plain)
        val shows = object : EditDisplay {
            override val width = 4

            override fun cursorLine() = 4 until 7
        }
        assertEquals(Edited.Taken(true, "Delete complete."), editing().press(box, Key.DeleteLine, shows))
        assertEquals("one ", box.text)
    }
}
