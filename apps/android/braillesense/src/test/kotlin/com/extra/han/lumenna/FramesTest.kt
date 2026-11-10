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
import com.extra.han.lumenna.ui.Readings
import com.extra.han.lumenna.ui.Row
import dev.emassey0135.lumenna.app.ControlRole
import dev.emassey0135.lumenna.app.DeviceReading
import org.junit.Assert.assertArrayEquals
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

    /** The core's own rule is the core's to test: first-come letters, then a name's first free one. */
    override fun shortcuts(items: List<Pair<String, Char?>>): List<Char?> {
        val taken = HashSet<Char>()
        val given = items.map { (_, letter) -> letter?.takeIf(taken::add) }
        return items.zip(given).map { (item, letter) ->
            letter ?: item.first.lowercase().firstOrNull { it !in taken }?.also { taken += it }
        }
    }
}

/** A table where a cell is its code point and a word is its letters back. */
private object Plain : Translator {
    override fun computer(text: String): ByteArray = translate(text).cells

    override fun translate(text: String, cursor: Int): Translated {
        val cells = ByteArray(text.length) { text[it].code.toByte() }
        return Translated(cells, IntArray(text.length) { it }, IntArray(text.length) { it })
    }

    override fun back(cells: ByteArray): String = cells.joinToString("") { it.toInt().toChar().toString() }
}

/** The core's composition is the core's to test: this keeps each frame's own part visible. */
private val READ = Readings { line ->
    val name = listOf(line.label, line.speech).filter { it.isNotEmpty() }.joinToString(": ")
    val letter = line.letter?.let { " ($it)" }.orEmpty()
    val key = line.hotKey?.let { " $it" }.orEmpty()
    val role = if (line.role == ControlRole.MENU) " pulldown" else if (line.asks) " dialog" else ""
    DeviceReading("$name$letter$key$role", emptyList(), line.text, emptyList())
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

class FramesTest {
    private fun list(vararg names: String, moved: (Row) -> Unit = {}) = ListFrame(
        "l", "Title", names.map { Row(it, it) }, null, "none", BY_LETTER, moved, readings = READ,
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

    @Test fun anEmptyListSaysSo() = assertEquals("none", ListFrame("l", "", emptyList(), null, "none", BY_LETTER, readings = READ).rendered.speech)

    @Test fun aFirstLetterFindsTheNextRowWrappingRound() {
        val frame = list("apple", "banana", "avocado")
        frame.key(Key.Typed("a"))
        assertEquals("avocado", frame.rendered.speech)
        frame.key(Key.Typed("a"))
        assertEquals("apple", frame.rendered.speech)
    }

    @Test fun aListTheAppHoldsLeavesFirstLettersToTheApp() {
        val asked = mutableListOf<Char>()
        val frame = ListFrame("l", "", listOf(Row("a", "apple"), Row("b", "banana")), null, "", BY_LETTER, findInApp = { asked += it }, readings = READ)
        frame.key(Key.Typed("b"))
        assertEquals(listOf('b'), asked)
        assertEquals("apple", frame.rendered.speech)
    }

    @Test fun aLetterNoRowStartsWithIsSaidSo() {
        val missing = mutableListOf<Char>()
        val frame = ListFrame("l", "", listOf(Row("a", "apple")), null, "", BY_LETTER, notFound = { missing += it }, readings = READ)
        frame.key(Key.Typed("z"))
        assertEquals(listOf('z'), missing)
    }

    @Test fun aMenuLetterTheCoreGaveIsUsedAndATakenOneFallsToTheName() {
        var ran = ""
        val frame = MenuFrame(
            listOf(MenuEntry("New task", letter = 'a') { ran = "task" }, MenuEntry("New block", letter = 'a') { ran = "block" }),
            BY_LETTER, READ, closed = {},
        )
        assertEquals("New task (a)", frame.rendered.speech)
        frame.key(Key.Down)
        assertEquals("New block (n)", frame.rendered.speech)
        frame.key(Key.Typed("a"))
        assertEquals("task", ran)
    }

    @Test fun aFormsButtonIsPressedByItsLetter() {
        var pressed = false
        val frame = FormFrame("f", "Form", listOf(Control.Button("b", "Back up", 'b') { pressed = true }), null, BY_LETTER, READ)
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
        val frame = ListFrame("l", "", listOf(row), null, "", BY_LETTER, openActions = { actions++ }, readings = READ)
        frame.key(Key.Enter)
        frame.key(Key.Context)
        assertTrue(opened)
        assertEquals(1, actions)
    }

    @Test fun aMenuReadsAsTheDevicesDoAndRunsByLetter() {
        var ran = false
        val frame = MenuFrame(listOf(MenuEntry("File", children = listOf(MenuEntry("New", hotkey = "Enter-n") { ran = true }))), BY_LETTER, READ, closed = {})
        assertEquals("File (f) pulldown", frame.rendered.speech)
        frame.key(Key.Enter)
        assertEquals("New (n) Enter-n", frame.rendered.speech)
        frame.key(Key.Typed("n"))
        assertTrue(ran)
    }

    @Test fun onlyAnItemThatAsksSaysItOpensADialog() {
        val frame = MenuFrame(listOf(MenuEntry("Rename", asks = true), MenuEntry("Undo")), BY_LETTER, READ, closed = {})
        assertEquals("Rename (r) dialog", frame.rendered.speech)
        frame.key(Key.Down)
        assertEquals("Undo (u)", frame.rendered.speech)
    }

    @Test fun escapeInASubmenuGoesUpALevelAndAtTheTopLeavesCancelled() {
        val closed = mutableListOf<Boolean>()
        val frame = MenuFrame(listOf(MenuEntry("File", children = listOf(MenuEntry("New")))), BY_LETTER, READ, closed = { closed += it })
        frame.key(Key.Enter)
        frame.key(Key.Escape)
        assertEquals("File (f) pulldown", frame.rendered.speech)
        assertEquals(emptyList<Boolean>(), closed)
        frame.key(Key.Escape)
        assertEquals(listOf(true), closed)
    }

    @Test fun backspaceGoesUpALevelToo() {
        val closed = mutableListOf<Boolean>()
        val frame = MenuFrame(listOf(MenuEntry("File", children = listOf(MenuEntry("New")))), BY_LETTER, READ, closed = { closed += it })
        frame.key(Key.Enter)
        frame.key(Key.Backspace)
        assertEquals("File (f) pulldown", frame.rendered.speech)
        assertEquals(emptyList<Boolean>(), closed)
    }

    @Test fun theMenuKeyClosesTheMenuFromASubmenu() {
        val closed = mutableListOf<Boolean>()
        val frame = MenuFrame(listOf(MenuEntry("File", children = listOf(MenuEntry("New")))), BY_LETTER, READ, closed = { closed += it })
        frame.key(Key.Enter)
        frame.key(Key.Menu)
        assertEquals(listOf(true), closed)
    }

    @Test fun choosingAnItemIsNotCancelling() {
        val closed = mutableListOf<Boolean>()
        val frame = MenuFrame(listOf(MenuEntry("New")), BY_LETTER, READ, closed = { closed += it })
        frame.key(Key.Enter)
        assertEquals(listOf(false), closed)
    }

    @Test fun aMenuGivesTheCoresLettersFirstSoANameCannotTakeOne() {
        val frame = MenuFrame(listOf(MenuEntry("Today"), MenuEntry("Tasks", letter = 't')), BY_LETTER, READ, closed = {})
        assertEquals("Today (o)", frame.rendered.speech)
    }

    @Test fun anItemWhoseLettersAreAllTakenShowsNone() {
        val frame = MenuFrame(listOf(MenuEntry("A", letter = 'a'), MenuEntry("A")), BY_LETTER, READ, closed = {})
        frame.key(Key.Down)
        assertEquals("A", frame.rendered.speech)
    }

    @Test fun aLetterOnlyGoesToItsItemWhenTheDeviceSaysNotToOpen() {
        var ran = false
        val frame = MenuFrame(listOf(MenuEntry("One"), MenuEntry("Two", letter = 'w') { ran = true }), BY_LETTER, READ, opensOnLetter = { false }, closed = {})
        frame.key(Key.Typed("w"))
        assertEquals("Two (w)", frame.rendered.speech)
        assertFalse(ran)
    }

    @Test fun aRowCarriesItsNumberAndAPromptsAnswersDoNot() {
        val seen = mutableListOf<Pair<UInt, UInt>>()
        val readings = Readings { line -> seen += line.position to line.count; READ.read(line) }
        ListFrame("l", "", listOf(Row("a", "a"), Row("b", "b")), "b", "", BY_LETTER, readings = readings).rendered
        ListFrame("l", "", listOf(Row("y", "Yes")), null, "", BY_LETTER, readings = readings, prompt = true).rendered
        assertEquals(listOf(2u to 2u, 0u to 0u), seen)
    }

    @Test fun enterWithNothingToOpenSaysSoAndAnActionlessRowToo() {
        var said = ""
        val frame = ListFrame("l", "", listOf(Row("a", "apple")), null, "", BY_LETTER, readings = READ,
            opensNothing = { said = "nothing" }, noActions = { said = "no actions" })
        frame.key(Key.Enter)
        assertEquals("nothing", said)
        frame.key(Key.Context)
        assertEquals("no actions", said)
    }

    @Test fun tabAloneMovesBetweenControlsAndSpaceOneAndFourDoNot() {
        var chosen = ""
        val controls = listOf(
            Control.Static("s", "Note"),
            Control.Choice("c", "Priority", listOf("1" to "High", "2" to "Low"), 0, { chosen = it }),
            Control.Button("b", "Save") {},
        )
        val frame = FormFrame("f", "Form", controls, null, BY_LETTER, READ)
        assertFalse(frame.key(Key.Down))
        assertEquals("s", frame.current?.key)
        frame.key(Key.Tab)
        assertEquals("c", frame.current?.key)
        frame.key(Key.Down)
        assertEquals("c", frame.current?.key)
        assertEquals("2", chosen)
        frame.key(Key.Tab)
        assertEquals("b", frame.current?.key)
        assertFalse(frame.key(Key.Up))
        frame.key(Key.ShiftTab)
        assertEquals("c", frame.current?.key)
    }

    @Test fun backspaceCyclesAComboBoxBackAndAToggleChanges() {
        var chosen = ""
        var on = false
        val controls = listOf(
            Control.Choice("c", "P", listOf("1" to "A", "2" to "B"), 0, { chosen = it }),
            Control.Toggle("t", "Flag", false) { on = it },
        )
        val frame = FormFrame("f", null, controls, null, BY_LETTER, READ)
        frame.key(Key.Backspace)
        assertEquals("2", chosen)
        frame.key(Key.Tab)
        frame.key(Key.Backspace)
        assertTrue(on)
    }

    @Test fun anEditBoxGivesItsLabelAndLeavesSpaceOneToNothing() {
        val editor = LineEditor("milk", Plain)
        val frame = FormFrame("f", null, listOf(Control.Edit("e", "Title", editor)), null, BY_LETTER, READ)
        assertEquals("Title: milk", frame.rendered.speech)
        assertFalse(frame.key(Key.Down))
    }

    @Test fun aListFoldsItsRowWithSpaceThreeAndSix() {
        var folded = ""
        val row = Row("r", "r", collapse = { folded = "c" }, expand = { folded = "e" })
        val frame = ListFrame("l", "", listOf(row), null, "", BY_LETTER, readings = READ)
        frame.key(Key.CharLeft)
        assertEquals("c", folded)
        frame.key(Key.CharRight)
        assertEquals("e", folded)
    }

    @Test fun aRowWithNoFoldingLeavesTheKey() = assertFalse(list("a").key(Key.CharLeft))

    @Test fun aShortcutLetterRunsBeforeFirstLetterMoving() {
        val pressed = mutableListOf<Char>()
        val frame = ListFrame("l", "", listOf(Row("a", "apple"), Row("t", "today", letter = 't')), null, "", BY_LETTER, readings = READ,
            byLetter = { pressed += it; it == 't' })
        frame.key(Key.Typed("t"))
        assertEquals(listOf('t'), pressed)
        assertEquals("apple", frame.rendered.speech)
        frame.key(Key.Typed("a"))
        assertEquals("apple", frame.rendered.speech)
    }

    @Test fun aPromptsAnswersWrapWithSpaceAndBackspace() {
        val frame = ListFrame("l", "", listOf(Row("y", "Yes"), Row("n", "No")), null, "", BY_LETTER, readings = READ, prompt = true)
        frame.key(Key.Space)
        assertEquals("No", frame.rendered.speech)
        frame.key(Key.Space)
        assertEquals("Yes", frame.rendered.speech)
        frame.key(Key.Backspace)
        assertEquals("No", frame.rendered.speech)
    }

    @Test fun aMenuWrapsAndGoesToItsEnds() {
        val frame = MenuFrame(listOf(MenuEntry("One"), MenuEntry("Two"), MenuEntry("Three")), BY_LETTER, READ, closed = {})
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

    @Test fun aFormTabsAndEachControlTakesItsKeys() {
        var on = false
        var pressed = false
        val edit = LineEditor("", Plain)
        val frame = FormFrame(
            "f",
            "Form",
            listOf(Control.Edit("e", "Name", edit), Control.Toggle("t", "Flag", on) { on = it }, Control.Button("b", "Save", press = { pressed = true })),
            null, BY_LETTER, READ,
        )
        frame.key(Key.Cell('x'.code))
        frame.key(Key.Tab)
        assertEquals("x", edit.text)
        assertEquals("Flag", frame.rendered.speech)
        frame.key(Key.Space)
        assertTrue(on)
        frame.key(Key.Tab)
        frame.key(Key.Enter)
        assertTrue(pressed)
    }

    @Test fun theFirstAndLastKeysStayOnTheControl() {
        val frame = FormFrame("f", null, listOf(Control.Static("a", "One"), Control.Static("b", "Two")), null, BY_LETTER, READ)
        assertFalse(frame.key(Key.Last))
        assertEquals("One", frame.rendered.speech)
    }

    @Test fun aComboBoxGoesToItsFirstAndLastOption() {
        var chosen = ""
        val options = listOf("a" to "Apple", "b" to "Banana")
        val frame = FormFrame("f", null, listOf(Control.Choice("c", "Pick", options, 0, { chosen = it })), null, BY_LETTER, READ)
        frame.key(Key.Last)
        assertEquals("b", chosen)
        frame.key(Key.First)
        assertEquals("a", chosen)
    }

    @Test fun aChoiceSteppedThroughAndFoundByLetter() {
        var chosen = "a"
        fun frame() = FormFrame(
            "f",
            null,
            listOf(Control.Choice("c", "Pick", listOf("a" to "Apple", "b" to "Banana", "c" to "Cherry"), listOf("a", "b", "c").indexOf(chosen), { chosen = it })),
            null, BY_LETTER, READ,
        )
        frame().key(Key.Space)
        assertEquals("b", chosen)
        frame().key(Key.Typed("c"))
        assertEquals("c", chosen)
    }

    @Test fun backspaceTakesOneCellOfAContraction() {
        val editor = LineEditor("", Contracted)
        editor.cell(THE)
        editor.cell('n'.code)
        assertEquals("then", editor.text)
        editor.backspace()
        assertEquals("the", editor.text)
        assertEquals('n'.code.toByte(), editor.takeRemoved())
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
        assertEquals(LineEditor.BLANK, editor.takeRemoved())
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
}
