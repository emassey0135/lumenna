package dev.emassey0135.braillesense.edit

import dev.emassey0135.braillesense.braille.Echo
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.input.Chords
import dev.emassey0135.braillesense.input.Key

/** Text the person copies and pastes: the system's clipboard ([SystemClipboard]), or an app's own. */
public interface Clipboard {
    /** What it holds as text; null for nothing. */
    public fun text(): String?

    /** Puts [text] on it, or empties it with null. */
    public fun set(text: String?)
}

/** What the display shows of a box, which Backspace-1-4 and Enter-U go by. */
public interface EditDisplay {
    /** How many cells the display has. */
    public val width: Int

    /**
     * The cells of the cursor's line on the display line that holds the cursor, as the device's
     * Backspace-1-4 takes them, with the box's line shown as it is now; null where it is not known.
     */
    public fun cursorLine(): IntRange?
}

/** What a key did in a box, for the app to draw and say. */
public sealed interface Edited {
    /** Not the box's: Tab, Enter in a box of one line, a command of the app's. */
    public data object NotTaken : Edited

    /** The box took it. [changed]: its text is not what it was. [said]: what the device says, if anything. */
    public data class Taken(val changed: Boolean = false, val said: String? = null) : Edited

    /** The device asks first ([question], Yes or No); the app asks it its own way, then calls [answer]. */
    public class Ask(public val question: String, public val answer: (yes: Boolean) -> Taken) : Edited
}

/**
 * The keys of an edit box, as the BrailleSense's own (its `BrailleEdit`, `BrailleMove` and
 * `BrailleRead`), and what it says for each: typing in cells with the person's Keyboard Echo,
 * moving by character, word and line, reading at the cursor, the selection (Enter-B, Enter-A) and
 * the clipboard (Enter-C, X, P, V, D, Backspace-1-2, Enter-U), and deleting (Space-D,
 * Backspace-2-5, Backspace-1-4, Backspace-1-3-4-6). Control with a letter on the QWERTY keyboard is
 * Enter with it. Tab, Enter in a single-line box and routing are the app's.
 */
public class BoxEditing(
    private val translator: Translator,
    private val clipboard: Clipboard,
    /** The person's Keyboard Echo and the grade they type, read at each key. */
    private val echo: () -> Echo,
    private val words: EditWords = EditWords.ENGLISH,
) {
    /** What [key] does to [box], as the device's own edit box takes it: [Edited.NotTaken] for a key that is not the box's, which the frame then takes. */
    public fun press(box: TextBox, key: Key, display: EditDisplay): Edited {
        command(box, key, display)?.let { return it }
        return keys(box, key)
    }

    /** The edit box's commands, ahead of its keys. Null for what is none. */
    private fun command(box: TextBox, key: Key, display: EditDisplay): Edited? {
        val letter = when (key) {
            is Key.EnterDots -> Chords.letter(key.dots)
            is Key.EnterLetter -> key.letter.lowercaseChar()
            else -> null
        }
        fun say(said: String?) = Edited.Taken(said = said)
        fun changed(said: String?) = Edited.Taken(changed = true, said = said)
        return when {
            letter == 'b' -> say(words.startSelection).also { box.startSelection() }
            letter == 'a' -> say(words.selectAll).also { box.selectAll() }
            letter == 'c' || letter == 'x' || letter == 'p' -> {
                val selected = box.selected() ?: return say(words.nothingSelected)
                when (letter) {
                    'p' -> clipboard.set(listOfNotNull(clipboard.text()?.takeIf { it.isNotEmpty() }, selected).joinToString("\n"))
                    else -> clipboard.set(selected)
                }
                when (letter) {
                    'x' -> changed(words.cut).also { box.deleteSelected() }
                    'c' -> say(words.copied).also { box.cancelSelection() }
                    else -> say(words.appended).also { box.cancelSelection() }
                }
            }
            letter == 'v' -> {
                val pasted = clipboard.text()?.takeIf { it.isNotEmpty() } ?: return say(words.clipboardEmpty)
                box.insert(pasted)
                changed(words.pasted)
            }
            letter == 'd' -> say(words.clipboardCleared).also { clipboard.set(null) }
            letter == 'u' -> say(box.selectionStart(display.width) ?: words.nothingSelected)
            key is Key.BackspaceDots && key.dots == SELECTION_READ -> say(box.selected() ?: words.nothingSelected)
            key == Key.DeleteWord -> when (val gone = box.deleteWord()) {
                null -> say(null)
                is TextBox.WordRemoved.Word -> changed(gone.text)
                TextBox.WordRemoved.Space -> changed(words.space)
                TextBox.WordRemoved.LineBreak -> changed(words.newParagraph)
            }
            key == Key.DeleteLine -> {
                val shown = display.cursorLine() ?: (0 until box.line.size)
                if (box.deleteLine(shown.first, shown.last + 1)) changed(words.deleted) else say(null)
            }
            key == Key.DeleteParagraph -> if (box.deleteParagraph()) changed(words.deleted) else say(null)
            !box.selecting -> null
            key == Key.Backspace || key == Key.Delete -> Edited.Ask(words.deleteSelected) { yes ->
                if (yes) {
                    box.deleteSelected()
                    changed(words.deleted)
                } else {
                    say(line(box))
                }
            }
            key == Key.Escape -> say(words.selectionCanceled).also { box.cancelSelection() }
            key is Key.Cell || key is Key.Typed || key == Key.Space || key == Key.Enter -> {
                // Typing ends a selection, and goes on to be typed.
                box.cancelSelection()
                null
            }
            else -> null
        }
    }

    /** Typing, moving and reading. */
    private fun keys(box: TextBox, key: Key): Edited {
        val editor = box.line
        val setting = echo()
        val from = box.row to editor.caret
        fun moved() = from != (box.row to box.line.caret)
        return when (key) {
            is Key.Cell -> {
                box.line.cell(key.dots)
                // A cell's word is read as text only once it is finished, so nothing is told yet.
                Edited.Taken(said = if (setting.characters) box.line.cellBeforeCaret()?.let(::cell) else null)
            }
            is Key.Typed -> {
                box.line.type(key.text)
                Edited.Taken(changed = true, said = key.text.takeIf { setting.characters })
            }
            Key.Space -> {
                box.line.space()
                val word = box.line.takeFinished()
                Edited.Taken(changed = true, said = when {
                    word != null && setting.words -> word
                    setting.space -> words.space
                    else -> null
                })
            }
            Key.Backspace -> {
                // What Backspace takes is said whatever the echo, as the device's own box says it.
                val said = if (box.backspace()) words.previousLine.also { box.takeJoined() } else box.line.takeRemoved()?.let(::cell)
                Edited.Taken(changed = true, said = said)
            }
            // Space-D: the character it took, as Backspace's; a line break taken says nothing, as
            // the device's own says nothing for it.
            Key.Delete -> {
                box.delete()
                Edited.Taken(changed = true, said = box.line.takeRemoved()?.let(::cell))
            }
            Key.Enter -> if (box.multiLine) {
                box.newLine()
                // As the device's multi-line box says it: the word it finished where the echo says
                // words, else "Enter".
                val word = box.takeFinished()
                Edited.Taken(changed = true, said = when {
                    word != null && setting.words -> word
                    setting.space -> words.enter
                    else -> null
                })
            } else {
                Edited.NotTaken
            }
            Key.Up, Key.Down -> if (box.multiLine) {
                if (key == Key.Up) box.up() else box.down()
                Edited.Taken(said = line(box))
            } else {
                Edited.NotTaken
            }
            // Home and End say the word they land in, as the device's box does, and nothing where
            // the cursor was there already; Top and Bottom say the same, where the device's box
            // reads the whole first or last line, so that the four say one thing.
            Key.First, Key.Last, Key.LineStart, Key.LineEnd -> {
                when (key) {
                    Key.First -> if (box.multiLine) box.top() else editor.start()
                    Key.Last -> if (box.multiLine) box.bottom() else editor.end()
                    Key.LineStart -> editor.start()
                    else -> editor.end()
                }
                Edited.Taken(said = if (moved()) word(box) else null)
            }
            Key.CharLeft, Key.CharRight -> {
                if (key == Key.CharLeft) editor.left() else editor.right()
                Edited.Taken(said = editor.cellAtCaret()?.let(::cell))
            }
            Key.WordLeft, Key.WordRight -> {
                if (key == Key.WordLeft) editor.wordLeft() else editor.wordRight()
                Edited.Taken(said = word(box))
            }
            // At the end of the line the cursor is past the last character: that one is said.
            Key.SayCharacter -> Edited.Taken(said = (editor.cellAtCaret() ?: editor.cellBeforeCaret())?.let(::cell))
            Key.SayWord -> Edited.Taken(said = word(box))
            // In a multi-line box, the line the cursor is on, as the device's own reads it; a box of
            // one line is read whole, with its label, by the app.
            Key.Repeat -> if (box.multiLine) Edited.Taken(said = line(box)) else Edited.NotTaken
            else -> Edited.NotTaken
        }
    }

    /** The line the cursor is on, or "blank". */
    private fun line(box: TextBox): String = box.line.text.ifBlank { words.blank }

    /** The word at the cursor, as Space-2-5 says it. */
    private fun word(box: TextBox): String? = box.line.wordAtCaret().takeIf { it.isNotBlank() }

    /**
     * A cell by itself, as the device's own edit boxes say it: in computer braille the character
     * it is, else its name (its letters, never the whole word it can stand for, and a cell with no
     * letters by its dots).
     */
    private fun cell(cell: CellInWord): String? {
        if (cell.cell == LineEditor.BLANK) return words.space
        val grade = echo().grade
        val said = if (grade == 0) translator.back(byteArrayOf(cell.cell))?.let(CellEcho::character) else CellEcho.cell(cell.word, cell.at, grade == 2)
        return said?.takeIf { it.isNotBlank() }
    }

    private companion object {
        /** Backspace-1-2, the device's "read the selection". */
        const val SELECTION_READ = 0x03
    }
}
