package dev.emassey0135.lumenna.orbit

import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.firstLetter
import dev.emassey0135.lumenna.app.noRowStartsWith
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.orbit.Key
import dev.emassey0135.orbit.ListFinder
import dev.emassey0135.orbit.T9Input

/**
 * Finding a row by typing. A letter moves to the next row starting with it: the app's own lists
 * by the core (`ShellEvent::FindLetter`, which goes by a row's name), this app's by the core's
 * `first_letter`. On the Player the SDK's [ListFinder] has digits open rows and holding star find
 * by letter with T9.
 */
class FirstLetters(private val ui: Ui) {
    private val finder = ListFinder(object : ListFinder.Host {
        override val count: Int get() = ui.top?.rows()?.size ?: 0
        override val commitMs: Long get() = ui.reader.characterCommitTimeout.toLong()
        override fun find(letter: String) = this@FirstLetters.letter(letter)
        override fun open(index: Int) = openRow(index)
        override fun refuse() = ui.refuse()
        override fun speak(text: String) = ui.reader.speak(text)
        override fun modeChanged(mode: T9Input.Mode) = ui.reader.speak(ui.orbitWords.mode(mode))
        override fun lettersChanged(on: Boolean) = ui.say(if (on) appWords.orbit.lettersOn else appWords.orbit.lettersOff)
    }, ui)

    /** Forgets what was typed, as any other key does. */
    fun reset() = finder.reset()

    /** A letter from the keys, or null for a cell that is none. */
    fun letter(letter: String?) {
        val screen = ui.top ?: return
        if (letter == null) {
            ui.refuse()
            return
        }
        // The app's own lists: it says if nothing starts with it, as a refusal.
        if (screen.lands == Lands.MAIN) {
            ui.send(ShellEvent.FindLetter(letter))
            return
        }
        val rows = screen.rows()
        val at = firstLetter(rows.map { it.text }, booleanArrayOf(), screen.index.toUInt(), letter)?.toInt()
        when {
            at == null -> {
                ui.refuse()
                ui.say(noRowStartsWith(letter))
            }
            at == screen.index -> ui.say(rows[at].text)
            else -> {
                screen.index = at
                screen.selectedKey = rows[at].key
                screen.moved(rows[at])
                ui.redraw(Why.Moved)
            }
        }
    }

    /** A Player key: true if it was finding's, the rest going on to the list's keys. */
    @Suppress("UNUSED_PARAMETER")
    fun phoneKey(key: Key, screen: Screen): Boolean = finder.phoneKey(key)

    private fun openRow(at: Int) {
        val screen = ui.top ?: return
        val rows = screen.rows()
        val row = rows.getOrNull(at) ?: return
        screen.index = at
        screen.selectedKey = row.key
        screen.moved(row)
        row.open?.invoke()
    }
}
