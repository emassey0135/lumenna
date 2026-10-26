package dev.emassey0135.lumenna

import org.junit.Assert.assertEquals
import org.junit.Test

/** D1: what follows the store looks again only when its heads move, not for every turn. */
class HeadsTest {
    private var heads = "a"
    private var moves = 0
    private val waiting = ArrayDeque<Runnable>()
    private val looking = Heads(read = { heads }, run = { waiting.addLast(it) }, moved = { moves++ })

    private fun runAll() {
        while (waiting.isNotEmpty()) waiting.removeFirst().run()
    }

    @Test
    fun turnsThatChangeNothingInTheStoreMoveNothing() {
        repeat(5) {
            looking.look()
            runAll()
        }
        assertEquals(0, moves)
    }

    @Test
    fun aWriteMovesWhatFollowsTheStoreOnce() {
        looking.look()
        runAll()
        heads = "b"
        looking.look()
        runAll()
        looking.look()
        runAll()
        assertEquals(1, moves)
    }

    @Test
    fun aRunOfTurnsIsOneLookAndTheLastWriteIsNotMissed() {
        looking.look()
        runAll()
        heads = "b"
        repeat(10) { looking.look() }
        assertEquals(1, waiting.size)
        runAll()
        // A write while the look was under way is looked for once more.
        looking.look()
        heads = "c"
        runAll()
        looking.look()
        runAll()
        assertEquals(2, moves)
    }

    @Test
    fun aStoreThatCannotSayMovesNothing() {
        val silent = Heads(read = { null }, run = { it.run() }, moved = { moves++ })
        repeat(3) { silent.look() }
        assertEquals(0, moves)
    }
}
