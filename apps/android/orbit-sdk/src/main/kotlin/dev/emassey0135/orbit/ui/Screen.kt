package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.Reader

/**
 * One screen of an [OrbitUi]'s stack: a list, a menu, a line, a dialog, or one of the app's own.
 *
 * The screen on top has the keys first ([key]); what it leaves, the stack does as Orbit's own apps
 * do (Left, Back, Escape and Dot 7 go [back], the menu key opens [menuBar] or else [inHandActions]
 * then [actions], a letter with Space or Dot 8 goes to [letter] and then to the action of that
 * letter), and anything else sounds the error tone. A screen tells Orbit's reader what it is as it
 * comes to the top ([tellShown]) and only what moved afterwards ([tellMoved], [refresh], [land]), so
 * nothing talks over the person unasked.
 */
abstract class Screen {
    /** The stack the screen is on, set as it is pushed. */
    lateinit var ui: OrbitUi
        internal set

    /** Whether it has been pushed: before, there is no [ui] to tell anything to. */
    internal val attached: Boolean get() = this::ui.isInitialized

    /** Its name, as Orbit says it. */
    abstract val title: String

    /** What the reader is told through. */
    protected val reader: Reader get() = ui.reader

    /**
     * Whether what the screen shows has arrived: until it has, it is not told, so nothing says a
     * list is empty on the way to it. The next [OrbitUi.refresh] tells it once it has.
     */
    open val loaded: Boolean get() = true

    /** The screen's own actions, after the row in hand's on the menu key's menu. */
    open fun actions(): List<Action> = emptyList()

    /** The actions of what is in hand (a list's row), first on the menu key's menu. */
    open fun inHandActions(): List<Action> = emptyList()

    /**
     * The screen's own menu tree for the menu key, in place of [inHandActions] then [actions]: a
     * menu bar of submenus, say, with hot keys that run from anywhere in it. None by default.
     */
    open fun menuBar(): MenuBar? = null

    /** Tells the reader the screen as newly shown: as it opens, and as what was over it closes. */
    abstract fun tellShown()

    /** Tells the reader what is in hand now, after it moved. */
    open fun tellMoved() {}

    /**
     * The app's data changed ([OrbitUi.refresh]): read it again, keeping what is in hand. Only when
     * [tell] is anything told, and then only what moved, since a change the person did not make must
     * not talk over them.
     */
    open fun refresh(tell: Boolean) {}

    /**
     * The app moved what is in hand itself ([OrbitUi.land]), to [key] where it names one: read it
     * again and tell what is in hand, unless it is what was just told, as it was told.
     */
    open fun land(key: String?) = refresh(true)

    /** The screen came to the top: pushed, or what was over it closed. Told or not, it is the app's to know. */
    open fun onTop() {}

    /** A key, the screen's first; true if it took it. */
    open fun key(key: Key): Boolean = false

    /**
     * Back, Escape, Left or Dot 7 that the screen did not take itself: true if the screen deals with
     * it (a form that discards first, a question its app closes); false closes it.
     */
    open fun back(): Boolean = false

    /** Space or Dot 8 with [letter], before any action of that letter: true if the app took it. */
    open fun letter(letter: Char): Boolean = false

    /** Whether the screen still stands: one that does not is closed at the next [OrbitUi.refresh]. */
    open fun stands(): Boolean = true

    /** The screen left the stack, after it was taken off: an ask answers here. */
    open fun closed() {}
}
