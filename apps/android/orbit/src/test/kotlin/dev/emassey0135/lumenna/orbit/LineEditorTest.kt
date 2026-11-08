package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

class LineEditorTest {
    private val output = Recorded()
    private var confirmedText: String? = null
    private var cancelled = false

    private fun editor(phone: Boolean, initial: String = "") = LineEditor(
        "Task", initial, phone, output,
        { cells, _ -> cells.joinToString("") { if (it == 0b10011) "h" else "i" } },
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

    @Test fun dot7AtTheStartIsAnError() {
        val editor = editor(phone = false)
        editor.key(Key.Dot7)
        assertEquals(listOf("error"), output.calls)
    }

    @Test fun spaceWithZCancelsOnTheSpeak() {
        editor(phone = false).key(Key.Chord(0b110101))
        assertEquals(true, cancelled)
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
        val editor = LineEditor(
            "Task", "", false, output, { _, _ -> "" },
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

    private class Recorded : LineEditor.Output {
        val calls = mutableListOf<String>()
        override fun focusEdit(name: String, content: String, cursor: Int) { calls += "focus" }
        override fun syncEdit(name: String, content: String, cursor: Int) { calls += "sync" }
        override fun typed(character: Char, resolvedWord: String?) { calls += "typed $character" }
        override fun typedDots(cell: Int) { calls += "dots" }
        override fun candidate(character: Char, field: String) { calls += "candidate" }
        override fun edited(name: String, content: String, cursor: Int, change: String) { calls += change }
        override fun playError() { calls += "error" }
        override fun commitFeedback(character: Char, mode: Int) { calls += "feedback" }
        override fun modeChanged(name: String, content: String, cursor: Int, mode: String) { calls += "mode" }
    }
}
