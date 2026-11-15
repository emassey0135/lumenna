package dev.emassey0135.braillesense.edit

/**
 * How the BrailleSense's edit boxes say a cell or a character by itself, as they do when it is
 * typed, taken away or read at the caret (the vendor's `HimsEditSoundFunc` table and
 * `HanEditTextUtil.getChangeToMessage`). The words are the device's English ones.
 */
object CellEcho {
    /** Braille ASCII by dots, dot 1 being bit 0: the cell of each of the 64 six-dot patterns. */
    private const val BRAILLE_ASCII = " A1B'K2L@CIF/MSP\"E3H9O6R^DJG>NTQ,*5<-U8V.%[\$+X!&;:4\\0Z7(_?W]#Y)="

    /**
     * What the edit boxes call each cell by itself, in Braille ASCII's order from the space: its
     * letters inside a word, never a whole word the cell stands for alone ("s t", not "still"), and
     * a cell with no letters by its dots ("dot six").
     */
    private val CELL_NAMES = listOf(
        "space", "the", "dot five", "number sign", "e d", "s h", "and", "apostrophe", "of", "with", "c h", "i n g",
        "dot six", "dash", "four six", "s t", "lower j", "dot two", "lower b", "lower c", "lower d", "e n", "lower f",
        "lower g", "lower h", "i n", "w h", "five six", "g h", "for", "a r", "t h", "dot four", "a", "b", "c", "d", "e",
        "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "o w",
        "o u", "e r", "four five", "underscore",
    )

    /** A lower cell at the end of a word, said as the punctuation it is there. */
    private fun wordEndPunctuation(cell: Char): String? = when (cell) {
        '1' -> "comma"
        '2' -> "semicolon"
        '3' -> "colon"
        '4' -> "period"
        '6' -> "exclamation"
        '7' -> "parenthesis"
        '8' -> "question"
        '0' -> "quotation mark"
        else -> null
    }

    private fun ascii(cell: Byte): Char = BRAILLE_ASCII[cell.toInt() and 0x3f]

    /**
     * One cell of a line by itself: [word] is the cells of the word it is in (dot 1 being bit 0,
     * dots 7 and 8 bits 6 and 7) and [at] where in it the cell is.
     *
     * A cell is said by its name, never back-translated: its letters, or its dots where it has none.
     * After a number sign, until a cell that is not a digit, a letter a to j is its digit. In
     * contracted braille a lower cell ending its word is the punctuation it is there; uncontracted,
     * it is that punctuation wherever it is. A cell with dot 7 or 8 is said by its six dots, or by
     * those two alone.
     */
    fun cell(word: ByteArray, at: Int, contracted: Boolean): String {
        val dots = word.getOrNull(at)?.toInt()?.and(0xff) ?: return ""
        val six = dots and 0x3f
        if (six == 0) {
            return when (dots and 0xc0) {
                0x40 -> "dot seven"
                0x80 -> "dot eight"
                0xc0 -> "seven eight"
                else -> "space"
            }
        }
        val cell = BRAILLE_ASCII[six]
        val sign = (at - 1 downTo 0).firstOrNull { ascii(word[it]) == '#' }
        val inNumber = sign != null && (sign + 1 until at).all { ascii(word[it]) in 'A'..'J' || ascii(word[it]) == '1' || ascii(word[it]) == '4' }
        if (inNumber && cell in 'A'..'J') return if (cell == 'J') "0" else ('1' + (cell - 'A')).toString()
        val endsWord = (at + 1 until word.size).all { word[it].toInt() and 0x3f == 0 }
        if (inNumber || !contracted || endsWord) wordEndPunctuation(cell)?.let { return it }
        return CELL_NAMES[cell - ' ']
    }

    /** Punctuation said by its name, in ASCII's order from the exclamation mark. */
    private val PUNCTUATION_NAMES = mapOf(
        '!' to "exclamation", '"' to "quotation mark", '#' to "number", '$' to "dollar", '%' to "percent",
        '&' to "ampersand", '\'' to "apostrophe", '(' to "left parenthesis", ')' to "right parenthesis",
        '*' to "asterisk", '+' to "plus", ',' to "comma", '-' to "dash", '.' to "period", '/' to "slash",
        ':' to "colon", ';' to "semicolon", '<' to "less than", '=' to "equals", '>' to "greater than",
        '?' to "question", '@' to "at", '[' to "left bracket", '\\' to "back slash", ']' to "right bracket",
        '^' to "caret", '_' to "underscore", '`' to "grave accent", '{' to "left brace", '|' to "vertical bar",
        '}' to "right brace", '~' to "tilde",
    )

    /**
     * One character by itself, as in computer braille, where a cell is the character it stands for:
     * a punctuation mark by its name ("comma"), a space as "space", anything else as itself.
     */
    fun character(character: String): String {
        val one = character.singleOrNull() ?: return character
        if (one == ' ') return "space"
        return PUNCTUATION_NAMES[one] ?: character
    }
}
