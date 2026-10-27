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
 * The core's "level 2" where each of [depths], rows shown in order, changes from the one before,
 * else null: for a list a shell narrows or cuts out itself, so no shell counts levels.
 */
@OptIn(ExperimentalUnsignedTypes::class)
fun levelsOf(depths: List<UInt>): List<String?> =
    dev.emassey0135.lumenna.app.levels(depths.toUIntArray()).map { it.ifEmpty { null } }

/**
 * These options, each with its level where it changes from the one shown before it: for a list
 * the core gives no lines for, or one narrowed by typing, where the core's level, against the
 * whole list, no longer follows the option before. [levels] is the core's ([levelsOf]).
 */
fun List<Option>.leveled(levels: (List<UInt>) -> List<String?> = ::levelsOf): List<Option> =
    zip(levels(map { it.depth.toUInt() })) { option, level -> option.copy(level = level) }

/** What tells an option apart beneath its title: its detail, and its level where that changes. */
fun detailOf(option: Option): String = listOfNotNull(option.detail.ifEmpty { null }, option.level).joinToString(", ")
