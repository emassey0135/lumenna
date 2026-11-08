package dev.emassey0135.lumenna.braille

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ClaimTest {
    private val seen = mutableListOf<String>()
    private val claim = Claim(take = { seen += "take" }, release = { seen += "give" })

    private fun inFront() {
        claim.resumed()
        claim.windowFocus(true)
    }

    @Test fun nothingIsTakenUntilResumedAndFocused() {
        claim.resumed()
        assertFalse(claim.holding)
        assertEquals(emptyList<String>(), seen)
        claim.windowFocus(true)
        assertTrue(claim.holding)
        assertEquals(listOf("take"), seen)
    }

    @Test fun pausingGivesBackAtOnce() {
        inFront()
        claim.paused()
        assertFalse(claim.holding)
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun somethingDrawnOverTheAppGivesBackWithoutAPause() {
        inFront()
        claim.windowFocus(false)
        assertFalse(claim.holding)
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun comingBackFromSomethingDrawnOverItTakesAgain() {
        inFront()
        claim.windowFocus(false)
        claim.windowFocus(true)
        assertTrue(claim.holding)
        assertEquals(listOf("take", "give", "take"), seen)
    }

    @Test fun losingBothGivesBackOnce() {
        inFront()
        claim.windowFocus(false)
        claim.paused()
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun focusWithoutResumeTakesNothing() {
        claim.windowFocus(true)
        assertFalse(claim.holding)
        assertEquals(emptyList<String>(), seen)
    }

    @Test fun aReleaseThatFailsStillGivesUpTheClaim() {
        val failing = Claim(take = {}, release = { throw IllegalStateException("dead") })
        failing.resumed()
        failing.windowFocus(true)
        failing.paused()
        assertFalse(failing.holding)
    }

    @Test fun releasingTwiceGivesBackOnce() {
        inFront()
        claim.paused()
        claim.paused()
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun aClaimNeedingAServiceWaitsForIt() {
        val waiting = Claim(take = { seen += "take" }, release = { seen += "give" }, needsService = true)
        waiting.resumed()
        waiting.windowFocus(true)
        assertFalse(waiting.holding)
        waiting.serviceUp(true)
        assertEquals(listOf("take"), seen)
    }

    @Test fun aServiceThatWentHasNothingReleasedToIt() {
        val waiting = Claim(take = { seen += "take" }, release = { seen += "give" }, needsService = true)
        waiting.resumed()
        waiting.windowFocus(true)
        waiting.serviceUp(true)
        waiting.serviceUp(false)
        assertFalse(waiting.holding)
        assertEquals(listOf("take"), seen)
        waiting.serviceUp(true)
        assertEquals(listOf("take", "take"), seen)
    }

    @Test fun aKeyIsTakenOnlyWhileHeld() {
        val handled = mutableListOf<String>()
        val delivery = KeyDelivery<String>(claim, { it() }) { handled += it }
        assertFalse(delivery.deliver("a"))
        inFront()
        assertTrue(delivery.deliver("b"))
        assertFalse(delivery.deliver(null))
        assertEquals(listOf("b"), handled)
    }

    @Test fun aKeyPostedBeforeLeavingIsDroppedWhenItRuns() {
        val queue = ArrayDeque<() -> Unit>()
        val handled = mutableListOf<String>()
        val delivery = KeyDelivery<String>(claim, { queue.addLast(it) }) { handled += it }
        inFront()
        assertTrue(delivery.deliver("late"))
        claim.paused()
        queue.removeFirst().invoke()
        assertEquals(emptyList<String>(), handled)
    }
}
