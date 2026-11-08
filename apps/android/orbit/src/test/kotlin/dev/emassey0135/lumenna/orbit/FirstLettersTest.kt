package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

class FirstLettersTest {
    private val labels = listOf("Apples", "Bread", "Butter", "Cheese")

    @Test fun aLetterMovesToTheNextRowStartingWithIt() {
        assertEquals(FirstLetters.Found(1, "b"), FirstLetters.find(labels, 0, "", 'b'))
    }

    @Test fun lettersTypedQuicklyFindTheRowStartingWithThemAll() {
        assertEquals(FirstLetters.Found(2, "bu"), FirstLetters.find(labels, 1, "b", 'u'))
    }

    @Test fun whenNothingStartsWithThemAllTheLastLetterAloneIsLookedFor() {
        assertEquals(FirstLetters.Found(3, "c"), FirstLetters.find(labels, 1, "bx", 'c'))
    }

    @Test fun nothingMatchingIsMinusOne() {
        assertEquals(FirstLetters.Found(-1, ""), FirstLetters.find(labels, 0, "", 'z'))
    }

    @Test fun theOnlyRowStartingWithItIsMinusOneTheLetterKept() {
        assertEquals(FirstLetters.Found(-1, "c"), FirstLetters.find(labels, 3, "", 'c'))
    }
}
