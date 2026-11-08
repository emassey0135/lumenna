package com.extra.han.lumenna.device

import android.content.Context
import java.io.File

/** Text in cells, with what maps between them. */
class Translated(
    /** A byte per cell, bit 0 dot 1. */
    val cells: ByteArray,
    /** For each UTF-16 unit of the text, its cell. */
    val cellOf: IntArray,
    /** For each cell, the unit it came from. */
    val textAt: IntArray,
)

/** What the screens need of braille translation; the tests give their own. */
interface Translator {
    fun translate(text: String, cursor: Int = -1): Translated

    /** The text [cells] (a byte per cell) read back in the person's table, or null if refused. */
    fun back(cells: ByteArray): String?
}

/**
 * liblouis through its own small JNI, in the person's table ([table]), which `BrailleSettings`
 * reads from the device. Tables live in the app's files, copied once from the assets.
 */
object Louis {
    init {
        System.loadLibrary("lumenna-louis")
    }

    @JvmStatic external fun setDataPath(path: String)
    @JvmStatic external fun translate(tables: String, text: String, cursor: Int): Array<Any>?
    @JvmStatic external fun backTranslate(tables: String, cells: ByteArray): String?

    private val installed = java.util.concurrent.CountDownLatch(1)

    /** Copies the tables out of the assets, once per version: liblouis reads them as files. */
    fun install(context: Context, version: String) {
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
    fun translator(table: () -> String): Translator = object : Translator {
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

        override fun back(cells: ByteArray): String? = synchronized(Louis) {
            installed.await()
            backTranslate(table(), cells)
        }
    }
}
