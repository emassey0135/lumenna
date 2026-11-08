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
 * the service dies: on an update, or after the screen reader's.
 */
class HanOutputClient(private val context: Context) : Output {
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "lumenna-han-output") }
    private val token = Binder()
    private var service: IHanOutputService? = null
    private var claimed = false
    private var lastCells: Pair<ByteArray, Int>? = null

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
        attempt {
            new.register(token)
            if (claimed) new.setActiveOutputService(true)
            measure(new)
            lastCells?.let { (cells, cursor) -> sendCells(new, cells, cursor) }
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
        worker.execute {
            attempt { service?.unregister(token) }
            service = null
        }
        runCatching { context.unbindService(connection) }
    }

    /** Takes the display and speech while the app is in front; the screen reader yields them. */
    fun claim() = worker.execute {
        claimed = true
        attempt {
            service?.setActiveOutputService(true)
            lastCells?.let { (cells, cursor) -> service?.let { sendCells(it, cells, cursor) } }
        }
    }

    /** Gives the display back as the app leaves the front. */
    fun release() = worker.execute {
        claimed = false
        attempt {
            service?.setActiveOutputService(false)
            service?.stopTTS()
        }
    }

    override fun say(text: String) {
        if (text.isBlank()) return
        if (BuildConfig.DEBUG) Log.i(TAG, "say: $text")
        worker.execute {
            attempt {
                val output = HanOutput(text, text, null, "")
                service?.stopTTS()
                service?.displayAndPlayTTS(2, output, false, false, false, -1, -1, -1, -1, null)
            }
        }
    }

    override fun silence() = worker.execute { attempt { service?.stopTTS() } }

    override fun cells(cells: ByteArray, cursor: Int) {
        val copy = cells.copyOf()
        if (BuildConfig.DEBUG) Log.i(TAG, "cells: ${copy.size} " + copy.joinToString(" ") { (it.toInt() and 0xff).toString(2).reversed().mapIndexed { i, c -> if (c == '1') "${i + 1}" else "" }.joinToString("") } + " cursor $cursor")
        worker.execute {
            lastCells = copy to cursor
            service?.let { attempt { sendCells(it, copy, cursor) } }
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
            Log.w(TAG, "output call failed", error)
        }
    }
}
