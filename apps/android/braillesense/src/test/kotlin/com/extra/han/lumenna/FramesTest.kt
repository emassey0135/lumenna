package com.extra.han.lumenna

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.braille.Translated
import dev.emassey0135.braillesense.braille.Translator
import com.extra.han.lumenna.ui.FormFrame
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.reading.DeviceReading
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import dev.emassey0135.lumenna.app.Choice
import dev.emassey0135.lumenna.app.ControlMove
import dev.emassey0135.lumenna.app.ControlSends
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.app.DialogShown
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.braille.DialogController
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.ui.Letters
import dev.emassey0135.braillesense.ui.Readings

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

class FramesTest {
    // ── Dialogs, over the shared DialogController ─────────────────────────────────────────

    /** What the app was sent, as the core's `control_event` would write it (the native library is absent here). */
    private val sent = mutableListOf<String>()

    private fun events(sends: ControlSends, value: String, how: ControlMove): List<ShellEvent> {
        val move = when (how) {
            is ControlMove.Typed -> "typed"
            is ControlMove.Set -> "set"
            is ControlMove.Pressed -> "pressed"
        }
        return when (sends) {
            is ControlSends.Press -> if (how is ControlMove.Pressed) sends.field0 else emptyList()
            is ControlSends.TaskField -> listOf(ShellEvent.Key("${sends.field0} $move $value"))
            is ControlSends.Pick -> listOf(ShellEvent.Key("pick $move $value"))
            else -> emptyList()
        }
    }

    private fun control(key: String, kind: ControlType, label: String = key, value: String = "", sends: ControlSends = ControlSends.TaskField(key)) =
        ControlShown(key, kind, label, value, "", "", emptyList(), null, false, null, null, "", false, false, false, sends)

    private fun option(id: String, title: String) = Choice(id, title, 0u, null, null, null, null, null, null)

    private fun button(key: String, label: String = key, letter: String = "", default: Boolean = false) =
        control(key, ControlType.BUTTON, label, sends = ControlSends.Press(listOf(ShellEvent.Key("press $key")))).copy(letter = letter, default = default)

    private fun choice(key: String, vararg options: Pair<String, String>, kind: ControlType = ControlType.CHOICE, sends: ControlSends = ControlSends.TaskField(key)) =
        control(key, kind, sends = sends).copy(options = options.map { (id, title) -> option(id, title) }, selected = 0u)

    /** [said] is the dialog's prompt, which the core joins (`DialogShown.prompt`): given here as the core would. */
    private fun form(vararg controls: ControlShown, focus: Int = 0, id: String = "form", boxes: Map<String, TextBox> = emptyMap(), prompt: Boolean = false, said: String = "Form"): FormFrame {
        val controller = DialogController(DialogShown(id = id, title = "Form", prompt = said, controls = controls.toList(), focus = focus.toUInt(), cancel = emptyList()), { sent += (it as ShellEvent.Key).field0 }, events = ::events)
        val made = HashMap(boxes)
        return FormFrame(controller, BY_LETTER, READ, boxOf = { made.getOrPut(it.key) { TextBox(controller.text(it), Plain, it.kind == ControlType.LINES) } },
            letterOf = { it.letter.singleOrNull() }, prompt = prompt)
    }

    @Test fun aFormsButtonIsPressedByItsLetter() {
        val frame = form(button("back-up", "Back up", letter = "b"))
        assertFalse(frame.press('z'))
        assertTrue(frame.press('b'))
        assertEquals(listOf("press back-up"), sent)
    }

    @Test fun tabAloneMovesBetweenControlsAndSpaceOneAndFourDoNot() {
        val frame = form(control("note", ControlType.TEXT, value = "Note"), choice("priority", "1" to "High", "2" to "Low"), button("save"))
        assertFalse(frame.key(Key.Down))
        assertEquals("note", frame.current?.key)
        frame.key(Key.Tab)
        assertEquals("priority", frame.current?.key)
        frame.key(Key.Down)
        assertEquals("priority", frame.current?.key)
        assertEquals(listOf("priority set 2"), sent)
        frame.key(Key.Tab)
        assertEquals("save", frame.current?.key)
        assertFalse(frame.key(Key.Up))
        frame.key(Key.ShiftTab)
        assertEquals("priority", frame.current?.key)
    }

    @Test fun backspaceCyclesAComboBoxBackAndAToggleChanges() {
        val frame = form(choice("p", "1" to "A", "2" to "B"), control("flag", ControlType.CHECK))
        frame.key(Key.Backspace)
        assertEquals("round from the first to the last", listOf("p set 2"), sent)
        frame.key(Key.Tab)
        frame.key(Key.Backspace)
        assertEquals("flag set true", sent.last())
    }

    @Test fun anEditBoxGivesItsLabelAndLeavesSpaceOneToNothing() {
        val frame = form(control("title", ControlType.LINE, "Title", "milk"))
        assertEquals("Title: milk", frame.rendered.speech)
        assertFalse(frame.key(Key.Down))
    }

    @Test fun aLineIsToldAsTypedAndTakenAsItIsLeft() {
        val frame = form(control("name", ControlType.LINE, "Name"), control("flag", ControlType.CHECK, "Flag"), button("save"))
        frame.box()!!.line.cell('x'.code)
        frame.key(Key.Tab)
        assertEquals("a cell's word is told as the line is left, then taken", listOf("name typed x", "name set x"), sent)
        assertEquals("Flag", frame.rendered.speech)
        frame.key(Key.Space)
        assertEquals("flag set true", sent.last())
        frame.key(Key.Tab)
        frame.key(Key.Enter)
        assertEquals("press save", sent.last())
    }

    @Test fun theFirstAndLastKeysStayOnTheControl() {
        val frame = form(control("a", ControlType.TEXT, value = "One"), control("b", ControlType.TEXT, value = "Two"))
        assertFalse(frame.key(Key.Last))
        assertEquals("One", frame.rendered.speech)
    }

    @Test fun aComboBoxGoesToItsFirstAndLastOptionAndFindsByLetter() {
        val frame = form(choice("fruit", "a" to "Apple", "b" to "Banana", "c" to "Cherry"))
        frame.key(Key.Last)
        assertEquals("fruit set c", sent.last())
        frame.key(Key.First)
        assertEquals("fruit set a", sent.last())
        frame.key(Key.Space)
        assertEquals("fruit set b", sent.last())
        frame.key(Key.Typed("c"))
        assertEquals("fruit set c", sent.last())
        assertEquals("what is chosen shows before the app answers", "fruit: Cherry", frame.rendered.speech)
    }

    @Test fun enterInAMultiLineBoxIsTheBoxsAndInABoxOfOneLinePressesTheDefault() {
        val frame = form(control("notes", ControlType.LINES, "Notes"), control("title", ControlType.LINE, "Title"), button("save", default = true))
        assertFalse("the box's own key, a new line", frame.key(Key.Enter))
        frame.key(Key.Tab)
        assertTrue(frame.key(Key.Enter))
        assertEquals("press save", sent.last())
    }

    @Test fun aDialogsListMovesInPlaceAndEnterPicksTheOneInHand() {
        val frame = form(choice("choices", "a" to "Home", "b" to "Office", "c" to "Garden", kind = ControlType.LIST, sends = ControlSends.Pick), id = "question:1")
        frame.key(Key.Down)
        assertTrue("a list only moves until it is picked", sent.isEmpty())
        frame.key(Key.Typed("g"))
        assertEquals("a first letter finds its option", "choices: Garden", frame.rendered.speech)
        frame.key(Key.Enter)
        assertEquals(listOf("pick pressed c"), sent)
    }

    @Test fun aQuestionsAnswersAreSteppedWithSpaceAndBackspace() {
        val frame = form(control("message", ControlType.TEXT, value = "It cannot be undone."), button("cancel"), button("yes"), focus = 1, id = "question:2", prompt = true,
            said = "Delete it? It cannot be undone")
        assertEquals("the core's prompt is said as it opens", "Delete it? It cannot be undone", frame.said)
        assertEquals("the lines ahead of the first answer", listOf("It cannot be undone."), frame.leading)
        assertEquals("cancel", frame.current?.key)
        frame.key(Key.Space)
        assertEquals("yes", frame.current?.key)
        frame.key(Key.Space)
        assertEquals("wrapping round, never onto the message", "cancel", frame.current?.key)
    }
}
