package com.extra.han.lumenna

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.braille.Translated
import dev.emassey0135.braillesense.braille.Translator
import com.extra.han.lumenna.ui.FormFrame
import dev.emassey0135.braillesense.edit.TextBox
import com.extra.han.lumenna.ui.Letters
import com.extra.han.lumenna.ui.ListFrame
import com.extra.han.lumenna.ui.MenuEntry
import com.extra.han.lumenna.ui.MenuFrame
import com.extra.han.lumenna.ui.Readings
import com.extra.han.lumenna.ui.Row
import dev.emassey0135.braillesense.reading.ControlRole
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
        val seen = mutableListOf<Pair<Int, Int>>()
        val readings = Readings { line -> seen += line.position to line.count; READ.read(line) }
        ListFrame("l", "", listOf(Row("a", "a"), Row("b", "b")), "b", "", BY_LETTER, readings = readings).rendered
        ListFrame("l", "", listOf(Row("y", "Yes")), null, "", BY_LETTER, readings = readings, prompt = true).rendered
        assertEquals(listOf(2 to 2, 0 to 0), seen)
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

    @Test fun aHotKeyAMenuShowsRunsWhileTheMenuIsOpen() {
        var ran = false
        var closed = false
        val undo = MenuEntry("Undo", hotkey = "Space-u", letter = 'u') { ran = true }
        val frame = MenuFrame(listOf(MenuEntry("Edit", children = listOf(undo))), BY_LETTER, READ, closed = { closed = true })
        frame.key(Key.SpaceLetter('u'))
        assertTrue("from the top of the bar, its submenu unopened", ran)
        assertTrue(closed)
    }


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

    private fun form(vararg controls: ControlShown, focus: Int = 0, id: String = "form", boxes: Map<String, TextBox> = emptyMap(), prompt: Boolean = false): FormFrame {
        val controller = DialogController(DialogShown(id, "Form", controls.toList(), focus.toUInt(), emptyList()), { sent += (it as ShellEvent.Key).field0 }, events = ::events)
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
        val frame = form(control("message", ControlType.TEXT, value = "It cannot be undone."), button("cancel"), button("yes"), focus = 1, id = "question:2", prompt = true)
        assertEquals("the message is said as it opens", "It cannot be undone.", frame.preamble)
        assertEquals("cancel", frame.current?.key)
        frame.key(Key.Space)
        assertEquals("yes", frame.current?.key)
        frame.key(Key.Space)
        assertEquals("wrapping round, never onto the message", "cancel", frame.current?.key)
    }
}
