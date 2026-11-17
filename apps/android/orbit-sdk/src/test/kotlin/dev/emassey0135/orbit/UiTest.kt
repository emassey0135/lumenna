package dev.emassey0135.orbit

import dev.emassey0135.orbit.ui.Action
import dev.emassey0135.orbit.ui.Field
import dev.emassey0135.orbit.ui.FormValues
import dev.emassey0135.orbit.ui.Item
import dev.emassey0135.orbit.ui.MenuItem
import dev.emassey0135.orbit.ui.OrbitUi
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class UiTest {
    /** What Orbit's reader was told, a line a call. */
    private class Told : Reader {
        val calls = mutableListOf<String>()
        override val characterCommitTimeout = 500
        override val characterCommitFeedback = 1
        override fun speak(text: String) { calls += "speak $text" }
        override fun speakQueued(text: String) { calls += "say $text" }
        override fun stop() { calls += "stop" }
        override fun sound(id: Int) { calls += "sound $id" }
        override fun vibrate(milliseconds: Long) { calls += "vibrate" }
        override fun focusList(name: String, count: Int, selected: Int, item: String) { calls += "list $name $count $selected $item" }
        override fun itemChanged(name: String, count: Int, selected: Int, item: String) { calls += "item $name $count $selected $item" }
        override fun dialog(name: String, message: String, focused: String) { calls += "dialog $name" }
        override fun dialogClosed() { calls += "dialog closed" }
        override fun menuOpened() { calls += "menu" }
        override fun focusMenu(item: String, subMenu: Boolean, accessKey: Char?, position: Int, total: Int) {
            calls += "menu item $item $position of $total" + (if (subMenu) " submenu" else "")
        }
        override fun contextMenuClosed(dismissed: Boolean) { calls += if (dismissed) "menu dismissed" else "menu chosen" }
        override fun focusEdit(name: String, content: String, cursor: Int, line: String, multiline: Boolean) { calls += "edit $name $content" }
        override fun focusControl(name: String, role: Int) { calls += "button $name" }
        override fun focusText(content: String) { calls += "text $content" }
        override fun focusCheck(name: String, checked: Boolean) { calls += "check $name $checked" }
        override fun focusChoice(name: String, role: Int, count: Int, selected: Int, item: String) { calls += "choice $name $item" }
        override fun characterTyped(character: String, fieldName: String, capital: Boolean, resolvedWord: String?) { calls += "typed $fieldName" }
        override fun editChanged(fieldName: String, content: String, cursor: Int, changeType: String) {}
    }

    private class Manual : T9Input.Later {
        override fun after(milliseconds: Long, run: () -> Unit) {}
        override fun cancel() {}
    }

    private val told = Told()

    /** Cells made into their letters, so typing needs no liblouis. */
    private val ui = OrbitUi(
        told, later = Manual(),
        translator = CellBuffer.Translator { cells, _ -> cells.joinToString("") { BrailleLetters.letter(it)?.toString().orEmpty() } },
    ).apply { readerReady() }

    private fun press(name: String) = ui.key(KeyEvent(name, KeyEvent.PRESS, false), false)
    private fun chord(name: String) = ui.key(KeyEvent(name, KeyEvent.CHORD, false), false)
    private fun release(name: String) = ui.key(KeyEvent(name, KeyEvent.RELEASE, false), false)
    private fun phone(name: String, action: Int = KeyEvent.RELEASE) = ui.key(KeyEvent(name, action, false), true)

    private fun down() = press("DOWN")
    private fun select() = press("OK")
    private fun escape() = chord("SPACE_DOT1_DOT5")
    private fun menuKey() = chord("SPACE_DOT1_DOT3_DOT4")
    private fun tab() = chord("SPACE_DOT4_DOT5")

    /** A letter typed as a cell alone. */
    private fun cell(letter: Char) = release(dotsName(letter))

    /** Space with a letter. */
    private fun spaceWith(letter: Char) = chord("SPACE_" + dotsName(letter))

    private fun dotsName(letter: Char): String {
        val dots = BrailleLetters.dots(letter)!!
        return (1..8).filter { dots and (1 shl (it - 1)) != 0 }.joinToString("_") { "DOT$it" }
    }

    private val fruit = mutableListOf("apple", "banana", "cherry")
    private val opened = mutableListOf<String>()
    private fun rows() = fruit.map { name -> Item(name, name, { opened += name }) }

    @Test fun aListIsToldAsAListBoxAndEachMoveAsItsItemChanging() {
        ui.list("Fruit", ::rows)
        down()
        select()
        assertEquals(listOf("list Fruit 3 0 apple", "item Fruit 3 1 banana"), told.calls)
        assertEquals(listOf("banana"), opened)
    }

    @Test fun nothingIsToldBeforeTheReaderIsReady() {
        val waiting = OrbitUi(told, later = Manual())
        waiting.list("Fruit", ::rows)
        assertEquals(emptyList<String>(), told.calls)
        waiting.readerReady()
        assertEquals(listOf("list Fruit 3 0 apple"), told.calls)
    }

    @Test fun anEmptyListSaysSoOnce() {
        ui.list("Fruit", { emptyList() })
        assertEquals(listOf("list Fruit 0 0 ", "say Empty"), told.calls)
    }

    @Test fun aRefreshKeepsTheRowInHandByItsKeyAndSaysNothing() {
        val list = ui.list("Fruit", ::rows)
        down()
        told.calls.clear()
        fruit.add(0, "apricot")
        ui.refresh()
        assertEquals(emptyList<String>(), told.calls)
        assertEquals("banana", list.current?.key)
        down()
        assertEquals(listOf("item Fruit 4 3 cherry"), told.calls)
    }

    @Test fun aRefreshThatTakesTheRowInHandAwayTellsTheRowNowInHand() {
        ui.list("Fruit", ::rows)
        down()
        told.calls.clear()
        fruit.remove("banana")
        ui.refresh()
        assertEquals(listOf("item Fruit 2 1 cherry"), told.calls)
    }

    @Test fun aRowThePersonRewordedIsReadAgainAtTheRefresh() {
        val done = mutableSetOf<String>()
        ui.list("Tasks", { fruit.map { Item(it, if (it in done) "$it, done" else it, { done += it }) } })
        select()
        told.calls.clear()
        ui.refresh()
        assertEquals(listOf("item Tasks 3 0 apple, done"), told.calls)
    }

    @Test fun aCellMovesToTheNextRowStartingWithItsLetter() {
        ui.list("Fruit", ::rows)
        told.calls.clear()
        cell('c')
        assertEquals(listOf("item Fruit 3 2 cherry"), told.calls)
        cell('z')
        assertEquals("sound ${OrbitReader.SOUND_ERROR}", told.calls.last())
    }

    @Test fun backClosesTheScreenAndTheOneUnderIsToldAgainThenTheLastClosingEndsTheApp() {
        var ended = false
        ui.onEmpty = { ended = true }
        ui.list("Fruit", ::rows)
        ui.list("Colours", { listOf(Item("red", "red")) })
        told.calls.clear()
        press("LEFT")
        assertEquals(listOf("list Fruit 3 0 apple"), told.calls)
        release("DOT7")
        assertTrue(ended)
    }

    @Test fun theMenuKeyOpensTheRowsActionsThenTheScreensOwnAndAnAccessKeyRunsOne() {
        val ran = mutableListOf<String>()
        ui.list(
            "Fruit",
            { fruit.map { name -> Item(name, name, actions = listOf(Action("Delete", 'd') { ran += "delete $name" })) } },
            actions = { listOf(Action("Add", 'a') { ran += "add" }) },
        )
        told.calls.clear()
        menuKey()
        assertEquals(listOf("menu", "menu item Delete 1 of 2"), told.calls)
        down()
        assertEquals("menu item Add 2 of 2", told.calls.last())
        told.calls.clear()
        cell('d')
        assertEquals(listOf("delete apple"), ran)
        assertEquals("a choice says nothing of the list again", listOf("menu chosen"), told.calls)
    }

    @Test fun aMenuDismissedTellsTheScreenUnderItAgain() {
        ui.list("Fruit", ::rows, actions = { listOf(Action("Add") {}) })
        told.calls.clear()
        menuKey()
        escape()
        assertEquals(listOf("menu", "menu item Add 1 of 1", "menu dismissed", "list Fruit 3 0 apple"), told.calls)
    }

    @Test fun theMenuKeyWithNothingToOfferSoundsTheErrorTone() {
        ui.list("Fruit", ::rows)
        told.calls.clear()
        menuKey()
        assertEquals(listOf("sound ${OrbitReader.SOUND_ERROR}"), told.calls)
    }

    @Test fun spaceWithAnActionsLetterRunsItFromTheList() {
        var added = false
        ui.list("Fruit", ::rows, actions = { listOf(Action("Add", 'a') { added = true }) })
        // A is Space with dot 1, Up, so it is pressed with Dot 8 instead.
        chord("DOT1_DOT8")
        assertTrue(added)
        spaceWith('x')
        assertEquals("sound ${OrbitReader.SOUND_ERROR}", told.calls.last())
    }

    @Test fun aMenuOfSubmenusGivesTheValueChosenToTheCoroutineWaitingForIt() {
        var chosen: String? = "nothing yet"
        ui.launch {
            chosen = ui.menu(
                listOf(
                    MenuItem("File", 'f', listOf(MenuItem("Open", 'o', value = "open"), MenuItem("Save", 's', value = "save"))),
                    MenuItem("Quit", 'q', value = "quit"),
                ),
            )
        }
        assertEquals(listOf("menu", "menu item File 1 of 2 submenu"), told.calls)
        press("RIGHT")
        down()
        select()
        assertEquals("save", chosen)
    }

    @Test fun aLineTypedInCellsIsGivenAsConfirmedWithDot8() {
        val answers = mutableListOf<String?>()
        ui.list("Fruit", ::rows)
        ui.askLine("Name") { answers += it }
        assertEquals("edit Name ", told.calls.last())
        cell('h')
        cell('i')
        told.calls.clear()
        release("DOT8")
        assertEquals(listOf<String?>("hi"), answers)
        assertEquals("list Fruit 3 0 apple", told.calls.last())
    }

    @Test fun aLineCancelledGivesNullOnce() {
        val answers = mutableListOf<String?>()
        ui.askLine("Name", "old") { answers += it }
        escape()
        assertEquals(listOf<String?>(null), answers)
    }

    @Test fun aLinesMenuIsOkAndCancel() {
        var answer: String? = null
        ui.askLine("Name", "kept") { answer = it }
        menuKey()
        assertEquals("menu item OK 1 of 2", told.calls.last())
        select()
        assertEquals("kept", answer)
    }

    @Test fun confirmIsADialogOfYesAndNoTheDefaultInHand() {
        val answers = mutableListOf<Boolean>()
        ui.confirm("Delete it?") { answers += it }
        assertEquals(listOf("dialog Delete it?", "button No"), told.calls)
        tab()
        assertEquals("button Yes", told.calls.last())
        select()
        assertEquals(listOf(true), answers)
        assertEquals("dialog closed", told.calls.last())
    }

    @Test fun aConfirmationClosedUnansweredIsNo() {
        var answer: Boolean? = null
        ui.confirm("Delete it?", default = true) { answer = it }
        escape()
        assertEquals(false, answer)
    }

    @Test fun chooseIsAListWhoseSelectPicksTheOptionInHand() {
        val answers = mutableListOf<Int?>()
        ui.choose("Colour", listOf("red", "green", "blue"), selected = 1) { answers += it }
        assertEquals(listOf("list Colour 3 1 green"), told.calls)
        down()
        select()
        ui.choose("Colour", listOf("red", "green", "blue")) { answers += it }
        press("LEFT")
        assertEquals(listOf(2, null), answers)
    }

    private val fields = listOf(
        Field.Line("name", "Name", "x"),
        Field.Choice("size", "Size", listOf("small", "large")),
        Field.Check("done", "Done"),
    )

    @Test fun aFormTabsBetweenItsControlsEachToldInItsRoleAndOkGivesWhatTheyHold() {
        var values: FormValues? = null
        ui.form("New", fields) { values = it }
        assertEquals(listOf("dialog New", "edit Name x"), told.calls)
        cell('a')
        tab()
        assertEquals("choice Size small", told.calls.last())
        down()
        assertEquals("item Size 2 1 large", told.calls.last())
        tab()
        assertEquals("check Done false", told.calls.last())
        select()
        assertEquals("check Done true", told.calls.last())
        tab()
        assertEquals("button OK", told.calls.last())
        select()
        val held = values!!
        assertEquals("xa", held.text("name"))
        assertEquals(1, held.choice("size"))
        assertEquals(true, held.checked("done"))
        assertEquals("dialog closed", told.calls.last())
    }

    @Test fun selectInAFormsLinePressesOk() {
        var values: FormValues? = null
        ui.form("New", fields) { values = it }
        select()
        assertEquals("x", values?.text("name"))
    }

    @Test fun aFormCancelledGivesNull() {
        var values: FormValues? = null
        var called = 0
        ui.form("New", fields) {
            values = it
            called++
        }
        tab()
        escape()
        assertNull(values)
        assertEquals(1, called)
    }

    @Test fun onThePlayerAFormsControlsAreRowsAndALineOpensAScreenOfItsOwn() {
        ui.phoneKeypad = true
        var values: FormValues? = null
        ui.form("New", fields) { values = it }
        assertEquals(listOf("dialog New", "item New 5 0 Name, x"), told.calls)
        // The third row, the check: a digit no longer number can follow opens it at once.
        phone("3")
        assertEquals("item New 5 2 Done, checked", told.calls.last())
        phone("1")
        assertEquals("edit Name x", told.calls.last())
        phone("2")
        phone("OK", KeyEvent.PRESS)
        assertEquals("item New 5 0 Name, xa", told.calls.last())
        phone("4")
        assertEquals("xa", values?.text("name"))
        assertEquals(true, values?.checked("done"))
    }

    @Test fun thePlayersBackPressClosesAMenuWithoutItsReleaseDeletingInTheLineUnder() {
        ui.phoneKeypad = true
        var answer: String? = "none yet"
        ui.askLine("Name", "ab") { answer = it }
        phone("MENU")
        phone("BACK", KeyEvent.PRESS)
        phone("BACK", KeyEvent.RELEASE)
        phone("OK", KeyEvent.PRESS)
        assertEquals("ab", answer)
    }

    @Test fun aKeyNothingTakesSoundsTheErrorTone() {
        ui.list("Fruit", ::rows)
        told.calls.clear()
        release("SPACE")
        assertEquals(listOf("sound ${OrbitReader.SOUND_ERROR}"), told.calls)
    }
}
