package dev.emassey0135.braillesense.braille

import android.content.Context
import java.io.File

/** Text in cells, with what maps between them. */
public class Translated(
    /** A byte per cell, bit 0 dot 1. */
    public val cells: ByteArray,
    /** For each UTF-16 unit of the text, its cell. */
    public val cellOf: IntArray,
    /** For each cell, the unit it came from. */
    public val textAt: IntArray,
)

/** Braille translation in the person's table, as the display and the edit box need it; tests give their own. */
public interface Translator {
    /** [text] in the person's table, the cursor at [cursor] in it carried into the cells (-1 for none). */
    public fun translate(text: String, cursor: Int = -1): Translated

    /** [text] cell for cell, as Braille ASCII: the device's role symbols, signs and hot keys. */
    public fun computer(text: String): ByteArray

    /** The text [cells] (a byte per cell) read back in the person's table, or null if refused. */
    public fun back(cells: ByteArray): String?
}

/**
 * liblouis through its own small JNI, in the person's table ([translator]), which `BrailleSettings`
 * reads from the device. Tables live in the app's files, copied once from the assets.
 */
public object Louis {
    init {
        System.loadLibrary("braillesense-louis")
    }

    /** liblouis' own call: where its tables are. The SDK calls it as it installs them. */
    @JvmStatic public external fun setDataPath(path: String)
    /** liblouis' own forward translation of [text] in [tables]: the cells and the cursor's place, or null if refused. [translator] is the way to use it. */
    @JvmStatic public external fun translate(tables: String, text: String, cursor: Int): Array<Any>?
    /** liblouis' own back-translation of [cells] in [tables], or null if refused. */
    @JvmStatic public external fun backTranslate(tables: String, cells: ByteArray): String?

    private val installed = java.util.concurrent.CountDownLatch(1)
    private const val COMPUTER_TABLE = "en-us-comp8.ctb"

    /** Copies the tables out of the assets, once per version: liblouis reads them as files. */
    public fun install(context: Context, version: String) {
        try {
            copyTables(context, version)
        } finally {
            installed.countDown()
        }
    }

    private fun copyTables(context: Context, version: String) {
        val data = File(context.filesDir, "louis")
        val marker = File(data, ".installed-$version")
        if (!marker.exists()) {
            data.deleteRecursively()
            val target = File(data, "liblouis/tables")
            target.mkdirs()
            val assets = context.assets
            assets.list("louis/liblouis/tables").orEmpty().forEach { name ->
                assets.open("louis/liblouis/tables/$name").use { input ->
                    File(target, name).outputStream().use { input.copyTo(it) }
                }
            }
            marker.writeText(version)
        }
        setDataPath(data.absolutePath)
    }

    /** A translator in the table [table] gives at the moment of each call, so a change of settings takes hold. */
    public fun translator(table: () -> String): Translator = object : Translator {
        override fun translate(text: String, cursor: Int): Translated = synchronized(Louis) {
            // The tables are copied out once, off the main thread; only the first line waits.
            installed.await()
            val result = translate(table(), text, cursor)
            if (result != null) {
                Translated(result[0] as ByteArray, result[1] as IntArray, result[2] as IntArray)
            } else {
                Translated(ByteArray(text.length), IntArray(text.length) { it }, IntArray(text.length) { it })
            }
        }

        override fun computer(text: String): ByteArray = synchronized(Louis) {
            installed.await()
            translate(COMPUTER_TABLE, text, -1)?.get(0) as ByteArray? ?: ByteArray(text.length)
        }

        override fun back(cells: ByteArray): String? = synchronized(Louis) {
            installed.await()
            backTranslate(table(), cells)
        }
    }
}
