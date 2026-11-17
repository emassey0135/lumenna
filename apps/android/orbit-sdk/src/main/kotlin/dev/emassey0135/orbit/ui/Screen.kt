package dev.emassey0135.orbit.ui

import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.Reader

/**
 * One screen of an [OrbitUi]'s stack: a list, a menu, a line, a dialog, or one of the app's own.
 *
 * The screen on top has the keys first ([key]); what it leaves, the stack does as Orbit's own apps
 * do (Left, Back, Escape and Dot 7 close it, the menu key opens [inHandActions] then [actions], a
 * letter with Space or Dot 8 runs the action of that letter), and anything else sounds the error
 * tone. A screen tells Orbit's reader what it is as it comes to the top ([tellShown]) and only what
 * moved afterwards ([tellMoved], [refresh]), so nothing talks over the person unasked.
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

    /** The screen's own actions, after the row in hand's on the menu key's menu. */
    open fun actions(): List<Action> = emptyList()

    /** The actions of what is in hand (a list's row), first on the menu key's menu. */
    open fun inHandActions(): List<Action> = emptyList()

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

    /** A key, the screen's first; true if it took it. */
    open fun key(key: Key): Boolean = false

    /** Whether the screen still stands: one that does not is closed at the next [OrbitUi.refresh]. */
    open fun stands(): Boolean = true

    /** The screen left the stack, after it was taken off: an ask answers here. */
    open fun closed() {}
}
