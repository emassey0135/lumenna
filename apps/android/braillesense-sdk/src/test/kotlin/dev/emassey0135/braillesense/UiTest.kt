package dev.emassey0135.braillesense

import dev.emassey0135.braillesense.edit.Clipboard
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.output.BrailleDisplay
import dev.emassey0135.braillesense.output.Output
import dev.emassey0135.braillesense.reading.DeviceReading
import dev.emassey0135.braillesense.ui.Action
import dev.emassey0135.braillesense.ui.BrailleSenseUi
import dev.emassey0135.braillesense.ui.DeviceLetters
import dev.emassey0135.braillesense.ui.Field
import dev.emassey0135.braillesense.ui.FormValues
import dev.emassey0135.braillesense.ui.Item
import dev.emassey0135.braillesense.ui.MenuItem
import dev.emassey0135.braillesense.ui.Readings
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class UiTest {
    /** What was said, in order. */
    private val said = mutableListOf<String>()

    private val out = object : Output {
        override fun say(text: String) { said += text }
        override fun silence() {}
        override fun cells(cells: ByteArray, cursor: Int) {}
        override val width: Int = 40
    }

    private val clipboard = object : Clipboard {
        var held: String? = null
        override fun text(): String? = held
        override fun set(text: String?) { held = text }
    }

    /** How the device composes a line is `deviceReading`'s to test: here a line is its label and its words. */
    private val read = Readings { line ->
        DeviceReading(listOf(line.label, line.speech).filter { it.isNotEmpty() }.joinToString(": "), emptyList(), line.text, emptyList())
    }

    private val ui = BrailleSenseUi(BrailleDisplay(out, Plain), Plain, clipboard, read, speechDelay = 0)

    private val last: String get() = said.last()

    private fun type(text: String) = text.forEach { ui.press(Key.Cell(it.code)) }

    private val fruit = mutableListOf(Item("a", "Apple"), Item("b", "Banana"), Item("c", "Cherry"))

    @Test fun aListIsSaidWithItsTitleAsItOpensAndAMoveSaysTheRow() {
        ui.list("Fruit", { fruit })
        assertEquals("Fruit. Apple", last)
        ui.press(Key.Down)
        assertEquals("Banana", last)
    }

    @Test fun aRefreshKeepsTheRowInHandByItsKeyAndSaysNothing() {
        ui.list("Fruit", { fruit })
        ui.press(Key.Down)
        val before = said.size
        fruit.add(0, Item("z", "Apricot"))
        ui.refresh()
        assertEquals("nothing said", before, said.size)
        ui.press(Key.Up)
        assertEquals("one up from Banana, now third", "Apple", last)
    }

    @Test fun enterOpensARowAndALetterRunsTheRowThatShowsIt() {
        val opened = mutableListOf<String>()
        ui.list("Fruit", { listOf(Item("a", "Apple", open = { opened += "a" }), Item("b", "Banana", open = { opened += "b" }, letter = 'x')) })
        ui.press(Key.Enter)
        ui.press(Key.Typed("x"))
        assertEquals(listOf("a", "b"), opened)
    }

    @Test fun escapeClosesTheScreenSayingTheRowUnderItAndTheLastLeavesTheApp() {
        var left = false
        ui.onEmpty = { left = true }
        ui.list("Fruit", { fruit })
        ui.press(Key.Down)
        ui.list("More", { listOf(Item("m", "Mango")) })
        ui.press(Key.Escape)
        assertEquals("Banana", last)
        assertFalse(left)
        ui.press(Key.Escape)
        assertTrue(left)
    }

    @Test fun aLineIsTypedAndGivenAtEnterAndEscapeGivesNothing() {
        ui.list("Fruit", { fruit })
        var answer: String? = "unset"
        ui.askLine("Name") { answer = it }
        type("kiwi")
        ui.press(Key.Enter)
        assertEquals("kiwi", answer)
        assertEquals("back on the list", "Apple", last)
        ui.askLine("Name") { answer = it }
        ui.press(Key.Escape)
        assertNull(answer)
    }

    @Test fun aQuestionIsAnsweredOnceNoFirstAndSpaceStepsToYes() {
        ui.list("Fruit", { fruit })
        val answers = mutableListOf<Boolean>()
        ui.confirm("Delete it?") { answers += it }
        assertEquals("Delete it? No", last)
        ui.press(Key.Space)
        ui.press(Key.Enter)
        ui.press(Key.Enter)
        assertEquals(listOf(true), answers)
        ui.confirm("Delete it?") { answers += it }
        ui.press(Key.Escape)
        assertEquals("Escape is No", listOf(true, false), answers)
    }

    @Test fun aChoiceGivesThePlaceOfTheOptionPicked() {
        ui.list("Fruit", { fruit })
        var picked: Int? = null
        ui.choose("Colour", listOf("Red", "Green", "Blue"), selected = 1) { picked = it }
        assertEquals("Colour. Green", last)
        ui.press(Key.Down)
        ui.press(Key.Enter)
        assertEquals(2, picked)
    }

    @Test fun aFormMovesWithTabAndGivesWhatItHoldsAtOk() {
        ui.list("Fruit", { fruit })
        var values: FormValues? = null
        val fields = listOf(
            Field.Line("name", "Name"),
            Field.Choice("size", "Size", listOf("Small", "Large")),
            Field.Check("ripe", "Ripe"),
        )
        ui.form("New fruit", fields) { values = it }
        assertTrue("said as a dialog", last.startsWith("New fruit dialog"))
        type("fig")
        ui.press(Key.Tab)
        ui.press(Key.Space)
        assertEquals("Size: Large", last)
        ui.press(Key.Tab)
        ui.press(Key.Space)
        ui.press(Key.Tab)
        ui.press(Key.Enter)
        val held = values!!
        assertEquals("fig", held.text("name"))
        assertEquals(1, held.choice("size"))
        assertTrue(held.checked("ripe"))
    }

    @Test fun aFormLeftWithEscapeGivesNothing() {
        ui.list("Fruit", { fruit })
        var values: FormValues? = FormValues(emptyMap(), emptyMap(), emptyMap())
        ui.form("New fruit", listOf(Field.Line("name", "Name"))) { values = it }
        ui.press(Key.Escape)
        assertNull(values)
    }

    @Test fun theMenuKeyOpensTheScreensActionsAndEnterRunsOne() {
        var ran = false
        ui.list("Fruit", { fruit }, actions = { listOf(Action("Sort", 's') { ran = true }) })
        ui.press(Key.Menu)
        ui.press(Key.Enter)
        assertTrue(ran)
    }

    @Test fun aMenuGivesTheValueChosenAndClosedUnchosenSaysCancelThenTheRow() {
        ui.list("Fruit", { fruit })
        var chosen: String? = "unset"
        ui.menu(listOf(MenuItem("First", value = "1"), MenuItem("Second", value = "2"))) { chosen = it }
        ui.press(Key.Down)
        ui.press(Key.Enter)
        assertEquals("2", chosen)
        ui.menu(listOf(MenuItem("First", value = "1"))) { chosen = it }
        ui.press(Key.Escape)
        assertNull(chosen)
        assertEquals("Cancel. Apple", last)
    }

    @Test fun landingSaysTheRowWhereAPlainRefreshSaysNothing() {
        ui.list("Fruit", { fruit })
        val before = said.size
        ui.refresh()
        assertEquals(before, said.size)
        ui.land()
        assertEquals(before + 1, said.size)
        assertEquals("Apple", last)
    }

    @Test fun landingOnAKeyPutsThatRowInHandAndSaysIt() {
        ui.list("Fruit", { fruit })
        ui.land("c")
        assertEquals("Cherry", last)
        ui.press(Key.Up)
        assertEquals("Banana", last)
    }

    @Test fun aRowDeletedAfterAQuestionIsNotSaidTheRowInItsPlaceIs() {
        ui.list("Fruit", { fruit })
        ui.launch {
            if (ui.confirm("Delete Apple?")) {
                fruit.removeAt(0)
                ui.announce("Deleted")
                ui.refresh()
            }
        }
        ui.press(Key.Space)
        ui.press(Key.Enter)
        assertEquals("Deleted. Banana", last)
    }

    @Test fun aRefusalIsSaidWithTheLineThatFollowsIt() {
        ui.list("Fruit", { listOf(Item("a", "Apple", open = { ui.refused("Not ripe") })) })
        ui.press(Key.Enter)
        assertEquals("Not ripe. Apple", last)
    }

    @Test fun aBatchSaysOnlyWhereThePersonEndsUp() {
        ui.list("Fruit", { fruit })
        val before = said.size
        ui.batch {
            ui.list("More", { listOf(Item("m", "Mango")) })
            ui.list("Most", { listOf(Item("p", "Papaya")) })
        }
        assertEquals(before + 1, said.size)
        assertEquals("Most. Papaya", last)
    }

    @Test fun anAskInLaunchWaitsForItsAnswer() {
        ui.list("Fruit", { fruit })
        var answer: String? = null
        ui.launch { answer = ui.askLine("Name") }
        type("fig")
        assertNull(answer)
        ui.press(Key.Enter)
        assertEquals("fig", answer)
    }

    @Test fun space6UnfoldsAFoldedRowAndSpace3FoldsAnUnfoldedOne() {
        var open = false
        val folds = mutableListOf<Boolean>()
        ui.list("Fruit", { listOf(Item("a", if (open) "Apples, expanded" else "Apples, collapsed", expanded = open, fold = { folds += it; open = it })) })
        ui.press(Key.CharLeft)
        assertEquals("a folded row folds no further", emptyList<Boolean>(), folds)
        ui.press(Key.CharRight)
        ui.refresh()
        assertEquals("Apples, expanded", last)
        ui.press(Key.CharLeft)
        assertEquals(listOf(true, false), folds)
    }

    @Test fun aFormAskedForAKeyNoFieldHasThrows() {
        val values = FormValues(mapOf("name" to "fig"), emptyMap(), emptyMap())
        assertEquals("fig", values.text("name"))
        assertThrows(IllegalArgumentException::class.java) { values.text("size") }
        assertThrows(IllegalArgumentException::class.java) { values.choice("name") }
        assertThrows(IllegalArgumentException::class.java) { values.checked("ripe") }
    }

    @Test fun whatTheAppSaysAsAKeyRunsIsSaidWithTheLineAsOneThing() {
        ui.list("Fruit", { listOf(Item("a", "Apple", open = { ui.announce("Eaten") })) })
        ui.press(Key.Enter)
        assertEquals("Eaten. Apple", last)
    }

    @Test fun spaceTogglesACheckBoxSaidOnceTheAppHasChangedIt() {
        var ripe = false
        ui.list("Fruit", { listOf(Item("a", if (ripe) "Apple, ripe" else "Apple", toggle = { ripe = !ripe })) })
        val before = said.size
        ui.press(Key.Space)
        assertEquals("nothing said until the app shows the change", before, said.size)
        ui.refresh()
        assertEquals("Apple, ripe", last)
    }

    @Test fun aScreenThatNoLongerStandsClosesAtARefreshAndTheOneUnderIsSaid() {
        var open = true
        ui.list("Fruit", { fruit })
        ui.push(object : dev.emassey0135.braillesense.ui.Screen() {
            override val title = "Question"
            override fun frame() = dev.emassey0135.braillesense.ui.ListFrame("q", "Question", emptyList(), null, "Asked", DeviceLetters, readings = read)
            override fun stands() = open
        })
        open = false
        ui.refresh()
        assertEquals("Apple", last)
        assertEquals(1, ui.screens.size)
    }

    @Test fun aScreenThatTakesEscapeItselfStays() {
        var cancelled = 0
        ui.list("Fruit", { fruit })
        ui.push(object : dev.emassey0135.braillesense.ui.Screen() {
            override val title = "Form"
            override fun frame() = dev.emassey0135.braillesense.ui.ListFrame("f", "Form", emptyList(), null, "Empty", DeviceLetters, readings = read)
            override fun back() = true.also { cancelled++ }
        })
        ui.press(Key.Escape)
        assertEquals(1, cancelled)
        assertEquals(2, ui.screens.size)
    }

    @Test fun spaceWithALetterRunsTheRowsCommandElseSaysNothingIsOnIt() {
        val ran = mutableListOf<Char>()
        ui.list("Fruit", { listOf(Item("a", "Apple", command = { letter -> (letter == 'e').also { if (it) ran += letter } })) })
        ui.press(Key.SpaceLetter('e'))
        assertEquals(listOf('e'), ran)
        ui.press(Key.SpaceLetter('q'))
        assertEquals("Nothing on q.", last)
    }
}
