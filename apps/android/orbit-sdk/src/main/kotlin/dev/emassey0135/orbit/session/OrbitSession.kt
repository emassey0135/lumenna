package dev.emassey0135.orbit.session

import android.app.Activity
import android.content.Context
import android.os.Handler
import android.os.Looper
import dev.emassey0135.orbit.BrailleInput
import dev.emassey0135.orbit.Claim
import dev.emassey0135.orbit.DebugKeys
import dev.emassey0135.orbit.KeyDelivery
import dev.emassey0135.orbit.KeyEvent
import dev.emassey0135.orbit.OrbitReader
import dev.emassey0135.orbit.RootFailure

/**
 * The device for one activity that speaks for itself on an Orbit Speak or Player: Orbit's screen
 * reader ([reader]), which voices what the app describes and hands it the keypad; the Speak's
 * braille input ([braille]); and the keypad's keys, on the main thread ([onKey]), only while the
 * activity is in front ([claim]).
 *
 * The activity hands it its lifecycle, as the BrailleSense SDK's `BrailleSenseSession` is handed
 * one: [created] in `onCreate`, [resumed] in `onResume`, [paused] first thing in `onPause`,
 * [windowFocus] from `onWindowFocusChanged`, and [destroyed] in `onDestroy`. Orbit sends the keypad
 * to an app only when it speaks for itself: Satellite's hook does that for an app whose manifest
 * has the `orbit.self_voicing` meta-data.
 */
public class OrbitSession(
    activity: Activity,
    /** What Orbit says the app is called as it opens and as it comes back. */
    private val appName: String,
    /**
     * The action of a broadcast that presses Orbit's keys over adb ([DebugKeys]), only in a
     * debuggable build; null takes none. Name it for your own app, so two apps do not take each
     * other's keys.
     */
    debugKeys: String? = null,
) : OrbitReader.Listener {
    private val main = Handler(Looper.getMainLooper())

    /** The application's context: for the clipboard, and anything else that outlives the activity. */
    public val context: Context = activity.applicationContext

    /** Orbit's screen reader: what the app describes, Orbit says in the person's voice and verbosity. */
    public val reader: OrbitReader = OrbitReader(activity, this)

    /** The Speak's braille input in the person's table; not [BrailleInput.available] on the Player. */
    public val braille: BrailleInput = BrailleInput(activity)

    /** Whether the keypad is the app's: resumed, focused, and Orbit's reader up. */
    public val claim: Claim get() = reader.claim

    /** Whether the keypad is the app's now. */
    public val holding: Boolean get() = claim.holding

    /** A key from the keypad, on the main thread, in order, while the keypad is the app's; `phoneKeypad` when it is the Player's. */
    public var onKey: (event: KeyEvent, phoneKeypad: Boolean) -> Unit = { _, _ -> }

    /** Orbit's reader is ready, for the first time or again after it restarted, on the main thread: describe the screen. */
    public var onReady: () -> Unit = {}

    /** An Orbit setting changed, on the main thread; the braille settings are being read again already. */
    public var onSettingsChanged: () -> Unit = {}

    /** The braille table the person types in changed (Orbit's G chord): show again what was typed. On the main thread. */
    public var onTableChanged: () -> Unit = {}

    /** Orbit's settings could not be read (root refused, most likely), once until a read succeeds. On the main thread. */
    public var onRootFailed: (RootFailure) -> Unit = {}

    private val keys = KeyDelivery<Pair<KeyEvent, Boolean>>(claim, { main.post(it) }) { (event, phoneKeypad) -> onKey(event, phoneKeypad) }

    private val adbKeys: DebugKeys? = debugKeys?.let { action -> DebugKeys(activity, action) { event, phone -> keys.deliver(event to phone) } }

    /** Whether this build takes keys over adb: a debug broadcast was named and the build is debuggable. */
    public val takesDebugKeys: Boolean get() = adbKeys?.debuggable == true

    init {
        braille.changed = { onTableChanged() }
        braille.failed = { onRootFailed(it) }
    }

    /** From `onCreate`: guards the keypad against a crash and reaches Orbit's reader and braille input. */
    public fun created() {
        claim.guardCrashes()
        adbKeys?.start()
        reader.bind()
        braille.bind()
    }

    /** From `onResume`: the braille table is looked at again, since Orbit tells no app of its G chord. */
    public fun resumed() {
        braille.follow()
        claim.resumed()
    }

    /** First thing in `onPause`: whatever else leaving does, the keypad is the next app's. */
    public fun paused(): Unit = claim.paused()

    /**
     * From `onWindowFocusChanged`: what is drawn over the app without pausing it takes the window's
     * focus and nothing else, and the keypad goes with it.
     */
    public fun windowFocus(hasFocus: Boolean): Unit = claim.windowFocus(hasFocus)

    /** From `onDestroy`. */
    public fun destroyed() {
        adbKeys?.stop()
        reader.unbind()
        braille.unbind()
    }

    // ---- what Orbit tells the app, on binder threads ------------------------------------------

    /** Orbit's reader is ready: the app is said as opened, then [onReady] on the main thread. */
    override fun readerReady() {
        main.post {
            reader.appOpened(appName, "")
            onReady()
        }
    }

    /** A key: taken only while the keypad is the app's, then [onKey] on the main thread. */
    override fun key(event: KeyEvent, phoneKeypad: Boolean): Boolean = keys.deliver(event to phoneKeypad)

    /** An Orbit setting changed: the braille settings are read again, and [onSettingsChanged] is called. */
    override fun settingsChanged() {
        braille.refresh()
        main.post { onSettingsChanged() }
    }
}
