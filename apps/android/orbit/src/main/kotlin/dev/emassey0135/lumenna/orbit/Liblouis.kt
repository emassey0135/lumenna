package dev.emassey0135.lumenna.orbit

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.util.Log
import com.orbit.liblouisservice.ILiblouisServiceInterface
import dev.emassey0135.lumenna.braille.ServiceLink
import java.io.BufferedReader
import java.io.InputStreamReader
import java.nio.charset.StandardCharsets

/**
 * Orbit's braille translator, in the person's own input table.
 *
 * The table and selection mode are in Orbit's signature-protected settings, read through root as
 * Orbit's own apps read them. Each cell goes to the service as `0x8000 | dots`. The Player has
 * no braille keys and no translator: [translate] says nothing there. Ported from the Satellite
 * launcher (`LiblouisClient`, Apache-2.0, the same author's).
 *
 * The screen reader's G chord changes the selection mode in Orbit's settings, which tell no app but
 * Orbit's own (their provider needs Orbit's signature to watch, and the screen reader's settings
 * callback is only for its own settings). The settings app hands the new table to the liblouis
 * service, though, so [follow] asks the service which table it holds now (as a line is reached and
 * the app comes back), and a change there reads the table again through root. A table read that
 * differs calls [changed] on the main thread.
 */
class Liblouis(context: Context) {
    private val context = context.applicationContext

    @Volatile
    private var table = DEFAULT_TABLE

    /** The table read is another one now: what is shown is drawn again. On the main thread. */
    var changed: () -> Unit = {}

    private val main = Handler(Looper.getMainLooper())

    /** What the liblouis service last said it holds. */
    @Volatile
    private var held: String? = null

    private val link = ServiceLink(
        context,
        Intent("REMOTE_LIB_LOUIS_SERVICE_BIND")
            .setComponent(ComponentName("com.orbit.liblouisservice", "com.orbit.liblouisservice.LiblouisService")),
        { ILiblouisServiceInterface.Stub.asInterface(it) },
    )

    private val available: Boolean
        get() = runCatching {
            context.packageManager.getPackageInfo("com.orbit.liblouisservice", 0)
            true
        }.getOrDefault(false)

    fun bind() {
        if (!available) return
        link.bind()
        refresh()
    }

    fun unbind() = link.unbind()

    /** Reads the table again if the liblouis service holds another than it last did. Off the main thread. */
    fun follow() {
        if (!available) return
        Thread({
            val now = serviceTable() ?: return@Thread
            val before = held
            held = now
            if (before != null && before != now) readTable()
        }, "lumenna-braille-follow").start()
    }

    /**
     * The table the liblouis service holds: its transaction 5 (`getCurrentInputTable`), answered with
     * a table whose first field is its file name. Called by number, the AIDL here declaring only the
     * calls before it.
     */
    private fun serviceTable(): String? = link.ask(null) { service ->
        val data = android.os.Parcel.obtain()
        val reply = android.os.Parcel.obtain()
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

    /** Reads the person's table again, as Orbit said a setting changed. Off the main thread. */
    fun refresh() {
        if (!available) return
        Thread({ readTable() }, "lumenna-braille-table").start()
    }

    val isReady: Boolean get() = link.isBound

    private fun readTable() {
        val commands = KEYS.joinToString("") { "echo @@$it; content query --uri $SETTINGS$it; " }
        val output = RootQuery.run(commands) ?: return
        val values = parseMarked(output)
        val mode = values["key_selection_mode"]?.lowercase()
        val key = when {
            mode == null -> "key_contracted_braille_input_table"
            "uncontract" in mode -> "key_uncontracted_braille_input_table"
            "computer" in mode -> "key_computer_braille_input_table"
            else -> "key_contracted_braille_input_table"
        }
        val before = table
        values[key]?.takeIf { it.isNotEmpty() }?.let { table = it }
        Log.d(TAG, "braille input table = $table, the service holding ${held ?: serviceTable()}")
        if (table != before) main.post { changed() }
    }

    /** Translates [cells] in the person's table at liblouis [mode]; null when it cannot say. */
    fun translate(cells: List<Int>, mode: Int): String? =
        link.ask(null) { service ->
            val input = buildString { for (cell in cells) append((0x8000 or (cell and 0xFF)).toChar()) }
            service.translateBrailleToText(input, mode, table)?.toString()
        }

    private companion object {
        const val TAG = "LumennaOrbit"
        const val SETTINGS = "content://com.orbit.settings.provider/preferences/"
        const val DEFAULT_TABLE = "en-ueb-g2.ctb"
        const val DESCRIPTOR = "com.orbit.liblouisservice.ILiblouisServiceInterface"
        const val GET_CURRENT_INPUT_TABLE = 5
        val KEYS = listOf(
            "key_selection_mode",
            "key_contracted_braille_input_table",
            "key_uncontracted_braille_input_table",
            "key_computer_braille_input_table",
        )

        fun parseMarked(output: String): Map<String, String> {
            val values = mutableMapOf<String, String>()
            for (part in output.split("@@")) {
                val newline = part.indexOf('\n')
                if (newline < 0) continue
                RootQuery.valueField(part, newline)?.let { values[part.substring(0, newline).trim()] = it }
            }
            return values
        }
    }
}

/** Root helpers, through `su`, as the Satellite launcher has them (`RootQuery`, Apache-2.0). */
object RootQuery {
    /** The `value=` field of a single-row content query from [from] on; null if there is none. */
    fun valueField(output: String, from: Int): String? {
        val at = output.indexOf("value=", from)
        if (at < 0) return null
        var end = output.indexOf(", type=", at)
        if (end < 0) end = output.indexOf('\n', at)
        if (end < 0) end = output.length
        return output.substring(at + 6, end).trim()
    }

    /** Runs [command] as root; what it printed, or null if `su` could not. Off the main thread. */
    fun run(command: String): String? {
        var process: Process? = null
        return try {
            process = ProcessBuilder("su", "-c", command).redirectErrorStream(true).start()
            val output = BufferedReader(InputStreamReader(process.inputStream, StandardCharsets.UTF_8)).use { it.readText() }
            process.waitFor()
            output
        } catch (error: Exception) {
            Log.w("LumennaOrbit", "root command failed: ${error.message}")
            null
        } finally {
            process?.destroy()
        }
    }
}
