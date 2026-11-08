package dev.emassey0135.lumenna.orbit

import org.junit.Assert.assertEquals
import org.junit.Test

/** Ported from the Satellite launcher's (Apache-2.0, the same author's). */
class T9InputTest {
    private val field = Field()
    private val later = Manual()
    private val t9 = T9Input(field, later)

    private fun type(vararg digits: Int) = digits.forEach(t9::tap)

    @Test fun tappingAKeyAgainCyclesItsLetters() {
        type(2, 2, 2, 2)
        assertEquals("abca", field.offered.toString())
        t9.commitNow()
        assertEquals("a", field.text.toString())
    }

    @Test fun anotherKeyCommitsTheCandidate() {
        type(4, 4, 3)
        t9.commitNow()
        assertEquals("hd", field.text.toString())
    }

    @Test fun zeroIsSpace() {
        type(4, 0, 4, 4, 4)
        t9.commitNow()
        assertEquals("g i", field.text.toString())
    }

    @Test fun starGoesThroughTheModes() {
        t9.nextMode()
        type(2)
        t9.nextMode()
        type(5, 5)
        t9.nextMode()
        type(1)
        t9.commitNow()
        assertEquals("A55.", field.text.toString())
        assertEquals(listOf("uppercase", "digits", "symbols"), field.modes)
    }

    @Test fun theCandidateGoesWhereItsKeyWasFirstTapped() {
        field.text.append("xy")
        field.caret = 1
        type(4, 4)
        t9.commitNow()
        assertEquals("xhy", field.text.toString())
        assertEquals(2, field.caret)
    }

    @Test fun theTimeoutCommitsWithFeedback() {
        type(2)
        later.fire()
        assertEquals("a", field.text.toString())
        assertEquals(listOf(true), field.feedback)
    }

    @Test fun cancelDropsTheCandidate() {
        type(2)
        t9.cancel()
        t9.commitNow()
        assertEquals("", field.text.toString())
    }

    private class Manual : T9Input.Later {
        var run: (() -> Unit)? = null
        override fun after(milliseconds: Long, run: () -> Unit) { this.run = run }
        override fun cancel() { run = null }
        fun fire() = run?.invoke()
    }

    private class Field : T9Input.Host {
        val text = StringBuilder()
        val offered = StringBuilder()
        val modes = mutableListOf<String>()
        val feedback = mutableListOf<Boolean>()
        var caret = 0
        override fun caret(): Int = caret
        override fun timeoutMs(): Long = 1000
        override fun candidate(character: Char) { offered.append(character) }
        override fun modeChanged(name: String) { modes.add(name) }
        override fun commit(character: Char, position: Int, feedback: Boolean) {
            text.insert(position, character)
            caret = position + 1
            this.feedback.add(feedback)
        }
    }
}
