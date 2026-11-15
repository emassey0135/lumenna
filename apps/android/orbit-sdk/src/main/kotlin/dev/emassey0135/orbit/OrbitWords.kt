package dev.emassey0135.orbit

/**
 * What the device itself says, in its own words: its edit fields' echo, its Editor's selection and
 * clipboard, T9's modes, and what the SDK reports when it cannot reach what Orbit keeps: the stock
 * strings of Orbit's English build, and the Satellite launcher's for the echo of dots and deletions,
 * which Orbit's fields leave to the screen reader. They are the device's, not an app's, so an app
 * says them as they are; one with words of its own passes its own [OrbitWords]. Each `%s` is filled in.
 */
data class OrbitWords(
    /** A cell that makes no character yet, `%s` its dots: "dot 1 2 6". */
    val dots: String = "dot %s",
    /** What was deleted, `%s` it: "h deleted", "dot 6 deleted". */
    val deleted: String = "%s deleted",
    /** A space, typed or deleted. */
    val space: String = "space",
    /** A new line, typed or deleted. */
    val newLine: String = "new line",
    /** Deleting with nothing before the caret. */
    val blank: String = "blank",
    /** Selection mode on, the caret the selection's start, as the Editor says it. */
    val selectionStarted: String = "Selection started. Please move to desired position and press the same key again.",
    /** Selection mode off, the selection running to the character at the caret. */
    val selectionFinished: String = "Selection finished",
    val allSelected: String = "All text selected",
    /** Copy or cut with nothing selected. */
    val nothingSelected: String = "Nothing selected",
    /** Reading the selection with nothing selected. */
    val noTextSelected: String = "No text selected",
    val copied: String = "Text copied",
    val cut: String = "Text cut",
    val pasted: String = "Text pasted",
    /** T9's modes, as Orbit's own fields name them when star changes one. */
    val lowercase: String = "lowercase",
    val uppercase: String = "uppercase",
    val digits: String = "digits",
    val symbols: String = "symbols",
    /** Root was refused or failed, `%s` what could not be read and why. */
    val rootFailed: String = "Could not read Orbit's settings: %s",
    /** The cells the device's edit fields name rather than translate (see [CellNames]). */
    val cellNames: CellNames = CellNames(),
) {
    /** What T9's [mode] is called. */
    fun mode(mode: T9Input.Mode): String = when (mode) {
        T9Input.Mode.LOWERCASE -> lowercase
        T9Input.Mode.UPPERCASE -> uppercase
        T9Input.Mode.DIGITS -> digits
        T9Input.Mode.SYMBOLS -> symbols
    }
}

/**
 * The cells the Orbit Speak's own edit fields name rather than translate, a bit per dot (dot 1 bit
 * 0), with the names they say: the Orbit's table (its EditBox's echo replacements), kept to the
 * Orbit. In contracted braille a cell here is said by its name before anything it would make;
 * otherwise only when it makes nothing yet, as the capital sign does.
 */
data class CellNames(
    val names: Map<Int, String> = mapOf(
        0b00_0010 to "dot 2",
        0b00_0110 to "low b",
        0b01_0010 to "low c",
        0b11_0010 to "low d",
        0b01_0110 to "low f",
        0b11_0110 to "low g",
        0b10_0110 to "low h",
        0b11_0100 to "low j",
        0b11_1100 to "number sign",
        CAPITAL_SIGN to "capital sign",
        // The device's own table calls dots 3 6 "greek sign", which is dots 4 6 in UEB; dots 3 6
        // alone is the hyphen, so it is said as one here, a deliberate departure from the device.
        0b10_0100 to "hyphen",
    ),
    /** A capital sign straight after one. */
    val capitalWordSign: String = "capital word sign",
) {
    /** What the device's fields call cell [at] of [word] by itself, if it is one they name. */
    fun name(word: List<Int>, at: Int): String? {
        val cell = word.getOrNull(at) ?: return null
        if (cell == CAPITAL_SIGN && at > 0 && word[at - 1] == CAPITAL_SIGN) return capitalWordSign
        return names[cell]
    }

    private companion object {
        const val CAPITAL_SIGN = 0b10_0000
    }
}
