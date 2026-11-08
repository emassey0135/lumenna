package com.extra.han.lumenna.device

/**
 * When the keys and the display are the app's: while it is resumed and its window has focus, and
 * at no other moment. Android pauses an activity for another app, but something drawn over it
 * (the device's task list, its global options, an alert) takes only the window's focus, and the
 * keys have to go then too, or nothing can answer them.
 *
 * [giveBack] runs at once on losing either, before anything else leaving does.
 */
class Claim(private val take: () -> Unit, private val giveBack: () -> Unit) {
    private var resumed = false
    private var focused = false

    /** Whether the keys and display are held now. */
    @Volatile var holding = false
        private set

    fun resumed(now: Boolean) {
        resumed = now
        settle()
    }

    fun focused(now: Boolean) {
        focused = now
        settle()
    }

    private fun settle() {
        val should = resumed && focused
        if (should == holding) return
        holding = should
        if (should) take() else giveBack()
    }
}
