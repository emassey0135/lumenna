package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.BrailleLetters
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.OrbitMenu

/**
 * One row of a list ([OrbitUi.list]), read again at each [OrbitUi.refresh]. Its first eleven
 * parameters are the BrailleSense SDK's `Item`'s, by name and in order, so a screen's rows are
 * written once for both devices.
 */
public class Item(
    /** What names the row across a refresh, so the row in hand stays on it. */
    public val key: String,
    /** What Orbit says of it, ahead of [detail]. */
    public val text: String,
    /** What Select, Dot 8 and Right do; with none, Select is [toggle], else the error tone. */
    public val open: (() -> Unit)? = null,
    /** Its own actions, first on the menu key's menu while it is in hand. */
    public val actions: List<Action> = emptyList(),
    /** A letter that opens it from the list, pressed with Space (or Dot 8) on the Speak. */
    public val letter: Char? = null,
    /** Said after [text], joined with a comma: its state, its count, what else it holds. */
    public val detail: String = "",
    /** Whether the rows under it show, for a row that folds; null for one that does not. */
    public val expanded: Boolean? = null,
    /** Folds it open (true, Right) or shut (false, Left); a row with none goes on to Right's open and Left's back. */
    public val fold: ((open: Boolean) -> Unit)? = null,
    /**
     * Select on a row with no [open]: a check box's change. The row is read again once the app has
     * made it, when it reads otherwise after [OrbitUi.refresh].
     */
    public val toggle: (() -> Unit)? = null,
    /** Space with D on the row (Dot 8 with D where the app's [Letters] keep Space with it): deletes it. */
    public val delete: (() -> Unit)? = null,
    /** Its name alone, without what the line says after it: what a menu of its actions is named for. */
    public val title: String? = null,
) {
    /** What Orbit is told of the row: its [text], then its [detail]. */
    public val line: String get() = if (detail.isEmpty()) text else "$text, $detail"
}

/** Something the menu key offers, for the row in hand or for the screen. */
public class Action(
    /** What the menu calls it. */
    public val name: String,
    /** Its access key in the menu, and pressed with Space (or Dot 8) from the screen itself. */
    public val letter: Char? = null,
    /**
     * Whether running it opens a dialog. Orbit's menus do not mark one, so it changes nothing here;
     * it is the BrailleSense SDK's mark, kept so an action is written once for both devices.
     */
    public val asks: Boolean = false,
    /** What choosing it does. */
    public val run: () -> Unit,
)

/**
 * An item of a menu ([OrbitUi.menu]): a submenu when it has [children], else one that gives [value]
 * when chosen. The BrailleSense SDK's `MenuItem` less its hot key, which on Orbit is a [MenuBar]'s.
 */
public class MenuItem<out T>(
    /** What Orbit says of it. */
    public val name: String,
    /** Its access key in its level: the letter alone selects it. */
    public val letter: Char? = null,
    /** Its items, for a submenu. */
    public val children: List<MenuItem<T>> = emptyList(),
    /** What choosing it gives. */
    public val value: T? = null,
    /** Whether choosing it opens a dialog: Orbit's menus do not mark one, so it changes nothing here. */
    public val asks: Boolean = false,
)

/** [this] as Orbit's own menu holds it. */
internal fun <T> MenuItem<T>.orbit(): OrbitMenu.Item<T> =
    OrbitMenu.Item(name, letter?.lowercaseChar(), children.map { it.orbit() }, value)

/**
 * A screen's own menu tree for the menu key ([Screen.menuBar]): submenus of items, each running what
 * it holds, and the hot keys (Space or Dot 8 with a letter) that run an item from anywhere in it.
 */
public class MenuBar(
    /** The menus, each a submenu of what it runs. */
    public val items: List<MenuItem<() -> Unit>>,
    /** What Space or Dot 8 with each letter runs, from anywhere in the menu and from the screen under it. */
    public val hotKeys: Map<Char, () -> Unit> = emptyMap(),
)

/**
 * One control of a [OrbitUi.form], by the [key] its value is read back with. The same as the
 * BrailleSense SDK's `Field`.
 */
public sealed class Field(
    /** What its value is read back by, in [FormValues]. */
    public val key: String,
    /** What Orbit names it. */
    public val label: String,
) {
    /** An edit box holding [value]; several lines when [multiLine], where Dot 8 starts a line. [hint] is said after it. */
    public class Line(
        key: String,
        label: String,
        public val value: String = "",
        public val hint: String = "",
        public val multiLine: Boolean = false,
    ) : Field(key, label)

    /** A combo box of [options], [selected] in hand. */
    public class Choice(key: String, label: String, public val options: List<String>, public val selected: Int = 0) : Field(key, label)

    /** A check box, [checked] or not. */
    public class Check(key: String, label: String, public val checked: Boolean = false) : Field(key, label)

    /** A line to read, [value], which holds nothing. */
    public class Text(key: String, public val value: String) : Field(key, "")
}

/**
 * What a form held as OK was pressed, by each field's key. The form builds it; an app's own tests
 * may build one by hand. A key the form had no such field of throws, so a misspelt key is found at
 * once rather than read as empty.
 */
public class FormValues(
    private val texts: Map<String, String>,
    private val choices: Map<String, Int>,
    private val checks: Map<String, Boolean>,
) {
    /** What a [Field.Line] holds. */
    public fun text(key: String): String = texts[key] ?: throw IllegalArgumentException("No line is called $key")

    /** Which option of a [Field.Choice] is chosen, from 0. */
    public fun choice(key: String): Int = choices[key] ?: throw IllegalArgumentException("No choice is called $key")

    /** Whether a [Field.Check] is checked. */
    public fun checked(key: String): Boolean = checks[key] ?: throw IllegalArgumentException("No check is called $key")
}

/**
 * What the layer itself says or names, in English by default: the buttons of its dialogs, what an
 * empty list says, finding by letter on the Player, a check box's state where it is a row, and what
 * a letter that reaches nothing says. Pass your own for another language; a word with a part is a
 * function of it. [ok], [cancel], [yes], [no], [empty], [noRowStartsWith] and [nothingOnLetter] are
 * the BrailleSense SDK's `UiWords`' too.
 */
public data class UiWords(
    /** A form's button that confirms it. */
    val ok: String = "OK",
    /** A form's button that leaves it unconfirmed. */
    val cancel: String = "Cancel",
    /** A question's yes. */
    val yes: String = "Yes",
    /** A question's no. */
    val no: String = "No",
    /** What a list with no rows says. */
    val empty: String = "Empty",
    /** Holding star on the Player turned finding by letter on. */
    val lettersOn: String = "Finding by letter on",
    /** Holding star on the Player turned finding by letter off. */
    val lettersOff: String = "Finding by letter off",
    /** A check box's state, said in its row on the Player, where a form is a list. */
    val checked: String = "checked",
    /** A check box's state unchecked, said in its row on the Player. */
    val notChecked: String = "not checked",
    /** A letter alone that no row of the list starts with. */
    val noRowStartsWith: (letter: Char) -> String = { "No row starts with $it" },
    /** Space or Dot 8 with a letter that nothing here has. */
    val nothingOnLetter: (letter: Char) -> String = { "Nothing on $it" },
)

/**
 * Which letter a key presses: a cell alone moves by first letter or is a menu's access key, and
 * Space or Dot 8 with a letter runs that letter's row or action. By default a letter is its six
 * dots ([BrailleLetters]) and either key presses it; an app with rules of its own (which letters
 * Orbit keeps with Space, so they take Dot 8) gives its own.
 */
public open class Letters {
    /** What Space or Dot 8 with a letter came to. */
    public sealed interface Pressed {
        /** The [letter], lowercase, to run. */
        public data class Letter(val letter: Char) : Pressed

        /** A letter this key does not press: the error tone, then [said], where there is something to say. */
        public data class Refused(val said: String?) : Pressed

        /** No letter at all: the key is no one's. */
        public data object None : Pressed
    }

    /** The letter [dots] make, lowercase; null for a cell that is none. */
    public open fun of(dots: Int): Char? = BrailleLetters.letter(dots)

    /** What [key] (Space or Dot 8 with dots) presses. */
    public open fun pressed(key: Key): Pressed {
        val dots = when (key) {
            is Key.Chord -> key.dots
            is Key.Enter -> key.dots
            else -> return Pressed.None
        }
        return of(dots)?.let { Pressed.Letter(it) } ?: Pressed.None
    }
}
