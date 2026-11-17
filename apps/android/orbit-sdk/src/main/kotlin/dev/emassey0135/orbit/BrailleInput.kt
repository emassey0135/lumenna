package dev.emassey0135.orbit

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.os.Parcel
import android.util.Log
import com.orbit.liblouisservice.ILiblouisServiceInterface

/**
 * The person's braille input on the Speak: Orbit's liblouis service, translating in their own input
 * table, and the settings that say which table, which selection mode and which typing echo.
 *
 * The table and selection mode are in Orbit's signature-protected settings, read through [Root].
 * Each cell goes to the service as `0x8000 | dots`. The Player has no braille keys and no
 * translator: [translate] says nothing there. Ported from the Satellite launcher (`LiblouisClient`,
 * Apache-2.0, the same author's).
 *
 * The screen reader's G chord changes the selection mode in Orbit's settings, which tell no app but
 * Orbit's own (their provider needs Orbit's signature to watch, and the screen reader's settings
 * callback is only for its own settings). The settings app hands the new table to the liblouis
 * service, though, so [follow] asks the service which table it holds now (call it as a line is
 * reached and as the app comes back), and a change there reads the settings again. A table that
 * differs calls [changed] on the main thread.
 *
 * The screen reader's typing echo is read with them ([typingEcho]), since the reader takes no word
 * an app gives it: it echoes a word from the characters it was told, which for contracted braille
 * are not the word's. What cannot be read is [failed], once until a read succeeds again, and what
 * was read last stays.
 */
public class BrailleInput(context: Context) {
    private val context = context.applicationContext
    private val main = Handler(Looper.getMainLooper())

    /** The table read is another one now: what is shown is drawn again. On the main thread. */
    public var changed: () -> Unit = {}

    /** Orbit's settings could not be read (root refused, most likely). On the main thread. */
    public var failed: (RootFailure) -> Unit = {}

    @Volatile
    public var input: OrbitSettings.BrailleInput = OrbitSettings.BrailleInput(OrbitSettings.DEFAULT_TABLE, OrbitSettings.SelectionMode.CONTRACTED)
        private set

    /** Whether the person types contracted braille, by Orbit's selection mode. */
    public val contracted: Boolean get() = input.mode == OrbitSettings.SelectionMode.CONTRACTED

    /** Whether the person types computer braille, where dot 8 with a cell is typed. */
    public val computer: Boolean get() = input.mode == OrbitSettings.SelectionMode.COMPUTER

    @Volatile
    public var typingEcho: OrbitSettings.TypingEcho = OrbitSettings.TypingEcho.CHARACTERS
        private set

    /** What the liblouis service last said it holds. */
    @Volatile
    private var held: String? = null

    /** What has failed and been reported, until it is read again: the input, the echo. */
    private val reported = java.util.Collections.synchronizedSet(mutableSetOf<String>())

    private val link = ServiceLink(
        context,
        Intent("REMOTE_LIB_LOUIS_SERVICE_BIND")
            .setComponent(ComponentName(PACKAGE, "com.orbit.liblouisservice.LiblouisService")),
        { ILiblouisServiceInterface.Stub.asInterface(it) },
    )

    /** Whether this device has the service: the Speak does, the Player does not. */
    public val available: Boolean
        get() = runCatching {
            context.packageManager.getPackageInfo(PACKAGE, 0)
            true
        }.getOrDefault(false)

    /** Reaches the liblouis service and reads Orbit's settings; nothing on the Player. */
    public fun bind() {
        if (!available) return
        link.bind()
        refresh()
    }

    /** Lets the liblouis service go. */
    public fun unbind(): Unit = link.unbind()

    /** Whether the liblouis service is reached. */
    public val isReady: Boolean get() = link.isBound

    /** Reads the settings again if the liblouis service holds another table than it last did. */
    public fun follow() {
        if (!available) return
        Thread({
            readEcho()
            val now = serviceTable() ?: return@Thread
            val before = held
            held = now
            if (before != null && before != now) readInput()
        }, "orbit-braille-follow").start()
    }

    /** Reads the settings again, as Orbit said one changed ([OrbitReader.Listener.settingsChanged]). */
    public fun refresh() {
        if (!available) return
        Thread({
            readEcho()
            readInput()
        }, "orbit-braille-input").start()
    }

    /** Translates [cells] in the person's table at liblouis [mode]; null when it cannot say. */
    public fun translate(cells: List<Int>, mode: Int): String? =
        link.ask(null) { service ->
            val text = buildString { for (cell in cells) append((0x8000 or (cell and 0xFF)).toChar()) }
            service.translateBrailleToText(text, mode, input.table)?.toString()
        }

    /**
     * The table the liblouis service holds: its transaction 5 (`getCurrentInputTable`), answered with
     * a table whose first field is its file name. Called by number, the AIDL here declaring only the
     * calls before it.
     */
    private fun serviceTable(): String? = link.ask(null) { service ->
        val data = Parcel.obtain()
        val reply = Parcel.obtain()
        try {
            data.writeInterfaceToken(DESCRIPTOR)
            service.asBinder().transact(GET_CURRENT_INPUT_TABLE, data, reply, 0)
            reply.readException()
            if (reply.readInt() != 0) reply.readString() else null
        } finally {
            data.recycle()
            reply.recycle()
        }
    }

    private fun readInput() {
        val read = OrbitSettings.brailleInput().getOrElse { return report("input", it) }
        reported.remove("input")
        val before = input.table
        input = read
        Log.d(TAG, "braille input = $read, the service holding ${held ?: serviceTable()}")
        if (read.table != before) main.post { changed() }
    }

    private fun readEcho() {
        typingEcho = OrbitSettings.typingEcho().getOrElse { return report("echo", it) }
        reported.remove("echo")
    }

    private fun report(what: String, error: Throwable) {
        Log.w(TAG, "reading Orbit's settings ($what) failed: ${error.message}")
        if (!reported.add(what)) return
        val failure = error as? RootFailure ?: RootFailure(error.message ?: error.javaClass.simpleName)
        main.post { failed(failure) }
    }

    private companion object {
        const val TAG = "OrbitBraille"
        const val PACKAGE = "com.orbit.liblouisservice"
        const val DESCRIPTOR = "com.orbit.liblouisservice.ILiblouisServiceInterface"
        const val GET_CURRENT_INPUT_TABLE = 5
    }
}
