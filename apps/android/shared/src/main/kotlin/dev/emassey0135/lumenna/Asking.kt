package dev.emassey0135.lumenna

// Choosing one of what the app offers, for the phone and the watch alike: how a choice reads.
// Which actions a row has, what each asks and what a pick offers are the app's.

/**
 * Something offered for choosing, as a chooser shows it; a colour's [swatch] drawn beside it.
 * [level] is the core's "level 2" where the option's level changes from the one before it in
 * the whole list (`Line::level`).
 */
data class Option(
    val key: String,
    val title: String,
    val detail: String = "",
    val depth: Int = 0,
    val reason: String = "",
    val swatch: dev.emassey0135.lumenna.app.Swatch? = null,
    val level: String? = null,
)

/**
 * "level 2" where an option's level changes from the option shown before it, or nothing:
 * indentation alone says nothing aloud. For a list narrowed by typing, where the core's level,
 * against the whole list, no longer follows the option before; the words are the core's.
 */
fun levelChange(shown: List<Option>, index: Int, word: (UInt) -> String = { dev.emassey0135.lumenna.app.level(it) }): String? {
    val previous = if (index > 0) shown[index - 1].depth else 0
    val depth = shown[index].depth
    return if (depth != previous) word(depth.toUInt()) else null
}

/** These options, each with its level where it changes from the one before, for a list the core gives no lines for. */
fun List<Option>.leveled(word: (UInt) -> String = { dev.emassey0135.lumenna.app.level(it) }): List<Option> =
    mapIndexed { index, option -> option.copy(level = levelChange(this, index, word)) }

/**
 * What tells an option apart beneath its title: its detail and its level where that changes,
 * the core's in a [whole] list, else worked out against the options shown.
 */
fun detailOf(shown: List<Option>, index: Int, whole: Boolean = true, word: (UInt) -> String = { dev.emassey0135.lumenna.app.level(it) }): String =
    listOfNotNull(shown[index].detail.ifEmpty { null }, if (whole) shown[index].level else levelChange(shown, index, word)).joinToString(", ")
