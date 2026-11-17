package dev.emassey0135.orbit

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ApplicationInfo

/**
 * Orbit's keys pressed over adb, for trying an app without the device in hand: nothing else can
 * press the keypad remotely. Only a debuggable build listens. A key is a broadcast of [action] with
 * the keypad callback's fields:
 *
 * ```
 * adb shell am broadcast -a <action> --es name DOWN --ei action 0 --ez long false --ez phone false
 * ```
 *
 * `name` is the key's name as the keypad sends it (what [KeyDecoder] reads), `action` 0 for down
 * and 1 for up, `long` a long press, and `phone` the Player's phone keypad. Wake a sleeping device
 * first (`input keyevent KEYCODE_WAKEUP`), or the window has no focus and nothing is claimed.
 */
public class DebugKeys(
    private val context: Context,
    /** The broadcast's action: the app's own, so two apps do not take each other's keys. */
    private val action: String,
    /** A key, as [OrbitReader.Listener.key] hands one over. */
    private val key: (event: KeyEvent, phoneKeypad: Boolean) -> Unit,
) {
    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            key(
                KeyEvent(intent.getStringExtra("name").orEmpty(), intent.getIntExtra("action", 0), intent.getBooleanExtra("long", false)),
                intent.getBooleanExtra("phone", false),
            )
        }
    }

    private var listening = false

    /** Whether this build takes keys over adb: only a debuggable one. */
    public val debuggable: Boolean get() = context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0

    /** Starts listening, in a debuggable build. */
    public fun start() {
        if (listening || !debuggable) return
        context.registerReceiver(receiver, IntentFilter(action))
        listening = true
    }

    /** Stops listening. */
    public fun stop() {
        if (!listening) return
        context.unregisterReceiver(receiver)
        listening = false
    }
}
