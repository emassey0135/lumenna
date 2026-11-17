package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.BrailleLetters
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.OrbitMenu

/** One row of a list. */
class Item(
    /** What names the row across a refresh, so the row in hand stays on it. */
    val key: String,
    /** What Orbit says of it. */
    val text: String,
    /** What Select, Dot 8 and Right do; none sounds the error tone. */
    val open: (() -> Unit)? = null,
    /** Its own actions, first on the menu key's menu while it is in hand. */
    val actions: List<Action> = emptyList(),
    /** A letter that opens it from the list, pressed with Space (or Dot 8) on the Speak. */
    val letter: Char? = null,
    /** Whether the rows under it show, for a row that folds; null for one that does not. */
    val expanded: Boolean? = null,
    /** Folds it open (true, Right) or shut (false, Left); a row with none goes on to Right's open and Left's back. */
    val fold: ((open: Boolean) -> Unit)? = null,
    /** Its name alone, without what the line says after it: what a menu of its actions is named for. */
    val title: String? = null,
)

/** Something the menu key offers, for the row in hand or for the screen. */
class Action(
    val name: String,
    /** Its access key in the menu, and pressed with Space (or Dot 8) from the screen itself. */
    val letter: Char? = null,
    val run: () -> Unit,
)

/** An item or submenu of a menu: Orbit's own ([OrbitMenu.Item]), a submenu when it has children. */
typealias MenuItem<T> = OrbitMenu.Item<T>

/**
 * A screen's own menu tree for the menu key ([Screen.menuBar]): submenus of items, each running what
 * it holds, and the hot keys (Space or Dot 8 with a letter) that run an item from anywhere in it.
 */
class MenuBar(
    val items: List<MenuItem<() -> Unit>>,
    val hotKeys: Map<Char, () -> Unit> = emptyMap(),
)

/** One control of a [OrbitUi.form], by the [key] its value is read back with. */
sealed class Field(val key: String, val label: String) {
    /** An edit box; several lines when [multiLine], where Dot 8 starts a line. */
    class Line(key: String, label: String, val value: String = "", val hint: String = "", val multiLine: Boolean = false) : Field(key, label)

    /** A combo box of [options], [selected] in hand. */
    class Choice(key: String, label: String, val options: List<String>, val selected: Int = 0) : Field(key, label)

    /** A check box. */
    class Check(key: String, label: String, val checked: Boolean = false) : Field(key, label)

    /** A line to read, which holds nothing. */
    class Text(key: String, val value: String) : Field(key, "")
}

/** What a form held as OK was pressed, by each field's key. */
class FormValues internal constructor(
    private val texts: Map<String, String>,
    private val choices: Map<String, Int>,
    private val checks: Map<String, Boolean>,
) {
    /** What a [Field.Line] holds. */
    fun text(key: String): String = texts[key] ?: throw IllegalArgumentException("No line is called $key")

    /** Which option of a [Field.Choice] is chosen, from 0. */
    fun choice(key: String): Int = choices[key] ?: throw IllegalArgumentException("No choice is called $key")

    /** Whether a [Field.Check] is checked. */
    fun checked(key: String): Boolean = checks[key] ?: throw IllegalArgumentException("No check is called $key")
}

/**
 * What the layer itself says or names, in English by default: the buttons of its dialogs, what an
 * empty list says, finding by letter on the Player, a check box's state where it is a row, and what
 * a letter that reaches nothing says. Pass your own for another language; a word with a part is a
 * function of it.
 */
data class UiWords(
    val ok: String = "OK",
    val cancel: String = "Cancel",
    val yes: String = "Yes",
    val no: String = "No",
    /** What a list with no rows says. */
    val empty: String = "Empty",
    /** Holding star on the Player turned finding by letter on or off. */
    val lettersOn: String = "Finding by letter on",
    val lettersOff: String = "Finding by letter off",
    /** A check box's state, said in its row on the Player, where a form is a list. */
    val checked: String = "checked",
    val notChecked: String = "not checked",
    /** A letter alone that no row of the list starts with. */
    val noRowStartsWith: (letter: String) -> String = { "No row starts with $it" },
    /** Space or Dot 8 with a letter that nothing here has. */
    val nothingOnLetter: (letter: String) -> String = { "Nothing on $it" },
)

/**
 * Which letter a key presses: a cell alone moves by first letter or is a menu's access key, and
 * Space or Dot 8 with a letter runs that letter's row or action. By default a letter is its six
 * dots ([BrailleLetters]) and either key presses it; an app with rules of its own (which letters
 * Orbit keeps with Space, so they take Dot 8) gives its own.
 */
open class Letters {
    /** What Space or Dot 8 with a letter came to. */
    sealed interface Pressed {
        /** The letter, lowercase, to run. */
        data class Letter(val letter: Char) : Pressed

        /** A letter this key does not press: the error tone, then [said], where there is something to say. */
        data class Refused(val said: String?) : Pressed

        /** No letter at all: the key is no one's. */
        data object None : Pressed
    }

    /** The letter [dots] make, lowercase; null for a cell that is none. */
    open fun of(dots: Int): Char? = BrailleLetters.letter(dots)

    /** What [key] (Space or Dot 8 with dots) presses. */
    open fun pressed(key: Key): Pressed {
        val dots = when (key) {
            is Key.Chord -> key.dots
            is Key.Enter -> key.dots
            else -> return Pressed.None
        }
        return of(dots)?.let { Pressed.Letter(it) } ?: Pressed.None
    }
}
