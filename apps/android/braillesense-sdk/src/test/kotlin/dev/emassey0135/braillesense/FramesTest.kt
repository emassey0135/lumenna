package dev.emassey0135.braillesense

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.DeviceReading
import dev.emassey0135.braillesense.ui.DeviceLetters
import dev.emassey0135.braillesense.ui.Letters
import dev.emassey0135.braillesense.ui.ListFrame
import dev.emassey0135.braillesense.ui.MenuEntry
import dev.emassey0135.braillesense.ui.MenuFrame
import dev.emassey0135.braillesense.ui.Readings
import dev.emassey0135.braillesense.ui.Row
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** A cell is the letter of its number, so a test names a letter by its place in the alphabet. */
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

/** How the device composes a line is `deviceReading`'s to test: this keeps each frame's own part visible. */
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


    // ── The device's own letters ─────────────────────────────────────────────────────────

    @Test fun aCellIsTheLetterOfItsSixDots() {
        assertEquals('a', DeviceLetters.letter(Key.Cell(0x01)))
        assertEquals('x', DeviceLetters.letter(Key.Cell(0x2D)))
        assertNull("dot 7 makes no letter", DeviceLetters.letter(Key.Cell(0x41)))
        assertEquals('q', DeviceLetters.letter(Key.Typed("Q")))
    }

    @Test fun theDevicesFindingGoesOnFromTheRowInHandAndWrapsRound() {
        val lines = listOf("apple", "banana", "avocado")
        assertEquals(2, DeviceLetters.find(lines, 0, 'a'))
        assertEquals(0, DeviceLetters.find(lines, 2, 'a'))
        assertNull(DeviceLetters.find(lines, 0, 'z'))
    }

    @Test fun aMenuItemKeepsTheLetterItAsksForUnlessTakenThenItsNamesFirstFree() =
        assertEquals(listOf('n', 'e', 'o'), DeviceLetters.shortcuts(listOf("New" to 'n', "Next" to 'n', "None" to null)))
}
