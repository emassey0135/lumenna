package com.extra.han.lumenna.device

import android.content.Context
import android.os.IInterface
import android.util.Log
import com.extra.han.lumenna.han.IHanBrailleInputService
import com.extra.han.lumenna.han.IHanBrailleInputServiceCallback

private const val TAG = "LumennaInput"

/**
 * The braille keys, through the `han_brailleinput` system service: its callback, registered while
 * the app is in front and released as it leaves, the way the device's own apps take it. The
 * chords never come as key events.
 *
 * The framework's copy of the interface shadows any we ship in the same package, and is blocked
 * to an app, so the service is reached through its binder alone: `getSystemService` gives a proxy
 * only as `IInterface`, and our own stubs (package `com.extra.han.lumenna.han`, the vendor's
 * descriptors by `@Descriptor`) talk to that binder. Chords go to [keys] on the binder thread, and
 * only while claimed. If the process dies its callback binder dies with it, and the service stops
 * calling it.
 */
class HanInputClient(private val context: Context, private val keys: (Int, Int, String?) -> Boolean) {
    private var service: IHanBrailleInputService? = null

    /** Whether the keys are ours: only between [claim] and [release]. */
    @Volatile private var claimed = false

    private val callback = object : IHanBrailleInputServiceCallback.Stub() {
        // A chord that arrives after the app left is the device's: answering "taken" here is what
        // would leave the person with a keyboard that does nothing.
        override fun onResponse(code: Int, value: Int, data: String?): Boolean = claimed && keys(code, value, data)
    }

    /** Takes the keys; does nothing if they are held already. */
    fun claim() {
        if (claimed) return
        val binder = runCatching {
            (context.getSystemService("han_brailleinput") as? IInterface)?.asBinder()
        }.getOrNull()
        if (binder == null) {
            Log.w(TAG, "no braille input service here: only key events will arrive")
            return
        }
        val input = IHanBrailleInputService.Stub.asInterface(binder)
        try {
            input.registerCallback(callback)
            service = input
            claimed = true
        } catch (error: Exception) {
            Log.w(TAG, "registerCallback refused", error)
        }
    }

    /**
     * Gives the keys back, safe to call at any time and any number of times. The flag goes first,
     * so that even if unregistering fails every chord is answered "not taken".
     */
    fun release() {
        claimed = false
        val held = service
        service = null
        try {
            held?.unregisterCallback(callback)
        } catch (error: Exception) {
            Log.w(TAG, "unregisterCallback failed", error)
        }
    }
}
