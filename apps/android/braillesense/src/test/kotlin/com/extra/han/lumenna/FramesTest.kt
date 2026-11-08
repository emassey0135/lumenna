package com.extra.han.lumenna

import com.extra.han.lumenna.device.Key
import com.extra.han.lumenna.device.Translated
import com.extra.han.lumenna.device.Translator
import com.extra.han.lumenna.ui.Control
import com.extra.han.lumenna.ui.FormFrame
import com.extra.han.lumenna.ui.LineEditor
import com.extra.han.lumenna.ui.Letters
import com.extra.han.lumenna.ui.ListFrame
import com.extra.han.lumenna.ui.MenuEntry
import com.extra.han.lumenna.ui.MenuFrame
import com.extra.han.lumenna.ui.Row
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

private val BY_LETTER = object : Letters {
    override fun letter(key: Key): Char? = when (key) {
        is Key.Cell -> 'a' + key.dots - 1
        is Key.Typed -> key.text.single()
        else -> null
    }

    override fun find(lines: List<String>, from: Int, letter: Char): Int? =
        (1..lines.size).map { (from + it) % lines.size }.firstOrNull { lines[it].startsWith(letter, ignoreCase = true) }
}

/** A table where a cell is its code point and a word is its letters back. */
private object Plain : Translator {
    override fun translate(text: String, cursor: Int): Translated {
        val cells = ByteArray(text.length) { text[it].code.toByte() }
        return Translated(cells, IntArray(text.length) { it }, IntArray(text.length) { it })
    }

    override fun back(cells: ByteArray): String = cells.joinToString("") { it.toInt().toChar().toString() }
}

class FramesTest {
    private fun list(vararg names: String, moved: (Row) -> Unit = {}) = ListFrame(
        "l", "Title", names.map { Row(it, it) }, null, "none", BY_LETTER, moved,
    )

    @Test fun aListMovesAndStopsAtItsEnds() {
        val frame = list("one", "two")
        assertEquals("one", frame.rendered.speech)
        frame.key(Key.Down)
        assertEquals("two", frame.rendered.speech)
        frame.key(Key.Down)
        assertEquals("two", frame.rendered.speech)
        frame.key(Key.First)
        assertEquals("one", frame.rendered.speech)
    }

    @Test fun anEmptyListSaysSo() = assertEquals("none", ListFrame("l", "", emptyList(), null, "none", BY_LETTER).rendered.speech)

    @Test fun aFirstLetterFindsTheNextRowWrappingRound() {
        val frame = list("apple", "banana", "avocado")
        frame.key(Key.Typed("a"))
        assertEquals("avocado", frame.rendered.speech)
        frame.key(Key.Typed("a"))
        assertEquals("apple", frame.rendered.speech)
    }

    @Test fun aListTheAppHoldsLeavesFirstLettersToTheApp() {
        val asked = mutableListOf<Char>()
        val frame = ListFrame("l", "", listOf(Row("a", "apple"), Row("b", "banana")), null, "", BY_LETTER, findInApp = { asked += it })
        frame.key(Key.Typed("b"))
        assertEquals(listOf('b'), asked)
        assertEquals("apple", frame.rendered.speech)
    }

    @Test fun aLetterNoRowStartsWithIsSaidSo() {
        val missing = mutableListOf<Char>()
        val frame = ListFrame("l", "", listOf(Row("a", "apple")), null, "", BY_LETTER, notFound = { missing += it })
        frame.key(Key.Typed("z"))
        assertEquals(listOf('z'), missing)
    }

    @Test fun aMenuLetterTheCoreGaveIsUsedAndATakenOneFallsToTheName() {
        var ran = ""
        val frame = MenuFrame(
            listOf(MenuEntry("New task", letter = 'a') { ran = "task" }, MenuEntry("New block", letter = 'a') { ran = "block" }),
            BY_LETTER, "p", closed = {},
        )
        assertEquals("New task (a) p", frame.rendered.speech)
        frame.key(Key.Down)
        assertEquals("New block (n) p", frame.rendered.speech)
        frame.key(Key.Typed("a"))
        assertEquals("task", ran)
    }

    @Test fun aFormsButtonIsPressedByItsLetter() {
        var pressed = false
        val frame = FormFrame("f", listOf(Control.Button("b", "Back up", 'b') { pressed = true }), null, BY_LETTER)
        assertFalse(frame.press('z'))
        assertTrue(frame.press('b'))
        assertTrue(pressed)
    }

    @Test fun movingTellsTheAppWhichRow() {
        val seen = mutableListOf<String>()
        val frame = list("one", "two", moved = { seen += it.key })
        frame.key(Key.Down)
        assertEquals(listOf("two"), seen)
    }

    @Test fun enterOpensAndTheContextKeyOpensActions() {
        var opened = false
        var actions = 0
        val row = Row("r", "r", open = { opened = true }, actions = listOf(MenuEntry("Do")))
        val frame = ListFrame("l", "", listOf(row), null, "", BY_LETTER, openActions = { actions++ })
        frame.key(Key.Enter)
        frame.key(Key.Context)
        assertTrue(opened)
        assertEquals(1, actions)
    }

    @Test fun aMenuReadsAsTheDevicesDoAndRunsByLetter() {
        var ran = false
        val frame = MenuFrame(listOf(MenuEntry("File", children = listOf(MenuEntry("New", hotkey = "Enter-n") { ran = true }))), BY_LETTER, "pull down", closed = {})
        assertEquals("File (f) pull down", frame.rendered.speech)
        frame.key(Key.Enter)
        assertEquals("New (n) pull down Enter-n", frame.rendered.speech)
        frame.key(Key.Typed("n"))
        assertTrue(ran)
    }

    @Test fun backspaceAtASubmenuGoesBackUpAndEscapeCloses() {
        var closed = false
        val frame = MenuFrame(listOf(MenuEntry("File", children = listOf(MenuEntry("New")))), BY_LETTER, "pull down", closed = { closed = true })
        frame.key(Key.Enter)
        frame.key(Key.Backspace)
        assertEquals("File (f) pull down", frame.rendered.speech)
        assertFalse(closed)
        frame.key(Key.Escape)
        assertTrue(closed)
    }

    @Test fun aMenuWrapsAndGoesToItsEnds() {
        val frame = MenuFrame(listOf(MenuEntry("One"), MenuEntry("Two"), MenuEntry("Three")), BY_LETTER, "p", closed = {})
        frame.key(Key.Up)
        assertTrue(frame.rendered.speech.startsWith("Three"))
        frame.key(Key.First)
        assertTrue(frame.rendered.speech.startsWith("One"))
        frame.key(Key.Last)
        assertTrue(frame.rendered.speech.startsWith("Three"))
    }

    @Test fun aLineTakesAWordOfCellsAndReadsItBackAtSpace() {
        val editor = LineEditor("", Plain)
        "hi".forEach { editor.cell(it.code) }
        assertEquals("", editor.text)
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
        editor.commit()
        assertEquals("cabd", editor.text)
    }

    @Test fun aRoutingKeyPutsTheCaretOverItsCharacter() {
        val editor = LineEditor("abcd", Plain)
        editor.route(1)
        assertEquals(1, editor.caret)
    }

    @Test fun aFormTabsAndEachControlTakesItsKeys() {
        var on = false
        var pressed = false
        val edit = LineEditor("", Plain)
        val frame = FormFrame(
            "f",
            listOf(Control.Edit("e", "Name", edit), Control.Toggle("t", "Flag", on, "off") { on = it }, Control.Button("b", "Save", press = { pressed = true })),
            null, BY_LETTER,
        )
        frame.key(Key.Cell('x'.code))
        frame.key(Key.Tab)
        assertEquals("x", edit.text)
        assertEquals("Flag: off", frame.rendered.speech)
        frame.key(Key.Space)
        assertTrue(on)
        frame.key(Key.Tab)
        frame.key(Key.Enter)
        assertTrue(pressed)
    }

    @Test fun aFormGoesToItsFirstAndLastControls() {
        val frame = FormFrame("f", listOf(Control.Static("a", "One"), Control.Static("b", "Two"), Control.Static("c", "Three")), null, BY_LETTER)
        frame.key(Key.Last)
        assertEquals("Three", frame.rendered.speech)
        frame.key(Key.First)
        assertEquals("One", frame.rendered.speech)
    }

    @Test fun aChoiceSteppedThroughAndFoundByLetter() {
        var chosen = "a"
        fun frame() = FormFrame(
            "f",
            listOf(Control.Choice("c", "Pick", listOf("a" to "Apple", "b" to "Banana", "c" to "Cherry"), listOf("a", "b", "c").indexOf(chosen), { chosen = it })),
            null, BY_LETTER,
        )
        frame().key(Key.Space)
        assertEquals("b", chosen)
        frame().key(Key.Typed("c"))
        assertEquals("c", chosen)
    }
}
