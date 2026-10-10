package io.github.emassey0135.lumenna

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.InputStream
import java.io.OutputStream
import java.util.concurrent.LinkedBlockingQueue
import java.util.UUID
import kotlin.concurrent.thread
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The watch's own link, with two stores joined by pipes where the Data Layer's channel would
 * be: the streams, the callbacks into the core, and pairing without words, on a device. The
 * Data Layer itself needs a phone and a watch paired through Google's Wear OS app.
 */
@RunWith(AndroidJUnit4::class)
class WatchLinkTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext
    private val directories = mutableListOf<File>()

    private fun store(): Core = Core(File(context.cacheDir, "link-${UUID.randomUUID()}").also { directories += it })

    @After
    fun close() {
        directories.forEach { it.deleteRecursively() }
    }

    @Test
    fun theFirstLinkPairsThePhoneAndTheWatchWithoutWordsAndSyncsATask() {
        val (phone, watch) = store() to store()
        watch.lumenna.addTask("written on the watch")
        // Each side's output is the other's input, as a channel's two ends.
        val (toWatch, watchReads) = pipe()
        val (toPhone, phoneReads) = pipe()
        var watchResult: Result<io.github.emassey0135.lumenna.core.LinkSynced>? = null
        val other = thread {
            watchResult = runCatching { watch.lumenna.syncOverLink(WatchLink.ChannelStreams(watchReads, toPhone), "Watch", "wearos") }
        }
        val phoneResult = phone.lumenna.syncOverLink(WatchLink.ChannelStreams(phoneReads, toWatch), "Phone", "android")
        other.join(30_000)

        assertTrue("the first link paired them", phoneResult.paired)
        assertTrue(watchResult!!.getOrThrow().paired)
        assertEquals(listOf("written on the watch"), phone.lumenna.listTasks("").rows.map { it.title })
        assertEquals(setOf("Phone", "Watch"), phone.lumenna.devices().devices.map { it.name }.toSet())
        assertEquals("wearos", phone.lumenna.devices().devices.first { it.name == "Watch" }.platform)
    }

    /**
     * A one-way pipe that any thread may write and read, as a channel's streams may be:
     * Java's own pipe breaks once the thread that last wrote has gone, and the core writes
     * from a pool.
     */
    private fun pipe(): Pair<OutputStream, InputStream> {
        val chunks = LinkedBlockingQueue<ByteArray>()
        val output = object : OutputStream() {
            override fun write(b: Int) = write(byteArrayOf(b.toByte()))
            override fun write(b: ByteArray, off: Int, len: Int) {
                chunks.put(b.copyOfRange(off, off + len))
            }
            override fun close() = chunks.put(ByteArray(0))
        }
        val input = object : InputStream() {
            private var current = ByteArray(0)
            private var at = 0
            private var ended = false
            override fun read(): Int {
                val one = ByteArray(1)
                return if (read(one, 0, 1) < 0) -1 else one[0].toInt() and 0xff
            }
            override fun read(b: ByteArray, off: Int, len: Int): Int {
                if (ended) return -1
                if (at == current.size) {
                    current = chunks.take()
                    at = 0
                    if (current.isEmpty()) {
                        ended = true
                        return -1
                    }
                }
                val n = minOf(len, current.size - at)
                current.copyInto(b, off, at, at + n)
                at += n
                return n
            }
        }
        return output to input
    }
}
