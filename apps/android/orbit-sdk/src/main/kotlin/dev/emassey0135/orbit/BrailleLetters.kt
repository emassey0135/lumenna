package dev.emassey0135.orbit

/**
 * The letters a to z as the Speak's six dots make them, a bit per dot (dot 1 bit 0): what a cell
 * pressed alone or with Space or Dot 8 names, whatever table the person types in, since a key's
 * letter is a command's and not text. A cell with dot 7 or 8 in it is no letter.
 */
object BrailleLetters {
    private val cells: IntArray = arrayOf(
        "1", "12", "14", "145", "15", "124", "1245", "125", "24", "245", "13", "123", "134",
        "1345", "135", "1234", "12345", "1235", "234", "2345", "136", "1236", "2456", "1346", "13456", "1356",
    ).map { cell -> cell.fold(0) { all, dot -> all or (1 shl (dot - '1')) } }.toIntArray()

    /** The letter [cell] makes, lowercase; null for a cell that is none. */
    fun letter(cell: Int): Char? {
        val at = cells.indexOf(cell)
        return if (at < 0) null else 'a' + at
    }

    /** The dots of [letter], either case; null for anything but a to z. */
    fun dots(letter: Char): Int? {
        val lower = letter.lowercaseChar()
        return if (lower in 'a'..'z') cells[lower - 'a'] else null
    }
}
