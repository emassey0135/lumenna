package dev.emassey0135.lumenna

import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class GlancesTest {
    private val entries = listOf(glance("2026-10-26T09:00:00Z"), glance("2026-10-26T10:30:00Z"), glance("2026-10-26T12:00:00Z"))

    @Test
    fun theEntryTrueNowIsTheLastToHaveStarted() {
        assertEquals("2026-10-26T10:30:00Z", Glances.now(entries, Instant.parse("2026-10-26T11:59:59Z"))?.moment?.at)
        assertEquals("2026-10-26T12:00:00Z", Glances.now(entries, Instant.parse("2026-10-26T12:00:00Z"))?.moment?.at)
    }

    @Test
    fun beforeTheFirstEntryTheFirstIsShown() {
        assertEquals("2026-10-26T09:00:00Z", Glances.now(entries, Instant.parse("2026-10-26T08:00:00Z"))?.moment?.at)
        assertNull(Glances.now(emptyList()))
    }
}
