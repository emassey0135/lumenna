package io.github.emassey0135.lumenna

// Choosing one of what the app offers, for the phone and the watch alike: how a choice reads.
// Which actions a row has, what each asks and what a pick offers are the app's.

/** Something offered for choosing, as a chooser shows it. */
data class Option(val key: String, val title: String, val detail: String = "", val depth: Int = 0, val reason: String = "")

/**
 * "level 2" where an option's level in a tree changes from the option shown before it, or
 * nothing: indentation alone says nothing aloud. Said as a tree's rows say it.
 */
fun levelChange(shown: List<Option>, index: Int): String? {
    val previous = if (index > 0) shown[index - 1].depth else 0
    val depth = shown[index].depth
    return if (depth != previous) "level ${depth + 1}" else null
}

/** What tells an option apart beneath its title: its detail and its level where that changes. */
fun detailOf(shown: List<Option>, index: Int): String =
    listOfNotNull(shown[index].detail.ifEmpty { null }, levelChange(shown, index)).joinToString(", ")
