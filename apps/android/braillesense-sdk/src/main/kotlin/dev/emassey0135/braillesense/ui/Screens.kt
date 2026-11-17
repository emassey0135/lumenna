package dev.emassey0135.braillesense.ui

import dev.emassey0135.braillesense.reading.ControlRole

/**
 * One screen on [BrailleSenseUi]'s stack: what is in front until it closes. An app subclasses it
 * for a screen of its own; the layer's lists, menus, lines, questions and forms are screens too.
 */
public abstract class Screen {
    /** What it is called, said with its line as it comes to the front. */
    public abstract val title: String

    /** What is said ahead of its line as it comes to the front: its title, unless it says otherwise. */
    public open val heading: String get() = title

    /** What is said ahead of [frame]'s line as it is first shown: [heading], unless the frame calls for more. */
    public open fun heading(frame: Frame): String = heading

    /**
     * The frame it shows now, built again as the stack is [BrailleSenseUi.refresh]ed. A frame of
     * another [Frame.id] than the last is a new thing in front, said with its heading.
     */
    public abstract fun frame(): Frame

    /** A frame of a new [Frame.id] was shown: the app may need to know what is in front (a list beside its main one). */
    public open fun shown(frame: Frame) {}

    /** Space-M or F2: the screen's own commands, as the device's one menu of a screen. Empty for none. */
    public open fun menu(): List<MenuItem<() -> Unit>> = emptyList()

    /**
     * Escape, which the frame did not take: true if the screen dealt with it itself (sent its
     * cancel, and stays until the app closes it); false closes it, and closing the last screen
     * leaves the app.
     */
    public open fun back(): Boolean = false

    /** Space or Enter with [letter], which nothing in front took: true if the screen ran it. */
    public open fun letter(letter: Char): Boolean = false

    /** Whether it still stands: one that does not is closed at the next [BrailleSenseUi.refresh] (the app closed what it shows). */
    public open fun stands(): Boolean = true

    /** It left the stack, closed by Escape, by an answer, or by the app: called once. */
    public open fun closed() {}
}

/**
 * A row of a [BrailleSenseUi.list]: [key] names it across a refresh, so the row in hand stays where
 * it was. The parameters up to [title] are the Orbit SDK's `Item`'s too, in the same order, so a
 * screen written for one device builds on the other; those after it are this device's own.
 */
public class Item(
    /** What names the row across a refresh, so the row in hand stays on it. */
    public val key: String,
    /** What is said and shown of it. */
    public val text: String,
    /** Enter or a routing key on it; with none, Enter says the row opens nothing. */
    public val open: (() -> Unit)? = null,
    /** Enter-I or the Application key: its own actions, a menu of them. */
    public val actions: List<Action> = emptyList(),
    /** The letter that runs [open] from the list (Space or Enter with it), shown as the device shows a shortcut. */
    public val letter: Char? = null,
    /** Said and shown after the text, ahead of its number. */
    public val detail: String = "",
    /** Whether the rows under it show, for a row that folds; null for one that does not. */
    public val expanded: Boolean? = null,
    /**
     * Folds it open (true) or shut (false): Space-6 unfolds a folded row ([expanded] false), Space-3
     * folds an unfolded one ([expanded] true). The line is said once the app has folded it and
     * [BrailleSenseUi.refresh]ed.
     */
    public val fold: ((open: Boolean) -> Unit)? = null,
    /** Space on it: a check box's change, said once the app has changed it and [BrailleSenseUi.refresh]ed. */
    public val toggle: (() -> Unit)? = null,
    /** Space-D on it. */
    public val delete: (() -> Unit)? = null,
    /**
     * Its name alone, without what [text] says after it. The device names no menu of a row's
     * actions, so it is not used here; the Orbit SDK names a row's menu for it.
     */
    public val title: String? = null,
    /** What the cells show, where it is not what is said. */
    public val braille: String = text,
    /** What a first letter finds it by. */
    public val finding: String = text,
    /** Space or Enter with a letter on it: its command; true if taken, else the list's letters are tried. */
    public val command: ((Char) -> Boolean)? = null,
    /** What the device calls it: a list item, unless it is a button or a check box. */
    public val role: ControlRole = ControlRole.LIST_ITEM,
    /** Whether it opens a dialog, which the device marks. */
    public val asks: Boolean = false,
)

/** Something a row or a screen can do, with the letter that runs it from its menu. */
public class Action(
    /** What its menu item is called. */
    public val name: String,
    /** The letter of its menu item, and with Space or Enter from the screen. */
    public val letter: Char? = null,
    /** Whether it opens a dialog, which the device's menus mark. */
    public val asks: Boolean = false,
    /** What it does. */
    public val run: () -> Unit,
)

/**
 * An item of a menu: a submenu when it has [children], else one that gives [value] when chosen.
 * [letter] is the one it asks for, kept if no item before it took it. The parameters up to [asks]
 * are the Orbit SDK's `MenuItem`'s too.
 */
public class MenuItem<out T>(
    /** What it is called. */
    public val name: String,
    /** The letter it asks for, lowercase. */
    public val letter: Char? = null,
    /** Its items, for a submenu. */
    public val children: List<MenuItem<T>> = emptyList(),
    /** What choosing it gives. */
    public val value: T? = null,
    /** Whether choosing it opens a dialog, which the device's menus mark. */
    public val asks: Boolean = false,
    /** What runs it from anywhere in the menu ("Enter-n"), shown after its name: this device's own. */
    public val hotKey: String? = null,
)

/** A field of a form ([BrailleSenseUi.form]): its [key] names its value in the [FormValues]. */
public sealed class Field(
    /** What names its value in the [FormValues]. */
    public val key: String,
    /** What it is called, said and shown before its value. */
    public val label: String,
) {
    /** An edit box holding [value], of several lines where [multiLine]; [hint] is said while it is empty. */
    public class Line(
        key: String,
        label: String,
        /** What it holds as the form opens. */
        public val value: String = "",
        /** Said while it is empty. */
        public val hint: String = "",
        /** Whether Enter starts a new line in it rather than confirming the form. */
        public val multiLine: Boolean = false,
    ) : Field(key, label)

    /** A combo box of [options], [selected] chosen: Space and Backspace go round them. */
    public class Choice(key: String, label: String, public val options: List<String>, public val selected: Int = 0) : Field(key, label)

    /** A check box: Space turns it over. */
    public class Check(key: String, label: String, public val checked: Boolean = false) : Field(key, label)

    /** A line to read, a static box. */
    public class Text(key: String, public val value: String) : Field(key, "")
}

/**
 * What a form held as it was confirmed, by each field's key. Asking for a key no field of its kind
 * has is a mistake in the app: it throws [IllegalArgumentException].
 */
public class FormValues(
    private val texts: Map<String, String>,
    private val choices: Map<String, Int>,
    private val checks: Map<String, Boolean>,
) {
    /** A [Field.Line]'s text. */
    public fun text(key: String): String = texts[key] ?: throw IllegalArgumentException("No line is called $key")

    /** A [Field.Choice]'s option, from 0. */
    public fun choice(key: String): Int = choices[key] ?: throw IllegalArgumentException("No choice is called $key")

    /** A [Field.Check]'s state. */
    public fun checked(key: String): Boolean = checks[key] ?: throw IllegalArgumentException("No check is called $key")
}

/**
 * The words the layer says itself, in the device's own English: its buttons and prompt buttons, and
 * what a key that does nothing says. An app passes its own for another language. [ok], [cancel],
 * [yes], [no], [empty], [noRowStartsWith] and [nothingOnLetter] are the Orbit SDK's `UiWords`' too.
 */
public data class UiWords(
    /** A form's button that confirms it. */
    public val ok: String = "OK",
    /** A form's button that leaves it, and what a menu closed unchosen says. */
    public val cancel: String = "Cancel",
    /** A question's Yes. */
    public val yes: String = "Yes",
    /** A question's No. */
    public val no: String = "No",
    /** A list with no rows. */
    public val empty: String = "No items",
    /** A letter no row or item starts with. */
    public val noRowStartsWith: (Char) -> String = { "Nothing starts with $it" },
    /** Space or Enter with a letter that runs nothing here. */
    public val nothingOnLetter: (Char) -> String = { "Nothing on $it" },
    /** A key that does nothing where it was pressed (the menu key on a screen with no menu). */
    public val nothingHere: String = "Not available",
    /** Enter on a row that opens nothing. */
    public val opensNothing: String = "Not available",
    /** Enter-I on a row with no actions. */
    public val noActions: String = "No actions",
)
