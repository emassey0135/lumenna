package io.github.emassey0135.lumenna

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.emassey0135.lumenna.app.App
import io.github.emassey0135.lumenna.app.Shell
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.Turn
import java.io.File
import java.util.UUID
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** The Crux app through BoltFFI, on a device: events in, turns out through the Kotlin shell. */
@RunWith(AndroidJUnit4::class)
class CruxAppTest {
    private val directory = File(InstrumentationRegistry.getInstrumentation().targetContext.cacheDir, "crux-${UUID.randomUUID()}")

    @After
    fun close() {
        directory.deleteRecursively()
    }

    private class Turns : Shell {
        val turns = LinkedBlockingQueue<Turn>()
        override fun turn(turn: Turn) {
            turns.put(turn)
        }

        /** The first turn that satisfies [check], waiting for each in turn. */
        fun until(check: (Turn) -> Boolean): Turn {
            while (true) {
                val turn = turns.poll(10, TimeUnit.SECONDS) ?: error("no turn came")
                if (check(turn)) return turn
            }
        }
    }

    @Test
    fun aTaskAddedThroughTheAppArrivesInATurnWithItsRowInHandAndItsReadbackSaid() {
        val shell = Turns()
        val app = App(directory.absolutePath, "android", shell)
        shell.until { it.view.ready }
        app.send(ShellEvent.QuickAdd("Buy milk tomorrow"))
        val added = shell.until { turn -> turn.view.list?.items.orEmpty().any { it.row.title == "Buy milk" } }
        val row = added.view.list!!.items.single { it.row.title == "Buy milk" }
        assertEquals(row.row.id, added.view.focus)
        assertTrue(added.said.joinToString().contains("Buy milk"))
        app.close()
    }
}
