package dev.emassey0135.lumenna

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow

/**
 * Where what is to be said goes, in order, every one: a run of changes outran a buffer of
 * eight, and what overflowed was never said. Nothing is kept while nobody listens, as no
 * screen is there to say it.
 */
fun announcementFlow(): MutableSharedFlow<String> =
    MutableSharedFlow(extraBufferCapacity = 64, onBufferOverflow = BufferOverflow.DROP_OLDEST)

/**
 * One announcement as a live region carries it: the [count]th said, so each is a change
 * TalkBack hears, though its words be the same as the last.
 */
data class Said(val text: String, val count: Long) {
    /**
     * What the region says: the text, every other one with a mark that is not heard. A polite
     * live region is said when it changes, and the same sentence twice ("Marked done") was no
     * change, so the second was never said.
     */
    val description: String get() = if (count % 2 == 0L) text + "⁠" else text

    companion object {
        /** How long one is held before the next takes its place: long enough to be heard. */
        fun hold(text: String): Long = (1_500L + 60L * text.length).coerceAtMost(6_000L)

        /** How long the last stays before the region empties. */
        const val CLEARED = 8_000L
    }
}

/**
 * Says each of [announcements] through [show], in the order they came, each held for [hold]
 * before the next: set as they arrived, each took the place of the one before before it was
 * heard. What arrives meanwhile waits in the flow's own buffer.
 */
suspend fun sayInTurn(announcements: Flow<String>, hold: (String) -> Long = Said::hold, show: (Said) -> Unit) {
    var count = 0L
    announcements.collect { text ->
        show(Said(text, ++count))
        delay(hold(text))
    }
}
