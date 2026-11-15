package com.extra.han.lumenna.ui

import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.edit.LineEditor
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.reading.BraillePart
import dev.emassey0135.braillesense.reading.ControlRole
import dev.emassey0135.braillesense.reading.DeviceLine
import dev.emassey0135.lumenna.app.ControlShown
import dev.emassey0135.lumenna.app.ControlType
import dev.emassey0135.lumenna.braille.DialogController

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
    /** Whether it opens a dialog, which the device marks. */
    val asks: Boolean = false,
    /** A row of the shell's own, which the app does not hold: Home's actions beside its places. */
    val local: Boolean = false,
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
    /** Says that Enter has nothing to open here. */
    private val opensNothing: () -> Unit = {},
    /** Says that this row has no actions to list. */
    private val noActions: () -> Unit = {},
) : Frame(id) {
    private var index = rows.indexOfFirst { it.key == selected }.takeIf { it >= 0 } ?: 0

    val current: Row? get() = rows.getOrNull(index)

    /** The current row's place, as a shell announces it. */
    val position: Pair<Int, Int> get() = (index + 1) to rows.size

    override val rendered: Rendered
        get() = current?.let { row ->
            // A prompt's answers are a choice in place, numbered no more than the device numbers them.
            val (place, count) = if (prompt) 0 to 0 else position
            readings.read(line(row.role, row.speech, row.braille, detail = row.detail, letter = row.letter, asks = row.asks, position = place, count = count)).rendered()
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
        Key.Enter -> run { current?.open?.invoke() ?: opensNothing(); true }
        Key.Space -> if (prompt) wrap(+1) else run { current?.space?.invoke(); current?.space != null }
        Key.Backspace -> prompt && wrap(-1)
        Key.CharLeft -> run { current?.collapse?.invoke(); current?.collapse != null }
        Key.CharRight -> run { current?.expand?.invoke(); current?.expand != null }
        Key.Context -> run {
            current?.takeIf { it.actions.isNotEmpty() }?.let(openActions) ?: noActions()
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

/**
 * A line of text the person types, with its prompt, and what Enter does with it. Its other keys
 * are the box's (`BoxEditing`, which the shell runs ahead of the frame).
 */
class LineFrame(
    id: String,
    private val prompt: String,
    val box: TextBox,
    private val hint: String = "",
    private val readings: Readings,
    private val done: (String) -> Unit,
    private val changed: (String) -> Unit = {},
) : Frame(id) {
    val editor: LineEditor get() = box.line

    /** The text changed from outside the keys (a paste, a cut): told as a key's change is. */
    fun touched() {
        changed(box.text)
    }

    override val rendered: Rendered
        get() {
            val shown = editor.shown()
            return readings.read(line(ControlRole.EDIT_BOX, box.text.ifEmpty { hint }, box.text, label = prompt)).rendered(shown.cells, shown.cursor)
        }

    override fun routed(at: Int) {
        editor.route(at)
    }

    override fun key(key: Key): Boolean = when (key) {
        Key.Enter -> run { done(box.text); true }
        else -> false
    }
}

/**
 * A dialog the core describes (`DialogShown`), as the device works one, over the braille apps'
 * shared [DialogController]: the controller keeps the control in hand, the tab order, what is typed
 * until the core has it, each choice and check, and presses; this maps the device's keys onto it and
 * reads the control in hand in the device's own roles (`ControlRole`). Tab (Space-4-5, F3) and
 * Shift-Tab are all that move between controls, as in the device's own dialogs; Space-1 and
 * Space-4 move only within a list, Space and Backspace change a combo box or a check box, and step
 * between a question's answers. A line's own keys are the SDK's box (`BoxEditing`, run by the shell
 * ahead of the frame), its text handed to the controller as it changes.
 */
class FormFrame(
    val controller: DialogController,
    private val letters: Letters,
    private val readings: Readings,
    /** The text box of a line, which the shell keeps while the dialog is open. */
    private val boxOf: (ControlShown) -> TextBox,
    /** A button's letter: the core's, else the first free one of its name. */
    private val letterOf: (ControlShown) -> Char?,
    /** A question of answers alone: its buttons the device's prompt buttons, said as its prompt. */
    val prompt: Boolean = false,
) : Frame(controller.dialog.id) {
    /** What it is called, said as it opens. */
    val title: String? get() = controller.dialog.title.ifEmpty { null }

    /** The control in hand. */
    val current: ControlShown? get() = controller.control

    /** The lines to read ahead of where it opens (a message, why saving was refused), said as it opens. */
    val preamble: String
        get() = controller.controls.take(controller.dialog.focus.toInt()).filter { it.kind == ControlType.TEXT }.joinToString(". ") { it.value }

    /** The box of the line in hand, if a line is in hand. */
    fun box(): TextBox? = current?.takeIf(::isLine)?.let(boxOf)

    /** The line in hand's text, as its box now holds it, handed to the controller where it changed. */
    fun typed() {
        val line = current?.takeIf(::isLine) ?: return
        val text = boxOf(line).text
        if (text != controller.text(line)) controller.type(text)
    }

    override val rendered: Rendered
        get() {
            val control = current ?: return Rendered("")
            val reading = readings.read(lineOf(control))
            if (isLine(control)) {
                val shown = boxOf(control).line.shown()
                return reading.rendered(shown.cells, shown.cursor)
            }
            return reading.rendered()
        }

    private fun lineOf(control: ControlShown): DeviceLine {
        val hint = control.example.ifEmpty { control.hint }
        return when (control.kind) {
            ControlType.TEXT -> line(ControlRole.STATIC_BOX, control.value)
            ControlType.LINE -> boxOf(control).let { box -> line(ControlRole.EDIT_BOX, box.text.ifEmpty { hint }, box.text, label = control.label) }
            ControlType.LINES -> boxOf(control).let { box ->
                line(ControlRole.MULTI_LINE_EDIT_BOX, box.line.text.ifEmpty { if (box.text.isEmpty()) hint else "" }, box.line.text, label = control.label)
            }
            ControlType.CHOICE -> {
                val at = controller.selected(control) ?: 0
                line(ControlRole.COMBO_BOX, control.options.getOrNull(at)?.title.orEmpty(), label = control.label, position = at + 1, count = control.options.size)
            }
            ControlType.LIST -> {
                val at = controller.selected(control) ?: 0
                line(ControlRole.LIST_ITEM, control.options.getOrNull(at)?.title.orEmpty(), label = control.label, position = at + 1, count = control.options.size)
            }
            ControlType.CHECK -> line(if (controller.checked(control)) ControlRole.CHECK_BOX_CHECKED else ControlRole.CHECK_BOX_UNCHECKED, control.label)
            ControlType.BUTTON -> line(if (prompt) ControlRole.PROMPT_BUTTON else ControlRole.BUTTON, control.label, letter = letterOf(control), asks = control.asks)
        }
    }

    /** Presses the button of [letter], the line in hand taken first; false if none has it. */
    fun press(letter: Char): Boolean {
        val button = controller.controls.firstOrNull { it.kind == ControlType.BUTTON && letterOf(it) == letter } ?: return false
        typed()
        controller.press(button)
        return true
    }

    override fun routed(at: Int) {
        val control = current ?: return
        if (isLine(control)) boxOf(control).line.route(at)
        if (control.kind == ControlType.BUTTON) controller.press()
    }

    override fun key(key: Key): Boolean {
        when (key) {
            Key.Tab -> return run { typed(); controller.next(); true }
            Key.ShiftTab -> return run { typed(); controller.previous(); true }
            else -> {}
        }
        val control = current ?: return false
        return when (control.kind) {
            // Enter in a line of one presses it (quick add's, a question's) or the dialog's default;
            // a multi-line box's Enter is its own new line.
            ControlType.LINE -> key == Key.Enter && run { typed(); controller.press(); true }
            ControlType.LINES -> false
            ControlType.CHOICE -> choice(control, key)
            ControlType.LIST -> list(control, key)
            ControlType.CHECK -> when (key) {
                Key.Space, Key.Backspace, Key.Enter -> run { controller.toggle(); true }
                else -> false
            }
            ControlType.BUTTON -> when {
                key == Key.Enter -> controller.press()
                prompt && key == Key.Space -> answer(+1)
                prompt && key == Key.Backspace -> answer(-1)
                else -> false
            }
            ControlType.TEXT -> false
        }
    }

    /** Space and Backspace among a question's answers: to the next or last of them, wrapping. */
    private fun answer(by: Int): Boolean {
        val answers = controller.controls.withIndex().filter { it.value.kind == ControlType.BUTTON }.map { it.index }
        val at = answers.indexOf(controller.index)
        if (at >= 0 && answers.size > 1) controller.moveTo(answers[Math.floorMod(at + by, answers.size)])
        return true
    }

    /** A combo box steps round its options, as the device's own does, and goes to its ends. */
    private fun choice(control: ControlShown, key: Key): Boolean {
        val options = control.options
        val step = { by: Int ->
            if (options.isNotEmpty() && controller.step(by) == null) controller.choose(if (by > 0) 0 else options.lastIndex)
            true
        }
        return when (key) {
            Key.Up, Key.Backspace -> step(-1)
            Key.Down, Key.Space -> step(+1)
            Key.First -> run { if (options.isNotEmpty()) controller.choose(0); true }
            Key.Last -> run { if (options.isNotEmpty()) controller.choose(options.lastIndex); true }
            Key.Enter -> run { if (!controller.pressDefault()) controller.next(); true }
            is Key.Cell, is Key.Typed -> run { letters.letter(key)?.let { controller.find(it.toString()) }; true }
            else -> false
        }
    }

    /** A list moves in place, stopping at its ends, and Enter picks the one in hand. */
    private fun list(control: ControlShown, key: Key): Boolean = when (key) {
        Key.Up -> run { controller.step(-1); true }
        Key.Down -> run { controller.step(+1); true }
        Key.First -> run { if (control.options.isNotEmpty()) controller.choose(0); true }
        Key.Last -> run { if (control.options.isNotEmpty()) controller.choose(control.options.lastIndex); true }
        Key.Enter -> controller.press()
        is Key.Cell, is Key.Typed -> run { letters.letter(key)?.let { controller.find(it.toString()) }; true }
        else -> false
    }

    private fun isLine(control: ControlShown) = control.kind == ControlType.LINE || control.kind == ControlType.LINES
}
