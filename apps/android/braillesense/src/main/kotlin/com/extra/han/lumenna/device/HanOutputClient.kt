package com.extra.han.lumenna.device

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Binder
import android.os.IBinder
import android.util.Log
import com.extra.han.lumenna.BuildConfig
import com.extra.han.lumenna.han.HanOutput
import com.extra.han.lumenna.han.IHanOutputService
import java.util.concurrent.Executors

private const val TAG = "LumennaOutput"

/**
 * What the screens send out: speech and cells. A seam so the tests can read what was said.
 */
interface Output {
    /** Says [text] now, after stopping whatever is being said. */
    fun say(text: String)

    /** Stops speech. */
    fun silence()

    /** Puts [cells] on the display, the cursor at [cursor] (-1 for none). */
    fun cells(cells: ByteArray, cursor: Int)

    /** How many cells the display has, once known: the layout of panning waits on it. */
    val width: Int
}

/**
 * `IHanOutputService`, bound by explicit intent, every call on one ordered worker, so a slow
 * service never holds the key that asked for it and what is said arrives in order. Rebound when
 * the service dies: on an update, or after the screen reader's. [register]ed with a token of ours
 * for the service to watch, so a process that dies does not leave it holding a client.
 */
class HanOutputClient(private val context: Context) : Output {
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "lumenna-han-output") }
    private val token = Binder()
    @Volatile private var service: IHanOutputService? = null

    /** Whether speech and the cells are ours. Nothing is sent while they are not. */
    @Volatile private var claimed = false

    /** Held across the calls that touch the display, so that nothing lands after [release] returns. */
    private val claim = Any()

    /** What is on the cells, to put back when the display comes back or is taken again. */
    @Volatile private var lastCells: Pair<ByteArray, Int>? = null

    @Volatile override var width: Int = 0
        private set

    private val intent = Intent("com.jawon.han.output.ACTION_HAN_OUTPUT")
        .setClassName("com.jawon.han.accessibility", "com.jawon.han.output.HanOutputService")

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder) {
            worker.execute { attached(IHanOutputService.Stub.asInterface(binder)) }
        }

        // The service went: another will come through this same connection.
        override fun onServiceDisconnected(name: ComponentName?) {
            worker.execute { service = null }
        }

        // The binding itself is finished: it must be made again.
        override fun onBindingDied(name: ComponentName?) {
            worker.execute { service = null }
            runCatching { context.unbindService(this) }
            bind()
        }
    }

    private fun attached(new: IHanOutputService) {
        service = new
        attempt { new.register(token) }
        take()
    }

    /** On the worker: makes the claim good, if it still stands. */
    private fun take() = synchronized(claim) {
        val output = service
        if (!claimed || output == null) return@synchronized
        attempt {
            output.setActiveOutputService(true)
            measure(output)
            lastCells?.let { (cells, cursor) -> sendCells(output, cells, cursor) }
        }
    }

    /** Asks the service how wide the line is: it wraps a long word at its own width. */
    private fun measure(output: IHanOutputService) {
        if (width > 0) return
        val probe = 1600
        output.displayBrailleWordWrap("x".repeat(probe), true)
        // The service lays the text out on a thread of its own.
        var lines = 0
        repeat(40) {
            lines = output.getBrailleTotalCellLine()
            if (lines > 1) return@repeat
            Thread.sleep(50)
        }
        width = if (lines > 0) probe / lines else 40
        Log.i(TAG, "display is $width cells, $lines lines")
    }

    private fun bind() {
        if (!context.bindService(intent, connection, Context.BIND_AUTO_CREATE)) {
            Log.w(TAG, "no output service here")
        }
    }

    fun connect() = bind()

    fun disconnect() {
        release()
        worker.execute {
            attempt { service?.unregister(token) }
            service = null
        }
        runCatching { context.unbindService(connection) }
    }

    /** Takes the display and speech while the app is in front; the screen reader yields them. */
    fun claim() {
        claimed = true
        worker.execute { take() }
    }

    /**
     * Gives the display and speech back, now and on the caller's thread, safe to call at any time
     * and any number of times. Not queued on the worker: the next app claims the display as it
     * comes to the front, and a release that landed after its claim would take the display from it.
     */
    fun release() = synchronized(claim) {
        claimed = false
        val output = service
        attempt { output?.setActiveOutputService(false) }
        attempt { output?.stopTTS() }
    }

    override fun say(text: String) {
        if (text.isBlank() || !claimed) return
        if (BuildConfig.DEBUG) Log.i(TAG, "say: $text")
        worker.execute {
            synchronized(claim) {
                if (!claimed) return@execute
                attempt {
                    service?.stopTTS()
                    service?.displayAndPlayTTS(2, HanOutput(text, text, null, ""), false, false, false, -1, -1, -1, -1, null)
                }
            }
        }
    }

    override fun silence() = worker.execute {
        synchronized(claim) { if (claimed) attempt { service?.stopTTS() } }
    }

    override fun cells(cells: ByteArray, cursor: Int) {
        val copy = cells.copyOf()
        // Kept while away, so coming back shows what is true now; sent only while the display is ours.
        lastCells = copy to cursor
        if (!claimed) return
        if (BuildConfig.DEBUG) Log.i(TAG, "cells: ${copy.size} " + copy.joinToString(" ") { (it.toInt() and 0xff).toString(2).reversed().mapIndexed { i, c -> if (c == '1') "${i + 1}" else "" }.joinToString("") } + " cursor $cursor")
        worker.execute {
            synchronized(claim) {
                if (!claimed) return@execute
                service?.let { attempt { sendCells(it, copy, cursor) } }
            }
        }
    }

    private fun sendCells(output: IHanOutputService, cells: ByteArray, cursor: Int) {
        output.displayRawBraille(cells)
        output.brailleCursorPos(cursor)
    }

    private inline fun attempt(call: () -> Unit) {
        try {
            call()
        } catch (error: Exception) {
            // A dead service throws here too: a release must carry on regardless.
            Log.w(TAG, "output call failed", error)
        }
    }
}
