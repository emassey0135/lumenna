package com.extra.han.lumenna

import com.extra.han.lumenna.device.Claim
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ClaimTest {
    private val seen = mutableListOf<String>()
    private val claim = Claim(take = { seen += "take" }, giveBack = { seen += "give" })

    private fun inFront() {
        claim.resumed(true)
        claim.focused(true)
    }

    @Test fun nothingIsTakenUntilResumedAndFocused() {
        claim.resumed(true)
        assertFalse(claim.holding)
        assertEquals(emptyList<String>(), seen)
        claim.focused(true)
        assertTrue(claim.holding)
        assertEquals(listOf("take"), seen)
    }

    @Test fun pausingGivesBackAtOnce() {
        inFront()
        claim.resumed(false)
        assertFalse(claim.holding)
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun somethingDrawnOverTheAppGivesBackWithoutAPause() {
        inFront()
        claim.focused(false)
        assertFalse(claim.holding)
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun comingBackFromSomethingDrawnOverItTakesAgain() {
        inFront()
        claim.focused(false)
        claim.focused(true)
        assertTrue(claim.holding)
        assertEquals(listOf("take", "give", "take"), seen)
    }

    @Test fun losingBothGivesBackOnce() {
        inFront()
        claim.focused(false)
        claim.resumed(false)
        assertEquals(listOf("take", "give"), seen)
    }

    @Test fun focusWithoutResumeTakesNothing() {
        claim.focused(true)
        assertFalse(claim.holding)
        assertEquals(emptyList<String>(), seen)
    }
}
