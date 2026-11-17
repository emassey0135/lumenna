package dev.emassey0135.orbit

/**
 * An edit box as Orbit's own are: the Speak's cell buffer through liblouis, or the Player's T9.
 *
 * The keys are those of Orbit's edit fields: on the Speak Select or dot 8 confirms, dot 7 is
 * backspace, Left and Right and Space with dots 3 and 6 move by a character, Space with 2 and 5 by a
 * word, Space with dots 1 3 and 4 6 to the line's start and end, Space with L and dots 4 5 6 to the
 * text's, Space with E or Z cancels. On the Player digits are multi-tap, a held digit types itself, star
 * the mode, Left and Right move, Up and Down go to the ends, Back deletes, held cancels, and OK
 * confirms. Changes are reported to Orbit as edit-field changes ([Output]), so it speaks them by
 * the person's own typing echo. Down offers completions where the line has them ([completing]).
 *
 * On the Speak the selection and clipboard are its Editor's: dots 1 2 8 start and end a selection,
 * dots 1 8 select all, dots 1 4 8 copy, 1 3 4 6 8 cut, 1 2 3 6 8 paste (each with Space too in
 * computer braille, where dot 8 with a cell types it), and Space with dots 1 2 7 reads it, over
 * the system clipboard.
 */
public class EditBox(
    /** What Orbit names the field. */
    public val name: String,
    initial: String,
    private val phoneKeypad: Boolean,
    private val output: Output,
    /** A field of several lines (the task's notes): dot 8 starts a new line, Up and Down move by line. */
    private val multiline: Boolean,
    translate: CellBuffer.Translator,
    later: T9Input.Later,
    private val timeoutMs: () -> Long,
    private val commitFeedback: () -> Int,
    private val changed: (text: String, caret: Int) -> Unit,
    private val confirmed: (String) -> Unit,
    private val cancelled: () -> Unit,
    /** Whether Down offers completions now; what opens them. */
    private val completing: (() -> Boolean)? = null,
    private val complete: () -> Unit = {},
) {
    /** What the reader is told. Calls go in the order they happen. */
    public interface Output {
        /** The field reached: Orbit says it, its content and where the caret is. */
        public fun focusEdit(name: String, content: String, cursor: Int, line: String, multiline: Boolean)
        /** The field's content and caret, told again without saying them. */
        public fun syncEdit(name: String, content: String, cursor: Int)
        /** The Speak's cell echo: the character it made, said by the person's character echo. */
        public fun typed(character: Char)
        /**
         * A word committed, as it reads in print: said if the person's echo says words, and whether
         * it was. The space after a word said is not echoed too, since the reader would stop the word for it.
         */
        public fun word(word: String): Boolean
        /** Whether the person types contracted braille now. */
        public val contracted: Boolean
        /** Whether the person types computer braille now, where dot 8 with a cell types it. */
        public val computer: Boolean
        /** Puts [text] on the system clipboard. */
        public fun copy(text: String)
        /** The system clipboard's text, or null for none. */
        public fun paste(): String?
        /** Said as it is, cutting off what was being said: Satellite's echo of dots and deletions. */
        public fun say(text: String)
        /** A T9 candidate offered. */
        public fun candidate(character: Char, field: String)
        /** The content changed by [change] (Orbit's change types), which Orbit says as its own fields do. */
        public fun edited(name: String, content: String, cursor: Int, change: String)
        /** Orbit's error tone. */
        public fun playError()
        /** Feedback on a commit timeout: sound, speech, vibration by the person's setting. */
        public fun commitFeedback(character: Char, mode: Int)
        /** T9's mode changed to [mode], said by Orbit with the field. */
        public fun modeChanged(name: String, content: String, cursor: Int, mode: String)
        /** The words the echo is said in: the device's own. */
        public val words: OrbitWords
    }

    private val buffer = CellBuffer(translate).apply { set(initial) }
    private val t9 = T9Input(T9Host(), later)
    private var open = true

    /** The text as shown, with a word in progress or a T9 candidate in place. */
    public fun display(): String {
        val pending = t9.candidate
        val text = buffer.display()
        if (pending == null) return text
        val at = t9.candidatePosition.coerceIn(0, text.length)
        return text.substring(0, at) + pending + text.substring(at)
    }

    /** Opens the field: Orbit says it and its text, and takes its copy to diff from. */
    public fun focus() {
        val text = buffer.display()
        output.focusEdit(name, text, buffer.caret, if (multiline) buffer.line() else text, multiline)
        output.syncEdit(name, text, buffer.caret)
    }

    /** Puts the line back as [text] after a completion chose it, the caret at [caret]. */
    public fun replace(text: String, caret: Int) {
        t9.cancel()
        buffer.set(text)
        repeat(text.length - caret) { buffer.moveCharacter(-1) }
        output.edited(name, text, caret, "completion_chosen")
        changed(text, caret)
    }

    /** The line as it stands, what is being typed taken in: for leaving it for another control. */
    public fun text(): String {
        t9.commitNow()
        return buffer.confirmed()
    }

    /** Finishes the line as its own Select does: for a Confirm button beside it. */
    public fun finish(): Unit = confirm()

    /** Handles a key; false if it is none of this field's. */
    public fun key(key: Key): Boolean = if (phoneKeypad) phoneKey(key) else brailleKey(key)

    private fun brailleKey(key: Key): Boolean {
        when (key) {
            is Key.Cell -> inputCell(key.dots)
            Key.Space -> space()
            Key.Dot7 -> backspace()
            Key.Dot8, Key.Select -> if (multiline) newLine() else confirm()
            Key.Left -> moveCharacter(-1)
            Key.Right -> moveCharacter(1)
            Key.Down -> when {
                completing?.invoke() == true -> complete()
                multiline -> moveLine(1)
                else -> moveEdge(false)
            }
            Key.Up -> if (multiline) moveLine(-1) else moveEdge(true)
            Key.First -> moveEnd(true)
            Key.Last -> moveEnd(false)
            Key.Escape -> cancel()
            is Key.Enter -> if (output.computer || !selection(key.dots)) inputCell(key.dots or DOT8)
            is Key.Chord -> when (key.dots) {
                DOT3 -> moveCharacter(-1)
                DOT6 -> moveCharacter(1)
                DOT2 -> moveWord(-1)
                DOT5 -> moveWord(1)
                DOTS13 -> if (multiline) moveLineEdge(true) else moveEnd(true)
                DOTS46 -> if (multiline) moveLineEdge(false) else moveEnd(false)
                DOTS127 -> readSelection()
                else -> if (!output.computer || (key.dots and DOT8) == 0 || !selection(key.dots and DOT8.inv())) return false
            }
            else -> return false
        }
        return true
    }

    private fun phoneKey(key: Key): Boolean {
        when (key) {
            is Key.Digit -> t9.tap(key.digit)
            is Key.DigitHeld -> {
                t9.commitNow()
                insertChar(('0' + key.digit), buffer.caret, false)
            }
            Key.Star -> t9.nextMode()
            Key.Pound -> t9.nextLayout()
            Key.Left -> {
                t9.commitNow()
                moveCharacter(-1)
            }
            Key.Right -> {
                t9.commitNow()
                moveCharacter(1)
            }
            Key.Up -> {
                t9.commitNow()
                moveEdge(true)
            }
            Key.First -> {
                t9.commitNow()
                moveEnd(true)
            }
            Key.Down -> {
                t9.commitNow()
                if (completing?.invoke() == true) complete() else moveEdge(false)
            }
            Key.Last -> {
                t9.commitNow()
                moveEnd(false)
            }
            is Key.BackReleased -> if (key.held) {
                cancel()
            } else {
                t9.commitNow()
                backspace()
            }
            Key.Select -> {
                t9.commitNow()
                confirm()
            }
            Key.SelectHeld -> {}
            else -> return false
        }
        return true
    }

    /**
     * As Orbit's own fields echo a cell: in contracted braille a cell they name (the capital sign)
     * by its name, else the last character its word makes so far; otherwise the character, else its
     * name; else its dots.
     */
    private fun inputCell(cell: Int) {
        val typed = buffer.type(cell)
        val called = output.words.cellNames.name(typed.cells, typed.at)
        when {
            called != null && (output.contracted || typed.live.isEmpty()) -> output.say(called)
            typed.live.isNotEmpty() -> output.typed(typed.live.last())
            else -> output.say(output.words.dots.replace("%s", CellBuffer.dotNumbers(cell)))
        }
        // Orbit's copy of the field follows silently, so its review commands read what is typed.
        output.syncEdit(name, buffer.display(), buffer.caret)
        changed(buffer.display(), buffer.caret)
    }

    /** The word committed in print, then said whole: never the characters its cells echoed. */
    private fun space() {
        val word = buffer.space()
        val after = buffer.display()
        if (word.isEmpty() || !output.word(word)) output.typed(' ')
        output.syncEdit(name, after, buffer.caret)
        changed(after, buffer.caret)
    }

    /** As Satellite's field: a cell off the word says what it alone makes, else its dots; a character says itself. */
    private fun backspace() {
        // As the Editor's: the selection goes, unasked, and the reader says so in its own words.
        if (buffer.hasSelection) {
            buffer.deleteSelection()
            output.edited(name, buffer.display(), buffer.caret, "selection_deleted")
            changed(buffer.display(), buffer.caret)
            return
        }
        val words = output.words
        val gone = when (val deleted = buffer.backspace()) {
            CellBuffer.Deleted.Nothing -> {
                output.say(words.blank)
                return
            }
            is CellBuffer.Deleted.Cell -> output.words.cellNames.name(listOf(deleted.cell), 0)
                ?: deleted.translated.ifEmpty { words.dots.replace("%s", CellBuffer.dotNumbers(deleted.cell)) }
            is CellBuffer.Deleted.Character -> when (deleted.character) {
                ' ' -> words.space
                '\n' -> words.newLine
                else -> deleted.character.toString()
            }
        }
        output.say(words.deleted.replace("%s", gone))
        // Orbit's copy follows silently, so its review commands read what is left.
        output.syncEdit(name, buffer.display(), buffer.caret)
        changed(buffer.display(), buffer.caret)
    }

    private fun newLine() {
        buffer.newLine()
        output.say(output.words.newLine)
        output.syncEdit(name, buffer.display(), buffer.caret)
        changed(buffer.display(), buffer.caret)
    }

    /** Up and Down in a field of several lines: Orbit says the line reached, as its Editor's do. */
    private fun moveLine(delta: Int) {
        val line = buffer.moveLine(delta)
        if (line == null) {
            output.playError()
            output.edited(name, buffer.display(), buffer.caret, if (delta < 0) "boundary_start" else "boundary_end")
            return
        }
        output.edited(name, line, buffer.caret, "line_navigation")
        output.syncEdit(name, buffer.display(), buffer.caret)
    }

    private fun moveLineEdge(start: Boolean) {
        val line = buffer.moveLineEdge(start)
        output.edited(name, line, buffer.caret, "line_navigation")
        output.syncEdit(name, buffer.display(), buffer.caret)
    }

    private fun moveCharacter(delta: Int) {
        val moved = buffer.moveCharacter(delta)
        if (moved == null) {
            output.playError()
            output.edited(name, buffer.display(), buffer.caret, if (delta < 0) "boundary_start" else "boundary_end")
            return
        }
        // Right speaks the character it passed, as Orbit's fields do: the cursor is reported before it.
        output.edited(name, buffer.display(), if (delta > 0) buffer.caret - 1 else buffer.caret, "cursor_moved")
    }

    private fun moveWord(delta: Int) {
        val word = buffer.moveWord(delta)
        if (word == null) {
            output.playError()
            output.edited(name, buffer.display(), buffer.caret, if (delta < 0) "boundary_start" else "boundary_end")
        } else {
            output.edited(name, buffer.display(), buffer.caret, "cursor_moved")
        }
    }

    /**
     * Up or Down in a field of one line, as Orbit's: the caret goes to the start or the end, and
     * there is no line beyond, so the error tone and the boundary are said whether it moved or not.
     */
    private fun moveEdge(start: Boolean) {
        buffer.moveEdge(start)
        output.playError()
        output.edited(name, buffer.display(), buffer.caret, if (start) "boundary_start" else "boundary_end")
    }

    /** To the start or the end of the text, asked for by name (Space with L): a move, no tone. */
    private fun moveEnd(start: Boolean) {
        buffer.moveEdge(start)
        output.edited(name, buffer.display(), buffer.caret, "cursor_moved")
    }

    /** The Editor's selection and clipboard commands, by the dots pressed with dot 8; false for none. */
    private fun selection(dots: Int): Boolean {
        val words = output.words
        when (dots) {
            DOT1 -> {
                buffer.selectAll()
                output.say(words.allSelected)
            }
            DOTS12 -> output.say(if (buffer.toggleSelection()) words.selectionStarted else words.selectionFinished)
            DOTS14 -> {
                val selected = buffer.selected() ?: return true.also { output.say(words.nothingSelected) }
                output.copy(selected)
                output.say(words.copied)
            }
            DOTS1346 -> {
                val selected = buffer.selected() ?: return true.also { output.say(words.nothingSelected) }
                output.copy(selected)
                buffer.deleteSelection()
                output.say(words.cut)
                edited()
            }
            DOTS1236 -> {
                val pasted = output.paste()?.let { if (multiline) it else it.replace(Regex("\\s*\\R\\s*"), " ") }
                if (pasted.isNullOrEmpty()) {
                    output.playError()
                    return true
                }
                buffer.paste(pasted)
                output.say(words.pasted)
                edited()
            }
            else -> return false
        }
        return true
    }

    private fun readSelection() = output.say(buffer.selected() ?: output.words.noTextSelected)

    /** Orbit's copy and the app follow a change the keys made. */
    private fun edited() {
        output.syncEdit(name, buffer.display(), buffer.caret)
        changed(buffer.display(), buffer.caret)
    }

    private fun insertChar(character: Char, position: Int, feedback: Boolean) {
        buffer.insert(character, position)
        output.edited(name, buffer.display(), buffer.caret, "character_inserted")
        changed(buffer.display(), buffer.caret)
        if (feedback) output.commitFeedback(character, commitFeedback())
    }

    private fun confirm() {
        t9.commitNow()
        close()
        confirmed(buffer.confirmed())
    }

    private fun cancel() {
        close()
        cancelled()
    }

    private fun close() {
        t9.cancel()
        open = false
    }

    private inner class T9Host : T9Input.Host {
        override fun caret(): Int = buffer.caret
        override fun timeoutMs(): Long = this@EditBox.timeoutMs()
        override fun candidate(character: Char) = output.candidate(character, name)
        override fun commit(character: Char, position: Int, feedback: Boolean) {
            if (open && position <= buffer.display().length) insertChar(character, position, feedback)
        }
        override fun modeChanged(mode: T9Input.Mode) =
            output.modeChanged(this@EditBox.name, buffer.display(), buffer.caret, output.words.mode(mode))
    }

    private companion object {
        const val DOT1 = 1
        const val DOTS12 = 0b11
        const val DOTS14 = 0b1001
        const val DOTS1346 = 0b101101
        const val DOTS1236 = 0b100111
        const val DOTS127 = 0b1000011
        const val DOT2 = 1 shl 1
        const val DOT3 = 1 shl 2
        const val DOT5 = 1 shl 4
        const val DOT6 = 1 shl 5
        const val DOT8 = 1 shl 7
        const val DOTS13 = 1 or (1 shl 2)
        const val DOTS46 = (1 shl 3) or (1 shl 5)
    }
}
