package dev.emassey0135.lumenna.orbit

import java.util.Locale

/**
 * Multi-tap (T9) letter entry on the Orbit Player's phone keypad, as Orbit's own edit fields behave.
 * Ported from the Satellite launcher (`T9Input`, Apache-2.0, the same author's).
 *
 * Tapping a digit offers its first character; tapping the same key again moves on to its next,
 * wrapping round. The offered character goes in at the caret where its key was first tapped when
 * another key is tapped, when the field is edited or moved, or when the person's commit timeout
 * passes with no further tap. Star switches mode (lowercase, uppercase, digits, symbols); pound
 * would switch keyboard layout, of which only English exists.
 */
class T9Input(private val host: Host, private val later: Later) {
    /** What the edit field does with T9's output. */
    interface Host {
        /** Where the caret is. */
        fun caret(): Int
        fun timeoutMs(): Long
        /** A new candidate character was offered. */
        fun candidate(character: Char)
        /** Insert [character] at [position]; [feedback] when the commit timeout did it. */
        fun commit(character: Char, position: Int, feedback: Boolean)
        /** The mode changed; [name] is said by the screen reader in the person's language. */
        fun modeChanged(name: String)
    }

    /** One pending call after a delay, replaced by the next. */
    interface Later {
        fun after(milliseconds: Long, run: () -> Unit)
        fun cancel()
    }

    enum class Mode(val spoken: String) {
        LOWERCASE("lowercase"),
        UPPERCASE("uppercase"),
        DIGITS("digits"),
        SYMBOLS("symbols"),
    }

    private var mode = Mode.LOWERCASE
    private var key = -1
    private var taps = 0
    private var pending: Char? = null
    private var pendingPosition = -1

    val hasPending: Boolean get() = pending != null

    /** The candidate and where it goes, for showing the text with it in place. */
    val candidate: Char? get() = pending
    val candidatePosition: Int get() = pendingPosition

    /** A tap (release) of digit [digit]. */
    fun tap(digit: Int) {
        val digits = mode == Mode.DIGITS
        val differentKey = key != -1 && key != digit
        // In digits mode each key has one character, so a repeated key types it again.
        if (!differentKey && digits && pending != null) commit(false)
        val characters = table()[digit]
        later.cancel()
        if (key == digit) {
            pending = characters[taps % characters.length]
            taps++
        } else {
            if (pending != null) commit(false)
            key = digit
            taps = 1
            pending = characters[0]
        }
        later.after(host.timeoutMs()) { commit(true) }
        if (pendingPosition < 0 || differentKey || digits) pendingPosition = host.caret()
        host.candidate(pending!!)
    }

    /** Star: next mode. */
    fun nextMode() {
        if (pending != null) commit(false)
        val modes = Mode.entries
        mode = modes[(mode.ordinal + 1) % modes.size]
        host.modeChanged(mode.spoken)
    }

    /** Pound: next keyboard layout. Only English exists, so this just commits any candidate. */
    fun nextLayout() {
        if (pending != null) commit(false)
    }

    /** Puts the candidate into the text now, with no feedback, as any edit or move does first. */
    fun commitNow() {
        if (pending != null) commit(false)
    }

    /** Drops any candidate without inserting it: the field is closing. */
    fun cancel() {
        reset()
        pendingPosition = -1
    }

    private fun commit(feedback: Boolean) {
        val character = pending
        val position = pendingPosition
        reset()
        pendingPosition = -1
        if (character != null && position >= 0) host.commit(character, position, feedback)
    }

    private fun reset() {
        key = -1
        taps = 0
        pending = null
        later.cancel()
    }

    private fun table(): Array<String> = when (mode) {
        Mode.UPPERCASE -> UPPERCASE
        Mode.DIGITS -> DIGITS
        Mode.SYMBOLS -> SYMBOLS
        Mode.LOWERCASE -> LOWERCASE
    }

    private companion object {
        val LOWERCASE = arrayOf(" ", ".,?!-@_;:/", "abc", "def", "ghi", "jkl", "mno", "pqrs", "tuv", "wxyz")
        val UPPERCASE = Array(LOWERCASE.size) { LOWERCASE[it].uppercase(Locale.ROOT) }
        val DIGITS = Array(10) { it.toString() }
        val SYMBOLS = arrayOf(" ", ".,?!", "-@_;", ":/\\|", "#$%^", "&*'`", "(){}", "{}<>", "\"~°№", "+=©•")
    }
}
