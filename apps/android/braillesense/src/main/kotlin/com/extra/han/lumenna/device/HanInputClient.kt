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
 * descriptors by `@Descriptor`) talk to that binder. [Key]s go to [keys] on the binder thread.
 */
class HanInputClient(private val context: Context, private val keys: (Int, Int, String?) -> Boolean) {
    private var service: IHanBrailleInputService? = null

    private val callback = object : IHanBrailleInputServiceCallback.Stub() {
        override fun onResponse(code: Int, value: Int, data: String?): Boolean = keys(code, value, data)
    }

    /** Whether the service is here at all: not on anything but a BrailleSense. */
    val available: Boolean get() = service != null

    fun claim() {
        val binder = runCatching {
            (context.getSystemService("han_brailleinput") as? IInterface)?.asBinder()
        }.getOrNull()
        if (binder == null) {
            Log.w(TAG, "no braille input service here: only key events will arrive")
            return
        }
        service = IHanBrailleInputService.Stub.asInterface(binder).also {
            runCatching { it.registerCallback(callback) }.onFailure { error -> Log.w(TAG, "registerCallback refused", error) }
        }
    }

    fun release() {
        runCatching { service?.unregisterCallback(callback) }
        service = null
    }
}
