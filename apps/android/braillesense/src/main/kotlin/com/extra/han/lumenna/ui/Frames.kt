package com.extra.han.lumenna.ui

import com.extra.han.lumenna.device.Key
import dev.emassey0135.lumenna.app.BraillePart
import dev.emassey0135.lumenna.app.ControlRole

/** What a frame shows and says now. */
data class Rendered(
    val speech: String,
    val braille: String = speech,
    /** Where the display's cursor belongs, in [braille]'s text: -1 for none. */
    val cursor: Int = -1,
    /** Cells ready made, for a line being typed in the person's own contractions. */
    val cells: ByteArray? = null,
    val cellCursor: Int = -1,
    /** What goes on the cells ahead of [braille]: the role, a label. */
    val before: List<BraillePart> = emptyList(),
    /** What follows it: a shortcut letter, a sign, the role. */
    val after: List<BraillePart> = emptyList(),
)

/**
 * One thing on the display, taking the keys that belong to it: a list, a menu, a line being
 * typed, a dialog's controls. The shell keeps what is common to them all (panning, routing,
 * Escape's way back, the menus' opening); a frame returns whether it took the key.
 */
abstract class Frame(
    /** What it is, so the shell can tell a new frame from the same one drawn again. */
    val id: String,
) {
    abstract fun key(key: Key): Boolean

    abstract val rendered: Rendered

    /** A routing key over a character of [Rendered.braille] (-1 past its end). */
    open fun routed(at: Int) {}

}

/** One row of a list. */
class Row(
    val key: String,
    val speech: String,
    val braille: String = speech,
    /** The letter it is found by: its first, in its title. */
    val finding: String = speech,
    val open: (() -> Unit)? = null,
    /** Space on the row: a check box's change. */
    val space: (() -> Unit)? = null,
    /** What its menu of actions offers. */
    val actions: List<MenuEntry> = emptyList(),
    /** Space or Enter with a letter, on a list the app does not hold: its command. True if taken. */
    val command: ((Char) -> Boolean)? = null,
    /** Delete on the row. */
    val delete: (() -> Unit)? = null,
    /** What the device calls it. */
    val role: ControlRole = ControlRole.LIST_ITEM,
    /** What follows its letter, said and shown; empty for none. */
    val detail: String = "",
    /** The letter that runs it, shown as the device shows a shortcut key. */
    val letter: Char? = null,
    /** Space-3 and Space-6: its folding. */
    val collapse: (() -> Unit)? = null,
    val expand: (() -> Unit)? = null,
)

/**
 * A list, moved through with Space-1 and Space-4 (or the arrows), first letters and routing:
 * the row in hand survives a redraw because the shell finds it again by its key.
 */
class ListFrame(
    id: String,
    val title: String,
    private val rows: List<Row>,
    selected: String?,
    private val empty: String,
    private val letters: Letters,
    private val moved: (Row) -> Unit = {},
    /** Opens the menu of [Row.actions]. */
    private val openActions: (Row) -> Unit = {},
    /** The app's own first-letter search of its main list; null for a list searched here. */
    private val findInApp: ((Char) -> Unit)? = null,
    /** Says that no row starts with the letter. */
    private val notFound: (Char) -> Unit = {},
    /** The row's letters and first letters are the app's: what is pressed goes to it. */
    val appHeld: Boolean = false,
    private val readings: Readings,
    /** A letter that is a shortcut here, tried before first-letter moving; true if taken. */
    private val byLetter: ((Char) -> Boolean)? = null,
    /** A question's answers: Space and Backspace choose among them, as the device's prompts do. */
    private val prompt: Boolean = false,
) : Frame(id) {
    private var index = rows.indexOfFirst { it.key == selected }.takeIf { it >= 0 } ?: 0

    val current: Row? get() = rows.getOrNull(index)

    /** The current row's place, as a shell announces it. */
    val position: Pair<Int, Int> get() = (index + 1) to rows.size

    override val rendered: Rendered
        get() = current?.let { row ->
            // A prompt's answers are a choice in place, numbered no more than the device numbers them.
            val (place, count) = if (prompt) 0 to 0 else position
            readings.read(line(row.role, row.speech, row.braille, detail = row.detail, letter = row.letter, position = place, count = count)).rendered()
        } ?: Rendered(empty)

    private fun moveTo(target: Int): Boolean {
        if (rows.isEmpty()) return true
        val clamped = target.coerceIn(rows.indices)
        if (clamped != index) {
            index = clamped
            current?.let(moved)
        }
        return true
    }

    override fun routed(at: Int) {
        // A list's lines are single rows; routing presses the row.
        current?.open?.invoke()
    }

    override fun key(key: Key): Boolean = when (key) {
        Key.Up -> moveTo(index - 1)
        Key.Down -> moveTo(index + 1)
        Key.First -> moveTo(0)
        Key.Last -> moveTo(rows.lastIndex)
        Key.PageUp -> moveTo(index - GROUP)
        Key.PageDown -> moveTo(index + GROUP)
        Key.Enter -> run { current?.open?.invoke(); current != null }
        Key.Space -> if (prompt) wrap(+1) else run { current?.space?.invoke(); current?.space != null }
        Key.Backspace -> prompt && wrap(-1)
        Key.CharLeft -> run { current?.collapse?.invoke(); current?.collapse != null }
        Key.CharRight -> run { current?.expand?.invoke(); current?.expand != null }
        Key.Context -> run {
            current?.takeIf { it.actions.isNotEmpty() }?.let(openActions)
            true
        }
        Key.Delete -> run { current?.delete?.invoke(); current?.delete != null }
        is Key.Cell, is Key.Typed -> run {
            letters.letter(key)?.let { letter -> if (byLetter?.invoke(letter) != true) find(letter) }
            true
        }
        else -> false
    }

    private fun wrap(step: Int): Boolean = moveTo((index + step + rows.size) % rows.size.coerceAtLeast(1))

    /** First-letter search: the app's for its main list, else the core's over these rows. */
    private fun find(letter: Char) {
        findInApp?.let { return it(letter) }
        if (rows.isEmpty()) return
        val at = letters.find(rows.map { it.finding }, index, letter)
        if (at != null) moveTo(at) else notFound(letter)
    }

    companion object {
        /** How far Space-1-2-6 and Space-3-4-5 go in a list: "32/40-item group" on this device. */
        private const val GROUP = 32
    }
}

/** A line of text the person types, with its prompt, and what Enter does with it. */
class LineFrame(
    id: String,
    private val prompt: String,
    val editor: LineEditor,
    private val hint: String = "",
    private val readings: Readings,
    private val done: (String) -> Unit,
    private val changed: (String) -> Unit = {},
) : Frame(id) {
    private fun push(): Boolean {
        changed(editor.text)
        return true
    }

    override val rendered: Rendered
        get() {
            val shown = editor.shown()
            return readings.read(line(ControlRole.EDIT_BOX, editor.text.ifEmpty { hint }, editor.text, label = prompt)).rendered(shown.cells, shown.cursor)
        }

    override fun routed(at: Int) {
        editor.route(at)
    }

    override fun key(key: Key): Boolean = when (key) {
        Key.Enter -> run { editor.commit(); done(editor.text); true }
        Key.CharLeft -> run { editor.left(); push() }
        Key.CharRight -> run { editor.right(); push() }
        Key.WordLeft -> run { editor.wordLeft(); push() }
        Key.WordRight -> run { editor.wordRight(); push() }
        Key.LineStart, Key.First -> run { editor.start(); push() }
        Key.LineEnd, Key.Last -> run { editor.end(); push() }
        Key.Backspace -> run { editor.backspace(); push() }
        Key.Delete -> run { editor.delete(); push() }
        Key.Space -> run { editor.space(); push() }
        is Key.Cell -> run { editor.cell(key.dots); true }
        is Key.Typed -> run { editor.type(key.text); push() }
        else -> false
    }
}

/** A control in a dialog: something Tab stops at. */
sealed class Control(val key: String) {
    /** The line it reads as. */
    abstract val line: dev.emassey0135.lumenna.app.DeviceLine

    /** Text to type; [editor] holds it while the dialog is open. */
    class Edit(
        key: String,
        val label: String,
        val editor: LineEditor,
        val hint: String = "",
        val changed: (String) -> Unit = {},
        val submit: (() -> Unit)? = null,
    ) : Control(key) {
        override val line get() = line(ControlRole.EDIT_BOX, editor.text.ifEmpty { hint }, editor.text, label = label)
    }

    /** One of several options, changed with Space, Backspace, Space-1 and Space-4 or by first letter. */
    class Choice(
        key: String,
        val label: String,
        val options: List<Pair<String, String>>,
        val selected: Int,
        val choose: (String) -> Unit,
        val press: (() -> Unit)? = null,
    ) : Control(key) {
        private val value get() = options.getOrNull(selected)?.second.orEmpty()
        override val line get() = line(ControlRole.COMBO_BOX, value, label = label, position = selected + 1, count = options.size)
    }

    /** A flag, changed with Space or Backspace. */
    class Toggle(key: String, val label: String, val on: Boolean, val change: (Boolean) -> Unit) : Control(key) {
        override val line get() = line(if (on) ControlRole.CHECK_BOX_CHECKED else ControlRole.CHECK_BOX_UNCHECKED, label)
    }

    /** Enter presses it, or its [letter] with Space or Enter. */
    class Button(key: String, val label: String, val letter: Char? = null, val asks: Boolean = false, val press: () -> Unit) : Control(key) {
        override val line get() = line(ControlRole.BUTTON, label, letter = letter, asks = asks)
    }

    /** A line of information: a refusal, a caption. */
    class Static(key: String, val text: String) : Control(key) {
        override val line get() = line(ControlRole.STATIC_BOX, text)
    }
}

/**
 * A dialog's controls, in order. Tab and Shift-Tab are all that move between them, as in the
 * device's own dialogs; Space-1 and Space-4 move within the control that is a list of choices, and
 * do nothing on one that is not.
 */
class FormFrame(
    id: String,
    /** What it is called, said when it opens; null for what has no title. */
    val title: String?,
    private val controls: List<Control>,
    selected: String?,
    private val letters: Letters,
    private val readings: Readings,
    private val moved: (Control) -> Unit = {},
) : Frame(id) {
    private var index = controls.indexOfFirst { it.key == selected }.takeIf { it >= 0 } ?: 0

    val current: Control? get() = controls.getOrNull(index)

    override val rendered: Rendered
        get() {
            val control = current ?: return Rendered("")
            val reading = readings.read(control.line)
            if (control is Control.Edit) {
                val shown = control.editor.shown()
                return reading.rendered(shown.cells, shown.cursor)
            }
            return reading.rendered()
        }

    private fun move(step: Int): Boolean {
        if (controls.isEmpty()) return true
        (current as? Control.Edit)?.editor?.commit()
        (current as? Control.Edit)?.let { it.changed(it.editor.text) }
        index = (index + step + controls.size) % controls.size
        current?.let(moved)
        return true
    }

    /** Presses the button of [letter]; false if none has it. */
    fun press(letter: Char): Boolean {
        val button = controls.filterIsInstance<Control.Button>().firstOrNull { it.letter == letter } ?: return false
        button.press()
        return true
    }

    override fun routed(at: Int) {
        val control = current
        if (control is Control.Edit) control.editor.route(at)
        if (control is Control.Button) control.press()
    }

    override fun key(key: Key): Boolean {
        when (key) {
            Key.Tab -> return move(+1)
            Key.ShiftTab -> return move(-1)
            else -> {}
        }
        return when (val control = current) {
            is Control.Edit -> edit(control, key)
            is Control.Choice -> choice(control, key)
            is Control.Toggle -> when (key) {
                Key.Space, Key.Backspace, Key.Enter -> run { control.change(!control.on); true }
                else -> false
            }
            is Control.Button -> when (key) {
                Key.Enter -> run { control.press(); true }
                else -> false
            }
            is Control.Static, null -> false
        }
    }

    private fun edit(control: Control.Edit, key: Key): Boolean {
        val editor = control.editor
        return when (key) {
            Key.Enter -> run {
                editor.commit()
                control.changed(editor.text)
                control.submit?.invoke() ?: move(+1)
                true
            }
            Key.CharLeft -> run { editor.left(); true }
            Key.CharRight -> run { editor.right(); true }
            Key.WordLeft -> run { editor.wordLeft(); true }
            Key.WordRight -> run { editor.wordRight(); true }
            Key.LineStart, Key.First -> run { editor.start(); true }
            Key.LineEnd, Key.Last -> run { editor.end(); true }
            Key.Backspace -> run { editor.backspace(); control.changed(editor.text); true }
            Key.Delete -> run { editor.delete(); control.changed(editor.text); true }
            Key.Space -> run { editor.space(); control.changed(editor.text); true }
            is Key.Cell -> run { editor.cell(key.dots); true }
            is Key.Typed -> run { editor.type(key.text); control.changed(editor.text); true }
            else -> false
        }
    }

    private fun choice(control: Control.Choice, key: Key): Boolean {
        val options = control.options
        val step = { by: Int ->
            if (options.isNotEmpty()) control.choose(options[(control.selected + by + options.size) % options.size].first)
            true
        }
        return when (key) {
            Key.Up, Key.Backspace -> step(-1)
            Key.Down, Key.Space -> step(+1)
            Key.First -> run { options.firstOrNull()?.let { control.choose(it.first) }; true }
            Key.Last -> run { options.lastOrNull()?.let { control.choose(it.first) }; true }
            Key.Enter -> run { control.press?.invoke() ?: move(+1); true }
            is Key.Cell, is Key.Typed -> run {
                val letter = letters.letter(key) ?: return@run true
                val found = letters.find(options.map { it.second }, control.selected, letter)
                if (found != null) control.choose(options[found].first)
                true
            }
            else -> false
        }
    }
}
