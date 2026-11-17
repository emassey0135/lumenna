package dev.emassey0135.braillesense.ui

/**
 * One screen on [BrailleSenseUi]'s stack: what is in front until it closes. An app subclasses it
 * for a screen of its own; the layer's lists, menus, lines, questions and forms are screens too.
 */
abstract class Screen {
    /** What it is called, said with its line as it comes to the front. */
    abstract val title: String

    /** What is said ahead of its line as it comes to the front: its title, unless it says otherwise. */
    open val heading: String get() = title

    /** What is said ahead of [frame]'s line as it is first shown: [heading], unless the frame calls for more. */
    open fun heading(frame: Frame): String = heading

    /**
     * The frame it shows now, built again as the stack is [BrailleSenseUi.refresh]ed. A frame of
     * another [Frame.id] than the last is a new thing in front, said with its heading.
     */
    abstract fun frame(): Frame

    /** A frame of a new [Frame.id] was shown: the app may need to know what is in front (a list beside its main one). */
    open fun shown(frame: Frame) {}

    /** Space-M or F2: the screen's own commands, as the device's one menu of a screen. Empty for none. */
    open fun menu(): List<MenuItem<() -> Unit>> = emptyList()

    /**
     * Escape, which the frame did not take: true if the screen dealt with it itself (sent its
     * cancel, and stays until the app closes it); false closes it, and closing the last screen
     * leaves the app.
     */
    open fun back(): Boolean = false

    /** Space or Enter with [letter], which nothing in front took: true if the screen ran it. */
    open fun letter(letter: Char): Boolean = false

    /** Whether it still stands: one that does not is closed at the next [BrailleSenseUi.refresh] (the app closed what it shows). */
    open fun stands(): Boolean = true

    /** It left the stack, closed by Escape, by an answer, or by the app: called once. */
    open fun closed() {}
}

/** A row of a list: [key] names it across a refresh, so the row in hand stays where it was. */
class Item(
    val key: String,
    val text: String,
    /** Enter or a routing key on it. */
    val open: (() -> Unit)? = null,
    /** Enter-I or the Application key: its own actions, a menu of them. */
    val actions: List<Action> = emptyList(),
    /** The letter that runs [open] from the list, shown as the device shows a shortcut. */
    val letter: Char? = null,
    /** Said and shown after the text, ahead of its number. */
    val detail: String = "",
    /** What the cells show, where it is not what is said. */
    val braille: String = text,
    /** What a first letter finds it by. */
    val finding: String = text,
    /** Space on it: a check box's change. Said once the app has changed it ([BrailleSenseUi.refresh]). */
    val space: (() -> Unit)? = null,
    /** Space-D on it. */
    val delete: (() -> Unit)? = null,
    /** Space-3 and Space-6: its folding. Said once the app has folded it. */
    val collapse: (() -> Unit)? = null,
    val expand: (() -> Unit)? = null,
    /** Space or Enter with a letter on it: its command; true if taken. */
    val command: ((Char) -> Boolean)? = null,
    /** What the device calls it. */
    val role: dev.emassey0135.braillesense.reading.ControlRole = dev.emassey0135.braillesense.reading.ControlRole.LIST_ITEM,
    /** Whether it opens a dialog, which the device marks. */
    val asks: Boolean = false,
)

/** Something a row or a screen can do, with the letter that runs it from its menu. */
class Action(val name: String, val letter: Char? = null, val asks: Boolean = false, val run: () -> Unit)

/**
 * An item of a menu: a submenu when it has [children], else one that gives [value] when chosen.
 * [letter] is the one it asks for, kept if no item before it took it; [hotKey] what runs it from
 * anywhere in the menu ("Enter-n"), shown after its name.
 */
class MenuItem<out T>(
    val name: String,
    val letter: Char? = null,
    val children: List<MenuItem<T>> = emptyList(),
    val value: T? = null,
    val hotKey: String? = null,
    /** Whether choosing it opens a dialog, which the device's menus mark. */
    val asks: Boolean = false,
)

/** A field of a form ([BrailleSenseUi.form]): its [key] names its value in the [FormValues]. */
sealed class Field(val key: String, val label: String) {
    /** An edit box, of several lines where [multiLine]. */
    class Line(key: String, label: String, val value: String = "", val hint: String = "", val multiLine: Boolean = false) : Field(key, label)

    /** A combo box: Space and Backspace go round its options. */
    class Choice(key: String, label: String, val options: List<String>, val selected: Int = 0) : Field(key, label)

    /** A check box: Space turns it over. */
    class Check(key: String, label: String, val checked: Boolean = false) : Field(key, label)

    /** A line to read, a static box. */
    class Text(key: String, val value: String) : Field(key, "")
}

/** What a form held as it was confirmed, by each field's key. */
class FormValues(
    private val texts: Map<String, String>,
    private val choices: Map<String, Int>,
    private val checks: Map<String, Boolean>,
) {
    /** A [Field.Line]'s text. */
    fun text(key: String): String = texts[key].orEmpty()

    /** A [Field.Choice]'s option, from 0. */
    fun choice(key: String): Int = choices[key] ?: 0

    /** A [Field.Check]'s state. */
    fun checked(key: String): Boolean = checks[key] ?: false
}

/**
 * The words the layer says itself, in the device's own English: its buttons and prompt buttons, and
 * what a menu that closes unchosen says. An app passes its own for another language.
 */
data class UiWords(
    val ok: String = "OK",
    val cancel: String = "Cancel",
    val yes: String = "Yes",
    val no: String = "No",
    /** A list with no rows. */
    val empty: String = "No items",
    /** A key that does nothing where it was pressed. */
    val nothingHere: String = "Not available",
    /** Enter on a row that opens nothing. */
    val opensNothing: String = "Not available",
    /** Enter-I on a row with no actions. */
    val noActions: String = "No actions",
    /** A letter no row or item starts with. */
    val notFound: (Char) -> String = { "Nothing starts with $it" },
    /** Space or Enter with a letter that runs nothing here. */
    val nothingOnLetter: (Char) -> String = { "Nothing on $it" },
)
