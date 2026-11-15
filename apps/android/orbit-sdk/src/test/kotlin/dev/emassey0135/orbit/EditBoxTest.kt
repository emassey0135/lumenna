package dev.emassey0135.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

class EditBoxTest {
    private val output = Recorded()
    private var confirmedText: String? = null
    private var cancelled = false

    private fun editor(phone: Boolean, initial: String = "", multiline: Boolean = false) = EditBox(
        "Task", initial, phone, output, multiline,
        { cells, _ -> cells.joinToString("") { if (it == 0b10011) "h" else if (it == CAPITAL) "" else "i" } },
        object : T9Input.Later {
            override fun after(milliseconds: Long, run: () -> Unit) {}
            override fun cancel() {}
        },
        timeoutMs = { 500 }, commitFeedback = { 1 },
        changed = { _, _ -> },
        confirmed = { confirmedText = it },
        cancelled = { cancelled = true },
    )

    @Test fun theSpeakTypesACellAndConfirmsWithDot8() {
        val editor = editor(phone = false)
        editor.key(Key.Cell(0b10011))
        editor.key(Key.Dot8)
        assertEquals("h", confirmedText)
        assertEquals(listOf("typed h"), output.calls.filter { it.startsWith("typed") })
    }

    @Test fun dot7AtTheStartSaysBlank() {
        val editor = editor(phone = false)
        editor.key(Key.Dot7)
        assertEquals(listOf("say blank"), output.calls)
    }

    @Test fun aSignOrbitNamesSaysItsName() {
        editor(phone = false).key(Key.Cell(CAPITAL))
        assertEquals("say capital sign", output.calls.first())
    }

    @Test fun aCellThatMakesNothingAndHasNoNameSaysDotAndItsNumbers() {
        val editor = EditBox(
            "Task", "", false, output, false, { _, _ -> "" },
            object : T9Input.Later {
                override fun after(milliseconds: Long, run: () -> Unit) {}
                override fun cancel() {}
            },
            timeoutMs = { 500 }, commitFeedback = { 1 }, changed = { _, _ -> }, confirmed = {}, cancelled = {},
        )
        editor.key(Key.Cell(0b110000))
        assertEquals("say dot 5 6", output.calls.first())
    }

    @Test fun spaceSaysTheWordAsCommittedNotTheCharactersItsCellsEchoed() {
        val editor = EditBox(
            "Task", "", false, output, false,
            { cells, mode -> if (mode == CellBuffer.MODE_COMMIT) "knowledge " else "k" },
            object : T9Input.Later {
                override fun after(milliseconds: Long, run: () -> Unit) {}
                override fun cancel() {}
            },
            timeoutMs = { 500 }, commitFeedback = { 1 }, changed = { _, _ -> }, confirmed = {}, cancelled = {},
        )
        editor.key(Key.Cell(0b101))
        editor.key(Key.Space)
        assertEquals(listOf("typed k", "word knowledge"), output.calls.filter { it.startsWith("typed") || it.startsWith("word") })
        assertEquals("knowledge ", editor.text())
    }

    @Test fun dot8ChordsSelectCopyAndPasteOverTheClipboard() {
        val editor = editor(phone = false, initial = "hi")
        editor.key(Key.Enter(0b1001))
        assertEquals("say nothing", output.calls.last())
        editor.key(Key.Enter(0b1))
        editor.key(Key.Enter(0b1001))
        assertEquals("hi", output.clipboard)
        assertEquals("say copied", output.calls.last())
        editor.key(Key.Enter(0b101101))
        assertEquals("", editor.text())
        editor.key(Key.Enter(0b100111))
        editor.key(Key.Enter(0b100111))
        assertEquals("hihi", editor.text())
        assertEquals("say pasted", output.calls.filter { it.startsWith("say") }.last())
    }

    @Test fun dot7WithASelectionDeletesItAndTheReaderSaysSo() {
        val editor = editor(phone = false, initial = "hi there")
        editor.key(Key.Enter(0b1))
        output.calls.clear()
        editor.key(Key.Dot7)
        assertEquals("", editor.text())
        assertEquals("selection_deleted", output.calls.first())
        output.calls.clear()
        editor.key(Key.Dot7)
        assertEquals("say blank", output.calls.first())
    }

    @Test fun inComputerBrailleTheSameCommandsTakeSpaceAndDot8WithACellTypesIt() {
        output.computer = true
        val editor = editor(phone = false, initial = "hi")
        editor.key(Key.Chord(0b10000001))
        assertEquals("say all", output.calls.last())
        editor.key(Key.Enter(0b1))
        assertEquals("hii", editor.text())
    }

    @Test fun deletingACellOfAWordNotYetCommittedSaysThatCell() {
        val editor = editor(phone = false)
        editor.key(Key.Cell(0b10011))
        editor.key(Key.Cell(0b01010))
        output.calls.clear()
        editor.key(Key.Dot7)
        assertEquals("say i deleted", output.calls.first())
        editor.key(Key.Cell(CAPITAL))
        output.calls.clear()
        editor.key(Key.Dot7)
        assertEquals("say capital sign deleted", output.calls.first())
    }

    @Test fun deletingOnceTheWordIsCommittedSaysTheCharacter() {
        val editor = editor(phone = false, initial = "ab ")
        editor.key(Key.Dot7)
        assertEquals("say space deleted", output.calls.first())
        editor.key(Key.Dot7)
        assertEquals("say b deleted", output.calls[2])
    }

    @Test fun inAFieldOfSeveralLinesDot8StartsALineAndUpReadsTheOneBefore() {
        val editor = editor(phone = false, initial = "first", multiline = true)
        editor.key(Key.Dot8)
        assertEquals(null, confirmedText)
        assertEquals("say new line", output.calls.first())
        editor.key(Key.Cell(0b10011))
        output.calls.clear()
        editor.key(Key.Up)
        assertEquals("edited first line_navigation", output.calls.first())
        assertEquals("first\nh", editor.text())
    }

    @Test fun spaceWithZCancelsOnTheSpeak() {
        editor(phone = false).key(Key.Escape)
        assertEquals(true, cancelled)
    }

    @Test fun spaceWithAllSixDotsIsNoneOfTheFieldsAsInOrbitsOwn() {
        val editor = editor(phone = false, initial = "hi")
        assertEquals(false, editor.key(Key.Chord(0x3f)))
        assertEquals(false, cancelled)
    }

    @Test fun upInALineOfOneSoundsTheEdgeAsOrbitsDoesButSpaceWithLOnlyMoves() {
        val editor = editor(phone = false, initial = "hi")
        editor.key(Key.Up)
        assertEquals(listOf("error", "boundary_start"), output.calls)
        output.calls.clear()
        editor.key(Key.Last)
        assertEquals(listOf("cursor_moved"), output.calls)
        output.calls.clear()
        editor.key(Key.Chord(0b101))
        assertEquals("Space with dots 1 3 is the line's start", listOf("cursor_moved"), output.calls)
    }

    @Test fun starNamesTheModeInTheDevicesWords() {
        val editor = editor(phone = true)
        editor.key(Key.Star)
        assertEquals("mode uppercase", output.calls.last())
    }

    @Test fun thePlayerTapsDigitsAndConfirmsWithOk() {
        val editor = editor(phone = true)
        editor.key(Key.Digit(2))
        editor.key(Key.Digit(2))
        editor.key(Key.Select)
        assertEquals("b", confirmedText)
    }

    @Test fun aHeldDigitTypesItself() {
        val editor = editor(phone = true)
        editor.key(Key.DigitHeld(7))
        assertEquals("7", editor.display())
    }

    @Test fun thePlayersBackDeletesAndHeldCancels() {
        val editor = editor(phone = true, initial = "ab")
        editor.key(Key.BackReleased(held = false))
        assertEquals("a", editor.display())
        editor.key(Key.BackReleased(held = true))
        assertEquals(true, cancelled)
    }

    @Test fun downOffersCompletionsOnlyWhereThereAreAny() {
        var offered = 0
        val editor = EditBox(
            "Task", "", false, output, false, { _, _ -> "" },
            object : T9Input.Later {
                override fun after(milliseconds: Long, run: () -> Unit) {}
                override fun cancel() {}
            },
            timeoutMs = { 500 }, commitFeedback = { 1 }, changed = { _, _ -> }, confirmed = {}, cancelled = {},
            completing = { true }, complete = { offered++ },
        )
        editor.key(Key.Down)
        assertEquals(1, offered)
    }

    private class Recorded : EditBox.Output {
        val calls = mutableListOf<String>()
        override fun focusEdit(name: String, content: String, cursor: Int, line: String, multiline: Boolean) { calls += "focus" }
        override fun syncEdit(name: String, content: String, cursor: Int) { calls += "sync" }
        override fun typed(character: Char) { calls += "typed $character" }
        override fun word(word: String): Boolean {
            calls += "word $word"
            return true
        }
        override var contracted = true
        override var computer = false
        var clipboard: String? = null
        override fun copy(text: String) { clipboard = text }
        override fun paste(): String? = clipboard
        override fun say(text: String) { calls += "say $text" }
        override val words = OrbitWords(
            selectionStarted = "started", selectionFinished = "finished", allSelected = "all",
            nothingSelected = "nothing", noTextSelected = "none", copied = "copied", cut = "cut", pasted = "pasted",
        )
        override fun candidate(character: Char, field: String) { calls += "candidate" }
        override fun edited(name: String, content: String, cursor: Int, change: String) {
            calls += if (change == "line_navigation") "edited $content $change" else change
        }
        override fun playError() { calls += "error" }
        override fun commitFeedback(character: Char, mode: Int) { calls += "feedback" }
        override fun modeChanged(name: String, content: String, cursor: Int, mode: String) { calls += "mode $mode" }
    }

    private companion object {
        /** A capital sign: no character of its own. */
        const val CAPITAL = 0b100000
    }
}
