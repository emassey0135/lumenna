package com.extra.han.lumenna

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.braille.Translated
import dev.emassey0135.braillesense.braille.Translator
import com.extra.han.lumenna.ui.Completing
import com.extra.han.lumenna.ui.FormFrame
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.lumenna.app.Candidate
import dev.emassey0135.lumenna.app.Completions
import dev.emassey0135.lumenna.app.Syntax
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
        ControlShown(key, kind, label, value, "", "", emptyList(), null, false, null, null, "", false, false, false, sends, "", emptyList())

    private fun option(id: String, title: String) = Choice(id, title, 0u, null, null, null, null, null, null)

    private fun button(key: String, label: String = key, letter: String = "", default: Boolean = false) =
        control(key, ControlType.BUTTON, label, sends = ControlSends.Press(listOf(ShellEvent.Key("press $key")))).copy(letter = letter, default = default)

    private fun choice(key: String, vararg options: Pair<String, String>, kind: ControlType = ControlType.CHOICE, sends: ControlSends = ControlSends.TaskField(key)) =
        control(key, kind, sends = sends).copy(options = options.map { (id, title) -> option(id, title) }, selected = 0u)

    /** [said] is the dialog's prompt, which the core joins (`DialogShown.prompt`): given here as the core would. */
    private fun form(vararg controls: ControlShown, focus: Int = 0, id: String = "form", boxes: Map<String, TextBox> = emptyMap(), prompt: Boolean = false, said: String = "Form", completing: Completing? = null): FormFrame {
        val controller = DialogController(DialogShown(id = id, title = "Form", prompt = said, controls = controls.toList(), focus = focus.toUInt(), cancel = emptyList()), { sent += (it as ShellEvent.Key).field0 }, events = ::events)
        val made = HashMap(boxes)
        return FormFrame(controller, BY_LETTER, READ, boxOf = { made.getOrPut(it.key) { TextBox(controller.text(it), Plain, it.kind == ControlType.LINES) } },
            letterOf = { it.letter.singleOrNull() }, prompt = prompt, completing = completing)
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

    @Test fun aListsOptionReadsAsTheCoreSaysItWithWhyItIsOffered() {
        val picks = choice("choices", "a" to "Write the report", "b" to "Call Sam", kind = ControlType.LIST, sends = ControlSends.Pick)
            .copy(optionLines = listOf("Write the report. Suggested because overdue", "Call Sam"))
        val frame = form(picks, id = "question:3")
        assertEquals("choices: Write the report. Suggested because overdue", frame.rendered.speech)
    }

    // ── Completion inside a line ──────────────────────────────────────────────────────────

    /** What the core's `put_in` does: [choice] over the UTF-8 bytes from [start] to [end]. */
    private fun splice(text: String, start: UInt, end: UInt, choice: String): String {
        val bytes = text.encodeToByteArray()
        return (bytes.copyOfRange(0, start.toInt()) + choice.encodeToByteArray() + bytes.copyOfRange(end.toInt(), bytes.size)).decodeToString()
    }

    private val asked = mutableListOf<Pair<String, UInt>>()
    private val offered = mutableListOf<Pair<String, List<String>>>()
    private val said = mutableListOf<String>()

    /** Completion over [found], answering each offer with [choose]. */
    private fun completing(found: Completions?, choose: Int?) = Completing(
        complete = { text, cursor, _ -> asked += text to cursor; found },
        offer = { title, lines, chosen -> offered += title to lines; chosen(choose) },
        say = { said += it },
        showCompletions = { "Show completions" },
        nothingToComplete = { "Nothing to complete here." },
        lineOf = { "${it.text}, ${it.kind}" },
        splice = ::splice,
    )

    private val work = Completions("2 completions", emptyList(), 4u, 7u, listOf(Candidate("#Work", "project", "project Work"), Candidate("#Workshop", "project", "project Workshop")))

    private fun quickAdd() = control("line", ControlType.LINE, "Task").copy(syntax = Syntax.QUICK_ADD)

    @Test fun spaceFourInALineThatCompletesOffersTheCoresCandidatesAndPutsInTheOneChosen() {
        val box = TextBox("Buy #wo tomorrow", Plain).apply { line.route(7) }
        val frame = form(quickAdd(), boxes = mapOf("line" to box), completing = completing(work, 0))
        assertTrue(frame.key(Key.Down))
        assertEquals("asked at the cursor, in UTF-8 bytes", listOf("Buy #wo tomorrow" to 7u), asked)
        assertEquals(listOf("2 completions" to listOf("#Work, project", "#Workshop, project")), offered)
        assertEquals("Buy #Work tomorrow", box.text)
        assertEquals("the cursor after what was put in", 9, box.line.caret)
        assertEquals("sent as typed, so the core has it", listOf("line typed Buy #Work tomorrow"), sent)
    }

    @Test fun aCompletionIsNeverPutInByItselfEvenWithOneCandidate() {
        val one = work.copy(candidates = work.candidates.take(1))
        val box = TextBox("Buy #wo", Plain)
        val frame = form(quickAdd(), boxes = mapOf("line" to box), completing = completing(one, null))
        frame.key(Key.Down)
        assertEquals("offered all the same", 1, offered.size)
        assertEquals("left unchosen, the line is as it was", "Buy #wo", box.text)
        assertTrue(sent.isEmpty())
    }

    @Test fun nothingToCompleteSaysTheCoresAnnouncementElseItsWordsForNothing() {
        val frame = form(quickAdd(), boxes = mapOf("line" to TextBox("Buy", Plain)), completing = completing(work.copy(announcement = "No completions", candidates = emptyList()), 0))
        frame.key(Key.Down)
        assertEquals(listOf("No completions"), said)
        assertTrue(offered.isEmpty())
        val unread = form(quickAdd(), boxes = mapOf("line" to TextBox("Buy", Plain)), completing = completing(null, 0))
        unread.key(Key.Down)
        assertEquals("Nothing to complete here.", said.last())
    }

    @Test fun aLineWithoutASyntaxLeavesSpaceFourToNothing() {
        val frame = form(control("title", ControlType.LINE, "Title"), completing = completing(work, 0))
        assertFalse(frame.key(Key.Down))
        assertTrue(asked.isEmpty())
    }

    @Test fun theCaretIsCountedByWordsBetweenCellsAndText() {
        val cells = TextBox("ab cd", Plain).line.shown().cells
        assertEquals(0, Completing.textCursor("ab cd", cells, 0))
        assertEquals("inside a word, its end", 2, Completing.textCursor("ab cd", cells, 1))
        assertEquals("after the space, the next word's start", 3, Completing.textCursor("ab cd", cells, 3))
        assertEquals(5, Completing.textCursor("ab cd", cells, 5))
        assertEquals(2, Completing.cellCursor("ab cd", 2, cells))
        assertEquals(3, Completing.cellCursor("ab cd", 3, cells))
        assertEquals(5, Completing.cellCursor("ab cd", 5, cells))
    }
}
