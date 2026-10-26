package dev.emassey0135.lumenna

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class SearchFingerprintTest {
    @Test
    fun aDocumentsFingerprintIsItsWholeFieldsNotA32BitHash() {
        // Two strings Java's hashCode cannot tell apart.
        assertEquals("Aa".hashCode(), "BB".hashCode())
        assertNotEquals(SearchIndex.fingerprint(listOf("Aa")), SearchIndex.fingerprint(listOf("BB")))
        assertEquals(64, SearchIndex.fingerprint(listOf("task", "Buy milk")).length)
    }

    @Test
    fun whereOneFieldEndsAndTheNextBeginsCounts() {
        assertNotEquals(SearchIndex.fingerprint(listOf("ab", "c")), SearchIndex.fingerprint(listOf("a", "bc")))
    }
}
