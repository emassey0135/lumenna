package dev.emassey0135.braillesense.session

import android.app.Activity
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.view.KeyEvent
import dev.emassey0135.braillesense.braille.BrailleSettings
import dev.emassey0135.braillesense.braille.Louis
import dev.emassey0135.braillesense.braille.Translator
import dev.emassey0135.braillesense.input.Chords
import dev.emassey0135.braillesense.input.HanInputClient
import dev.emassey0135.braillesense.input.Key
import dev.emassey0135.braillesense.input.QwertyKeyboard
import dev.emassey0135.braillesense.output.BrailleDisplay
import dev.emassey0135.braillesense.output.HanOutputClient

/**
 * The device for one activity that speaks and writes the cells itself: the braille keys and the
 * QWERTY keyboard in, as [Key]s on the main thread, speech and cells out ([display]), liblouis in
 * the person's table ([translator]) and their Global Options ([settings]), each held only while
 * the activity is in front ([claim]).
 *
 * The activity hands it its lifecycle: [created] in `onCreate`, [resumed] in `onResume`, [paused]
 * first thing in `onPause`, [windowFocus] from `onWindowFocusChanged`, [keyEvent] from
 * `dispatchKeyEvent` (falling back to `super` when it returns false), and [destroyed] in
 * `onDestroy`. The keys reach an app only when its package is `com.extra.han.*`: the device's
 * screen reader yields them to such a package by its name alone.
 */
class BrailleSenseSession(
    activity: Activity,
    /** The app's version: liblouis' tables are copied out of the APK again only when it changes. */
    private val version: String,
    /** Whether what is said and shown is logged at info level, under [tag]: for a debug build. */
    trace: Boolean = false,
    tag: String = "BrailleSenseOutput",
) {
    private val context = activity.applicationContext
    private val main = Handler(Looper.getMainLooper())

    /** The person's Global Options, read again as the app comes to the front and as they change. */
    val settings = BrailleSettings(context)

    /** Speech and the cells, through the device's output service. */
    val output = HanOutputClient(context, trace = trace, tag = tag)

    /** liblouis in the person's table and grade, as [settings] last read them. */
    val translator: Translator = Louis.translator { settings.tables }

    /** What says, shows a line in the person's table, pans and routes. */
    val display = BrailleDisplay(output, translator)

    private val keyboard = QwertyKeyboard()

    /** A key from either keyboard, on the main thread, while the device is the app's. */
    var onKey: (Key) -> Unit = {}

    /** The device became the app's: say where the person is. */
    var onTaken: () -> Unit = {}

    /** The device is about to go back: stop anything that would write to it. */
    var onReleased: () -> Unit = {}

    /**
     * Global Options are about to be read again: read every edit box's text in the table in force
     * now (`TextBox.settle`), since the table may change.
     */
    var beforeSettings: () -> Unit = {}

    /** The braille table changed: write every edit box again in the new one (`TextBox.recell`), and show the line again. */
    var onTableChanged: () -> Unit = {}

    /** Global Options changed while the app was in front (some have a hot key that works there): draw the line again. */
    var onSettingsChanged: () -> Unit = {}

    /** Whether the keys and display are the app's: resumed and focused, and nothing drawn over it. */
    val claim: Claim = Claim(
        take = {
            readSettings()
            output.claim()
            input.claim()
            onTaken()
        },
        release = {
            onReleased()
            input.release()
            output.release()
        },
    )

    private val keys = KeyDelivery<Key>(claim, { main.post(it) }) { onKey(it) }

    private val input = HanInputClient(context) { code, value, data ->
        keys.deliver(Chords.decode(code, value, data, settings.eightDot, settings.doubleTapRoutes))
    }

    private val changed = object : ContentObserver(main) {
        override fun onChange(selfChange: Boolean) {
            readSettings()
            if (claim.holding) onSettingsChanged()
        }
    }

    /** Whether the device is the app's now. */
    val holding: Boolean get() = claim.holding

    private fun readSettings() {
        val before = settings.tables
        beforeSettings()
        settings.refresh()
        if (settings.tables != before) onTableChanged()
    }

    /** From `onCreate`: reads the settings, follows them, reaches the output service and installs liblouis' tables. */
    fun created() {
        claim.guardCrashes()
        settings.refresh()
        context.contentResolver.registerContentObserver(BrailleSettings.URI, true, changed)
        output.connect()
        Thread({ Louis.install(context, version) }, "louis-install").start()
    }

    /** From `onResume`. */
    fun resumed() = claim.resumed()

    /** First thing in `onPause`: whatever else leaving does, the device is the next app's. */
    fun paused() = claim.paused()

    /**
     * From `onWindowFocusChanged`: what is drawn over the app without pausing it (the task list, the
     * global options, an alert) takes the window's focus and nothing else, and the keys go with it.
     */
    fun windowFocus(hasFocus: Boolean) = claim.windowFocus(hasFocus)

    /** From `onDestroy`. */
    fun destroyed() {
        context.contentResolver.unregisterContentObserver(changed)
        input.release()
        output.disconnect()
    }

    /**
     * From `dispatchKeyEvent`: the QWERTY keyboard, which reaches the app as ordinary key events,
     * handed on as [onKey]. Keys the braille keyboard's service did not take arrive here too, with
     * no device behind them and the chord as their scan code: those are not typing, and are
     * swallowed. False for what the activity should hand to `super`.
     */
    fun keyEvent(event: KeyEvent): Boolean {
        if (!claim.holding) return false
        if (event.deviceId == -1 && event.scanCode != 0) return true
        val key = when (event.action) {
            KeyEvent.ACTION_DOWN -> keyboard.down(event.keyCode, event.metaState, event.getUnicodeChar(event.metaState))
            KeyEvent.ACTION_UP -> keyboard.up(event.keyCode)
            else -> null
        } ?: return false
        onKey(key)
        return true
    }
}
