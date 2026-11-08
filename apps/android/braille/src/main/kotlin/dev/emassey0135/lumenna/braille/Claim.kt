package dev.emassey0135.lumenna.braille

import android.util.Log

/**
 * When a device's keys and display are the app's: while it is resumed and its window has focus,
 * and at no other moment. Android pauses an activity for another app, but something drawn over it
 * (a task list, global options, a system dialog, the notification shade) takes only the window's
 * focus, and whatever drives that window needs the keys, or the person is left with a keyboard
 * that does nothing.
 *
 * [release] runs first thing on losing either, on the caller's thread, and once for each [take]:
 * a release queued behind other work could land after the next app has claimed the device, and
 * take it from that app. A release that throws leaves the claim given up all the same.
 *
 * With [needsService] the claim also waits for [serviceUp], since none can be made before a
 * service is there; a service that went has forgotten the claim, so nothing is released to it.
 */
class Claim(
    private val take: () -> Unit,
    private val release: () -> Unit,
    private val needsService: Boolean = false,
) {
    private var resumed = false
    private var focused = false
    private var up = false

    /** Whether the keys and display are held now. Readable from a key's binder thread. */
    @Volatile var holding = false
        private set

    @Synchronized fun resumed() {
        resumed = true
        settle()
    }

    /** Call before anything else leaving does. */
    @Synchronized fun paused() {
        resumed = false
        settle()
    }

    @Synchronized fun windowFocus(focused: Boolean) {
        this.focused = focused
        settle()
    }

    /** The service came up, or went. */
    @Synchronized fun serviceUp(up: Boolean) {
        this.up = up
        if (!up) holding = false
        settle()
    }

    private fun settle() {
        val wanted = resumed && focused && (up || !needsService)
        if (wanted == holding) return
        holding = wanted
        if (wanted) take() else giveBack()
    }

    private fun giveBack() {
        try {
            release()
        } catch (error: Exception) {
            Log.w(TAG, "releasing the device failed", error)
        }
    }

    /**
     * A crash must not leave the device held, so one releases it before the process goes. A
     * native crash skips this; the service then sees the binders it was given die.
     */
    fun guardCrashes() {
        val crashed = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            holding = false
            giveBack()
            crashed?.uncaughtException(thread, error)
        }
    }

    private companion object {
        const val TAG = "LumennaClaim"
    }
}

/**
 * A decoded key handed from the binder thread it arrived on to the app, on the app's thread. A key
 * is taken only while [claim] holds, and looked at again when it runs, since the app may have left
 * while it waited: what is not taken goes on to the device.
 */
class KeyDelivery<K : Any>(
    private val claim: Claim,
    private val post: (() -> Unit) -> Unit,
    private val handle: (K) -> Unit,
) {
    /** Whether the key was taken: answered to the service that asked. */
    fun deliver(key: K?): Boolean {
        if (key == null || !claim.holding) return false
        post { if (claim.holding) handle(key) }
        return true
    }
}
