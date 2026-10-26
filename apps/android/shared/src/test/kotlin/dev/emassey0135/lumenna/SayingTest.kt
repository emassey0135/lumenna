package dev.emassey0135.lumenna

import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** D5 and D6: every announcement is said, in order, and the same sentence twice is said twice. */
class SayingTest {
    @Test
    fun announcementsComeInTurnEachHeldAndNoneDropped() = runBlocking {
        val flow = announcementFlow()
        val said = mutableListOf<Said>()
        val saying = launch(start = CoroutineStart.UNDISPATCHED) { sayInTurn(flow.take(20), hold = { 2L }) { said += it } }
        // Twenty at once, faster than they are said: the eight a buffer held were not enough.
        val texts = (1..20).map { "Change $it" }
        texts.forEach { assertTrue(flow.tryEmit(it)) }
        saying.join()
        assertEquals(texts, said.map { it.text })
        assertEquals((1L..20L).toList(), said.map { it.count })
    }

    @Test
    fun theSameSentenceTwiceIsAChangeALiveRegionHears() {
        val first = Said("Marked done", 1)
        val second = Said("Marked done", 2)
        assertNotEquals(first.description, second.description)
        assertEquals("Marked done", first.description)
        assertTrue(second.description.startsWith("Marked done"))
    }

    @Test
    fun aLongerSentenceIsHeldLongerWithinALimit() {
        assertTrue(Said.hold("Done") < Said.hold("Deep work starts in 5 minutes, 3 tasks assigned"))
        assertEquals(6_000L, Said.hold("x".repeat(1_000)))
    }
}
