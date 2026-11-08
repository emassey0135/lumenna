package dev.emassey0135.lumenna.orbit

/**
 * Holding a device's keys only while this app's window has focus.
 *
 * A system window over the app (an app-not-responding dialog, a notification shade) takes focus
 * without pausing the activity, and whatever drives that window needs the keys; on resume or
 * focus back, they are taken again. [take] and [release] are called only when the answer changes.
 * The service coming up after focus is [serviceUp]: the claim made then, since none could be
 * earlier. The BrailleSense app repeats this (`apps/android/braille/`, when lifted).
 */
class KeyClaim(private val take: () -> Unit, private val release: () -> Unit) {
    private var resumed = false
    private var focused = false
    private var up = false
    private var held = false

    fun resumed() {
        resumed = true
        settle()
    }

    fun paused() {
        resumed = false
        settle()
    }

    fun windowFocus(focused: Boolean) {
        this.focused = focused
        settle()
    }

    /** The service came up, or went: a service that came back has forgotten any claim. */
    fun serviceUp(up: Boolean) {
        this.up = up
        if (!up) held = false
        settle()
    }

    private fun settle() {
        val wanted = resumed && focused && up
        if (wanted == held) return
        held = wanted
        if (wanted) take() else release()
    }
}
