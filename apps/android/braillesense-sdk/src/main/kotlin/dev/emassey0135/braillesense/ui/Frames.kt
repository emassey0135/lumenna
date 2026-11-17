package dev.emassey0135.braillesense.ui

import dev.emassey0135.braillesense.edit.LineEditor
import dev.emassey0135.braillesense.edit.TextBox
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.reading.BraillePart
import dev.emassey0135.braillesense.reading.ControlRole

/** What a frame shows and says now. */
public data class Rendered(
    /** What is said. */
    public val speech: String,
    /** What the cells show, in the person's table. */
    public val braille: String = speech,
    /** Where the display's cursor belongs, in [braille]'s text: -1 for none. */
    public val cursor: Int = -1,
    /** Cells ready made, for a line being typed in the person's own contractions. */
    public val cells: ByteArray? = null,
    /** Where the cursor is in [cells]. */
    public val cellCursor: Int = -1,
    /** What goes on the cells ahead of [braille]: the role, a label. */
    public val before: List<BraillePart> = emptyList(),
    /** What follows it: a shortcut letter, a sign, the role. */
    public val after: List<BraillePart> = emptyList(),
)

/**
 * One thing on the display, taking the keys that belong to it: a list ([ListFrame]), a menu
 * ([MenuFrame]), a line being typed ([LineFrame]), or an app's own (a dialog's controls). The app
 * keeps what is common to them all (panning, routing, Escape's way back, opening the menu); a frame
 * returns whether it took the key, and [rendered] is what to say and show, which
 * `BrailleDisplay.show(Rendered)` puts on the cells.
 */
public abstract class Frame(
    /** What it is, so the app can tell a new frame from the same one drawn again. */
    public val id: String,
) {
    /** [key], pressed while it is in front: true if it took it. */
    public abstract fun key(key: Key): Boolean

    /** What it says and shows now. */
    public abstract val rendered: Rendered

    /** A routing key over a character of [Rendered.braille] (-1 past its end): `BrailleDisplay.routed`. */
    public open fun routed(at: Int) {}

    /** The edit box in hand, whose keys (`BoxEditing`) come ahead of the frame's; null for none. */
    public open fun editBox(): TextBox? = null

    /** Every edit box it holds, the one in hand among them: read again when the braille table changes. */
    public open fun editBoxes(): List<TextBox> = listOfNotNull(editBox())

    /** The edit box's text was changed by its keys: what the frame does with it as it changes. */
    public open fun edited() {}

    /** Space or Enter with [letter], which the frame's own keys did not take: true if something in it ran. */
    public open fun pressLetter(letter: Char): Boolean = false

    /** Whether [key] asks the app to change what the line says (a check box, folding): the line is said once the app has, not now. */
    public open fun changes(key: Key): Boolean = false
}

/** One row of a list. */
public class Row(
    /** What names it across a redraw. */
    public val key: String,
    /** What is said of it. */
    public val speech: String,
    /** What the cells show of it. */
    public val braille: String = speech,
    /** The letter it is found by: its first, in its title. */
    public val finding: String = speech,
    /** Enter or a routing key on it. */
    public val open: (() -> Unit)? = null,
    /** Space on the row: a check box's change. */
    public val space: (() -> Unit)? = null,
    /** What its menu of actions offers. */
    public val actions: List<MenuEntry> = emptyList(),
    /** Space or Enter with a letter on the row: its command, for the app to run. True if taken. */
    public val command: ((Char) -> Boolean)? = null,
    /** Delete on the row. */
    public val delete: (() -> Unit)? = null,
    /** What the device calls it. */
    public val role: ControlRole = ControlRole.LIST_ITEM,
    /** What follows its letter, said and shown; empty for none. */
    public val detail: String = "",
    /** The letter that runs it, shown as the device shows a shortcut key. */
    public val letter: Char? = null,
    /** Space-3 and Space-6: its folding. */
    public val collapse: (() -> Unit)? = null,
    /** Space-6: unfolds it. */
    public val expand: (() -> Unit)? = null,
    /** Whether it opens a dialog, which the device marks. */
    public val asks: Boolean = false,
    /** Whatever the app keeps with the row; the list never looks at it. */
    public val tag: Any? = null,
)

/**
 * A list as the device's own are: Space-1 and Space-4 (or the arrows) move, stopping at the ends,
 * Space-1-2-3 and Space-4-5-6 go to them, Space-1-2-6 and Space-3-4-5 a group of 32, a letter finds
 * the next row starting with it, Enter or a routing key opens the row, Space-3 and Space-6 fold it,
 * Space-D deletes it, and Enter-I or the Application key opens its actions. The row in hand
 * survives a redraw when the app gives its key again as [selected]. A row's number is read with it,
 * as the device's own lists do where Index Reporting is on.
 */
public class ListFrame(
    id: String,
    /** What the list is called. */
    public val title: String,
    private val rows: List<Row>,
    selected: String?,
    private val empty: String,
    private val letters: Letters,
    private val moved: (Row) -> Unit = {},
    /** Opens the menu of [Row.actions]. */
    private val openActions: (Row) -> Unit = {},
    /** The app's own first-letter search, for a list whose rows it finds itself; null to search here by [Row.finding]. */
    private val findInApp: ((Char) -> Unit)? = null,
    /** Says that no row starts with the letter. */
    private val notFound: (Char) -> Unit = {},
    /** Whether the app acts on this list's rows by their keys: the frame only carries it, for the app's own letters. */
    public val appHeld: Boolean = false,
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

    /** The row in hand; null in an empty list. */
    public val current: Row? get() = rows.getOrNull(index)

    /** The current row's place, as a shell announces it. */
    public val position: Pair<Int, Int> get() = (index + 1) to rows.size

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

    override fun pressLetter(letter: Char): Boolean = current?.command?.invoke(letter) == true

    override fun changes(key: Key): Boolean = when (key) {
        Key.Space -> !prompt && current?.space != null
        Key.CharLeft -> current?.collapse != null
        Key.CharRight -> current?.expand != null
        else -> false
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

    private companion object {
        /** How far Space-1-2-6 and Space-3-4-5 go in a list: "32/40-item group" on this device. */
        private const val GROUP = 32
    }
}

/**
 * A line of text the person types, with its prompt, and what Enter does with it. Its other keys
 * are the box's (`BoxEditing`, which the shell runs ahead of the frame).
 */
public class LineFrame(
    id: String,
    private val prompt: String,
    /** The line's edit box. */
    public val box: TextBox,
    private val hint: String = "",
    private val readings: Readings,
    private val done: (String) -> Unit,
    private val changed: (String) -> Unit = {},
) : Frame(id) {
    /** The box's one line. */
    public val editor: LineEditor get() = box.line

    /** The text changed from outside the keys (a paste, a cut): told as a key's change is. */
    public fun touched() {
        changed(box.text)
    }

    override fun editBox(): TextBox = box

    override fun edited(): Unit = touched()

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
