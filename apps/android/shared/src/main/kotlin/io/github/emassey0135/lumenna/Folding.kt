package io.github.emassey0135.lumenna

import androidx.compose.runtime.Composable
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable

/**
 * A row of a nested list as shown: whether anything is under it, and whether that is folded
 * away. Folding is how someone moving one row at a time skips a block or project with a lot
 * under it, so every list with depth has it, and everything starts expanded.
 */
data class Shown<T>(val item: T, val depth: Int, val parent: Boolean, val collapsed: Boolean) {
    /** What it says of its state: only a row with something under it has one. */
    val state: String? get() = if (!parent) null else if (collapsed) "collapsed" else "expanded"
}

/** The keys of the rows folded away on one screen, kept while the app runs. */
@Composable
fun rememberFolding(): MutableState<Set<String>> = rememberSaveable { mutableStateOf(emptySet()) }

/** [items] as shown with the rows under a collapsed one left out. */
fun <T> folded(items: List<T>, depth: (T) -> Int, key: (T) -> String, collapsed: Set<String>): List<Shown<T>> {
    val shown = mutableListOf<Shown<T>>()
    var hiddenBelow: Int? = null
    items.forEachIndexed { index, item ->
        val level = depth(item)
        val hider = hiddenBelow
        if (hider != null && level > hider) return@forEachIndexed
        hiddenBelow = null
        val parent = index + 1 < items.size && depth(items[index + 1]) > level
        val isCollapsed = parent && key(item) in collapsed
        if (isCollapsed) hiddenBelow = level
        shown += Shown(item, level, parent, isCollapsed)
    }
    return shown
}

/** "level 2" where the level changes from the row shown before, or nothing. */
fun levelChange(shown: List<Shown<*>>, index: Int): String? {
    val previous = if (index > 0) shown[index - 1].depth else 0
    val depth = shown[index].depth
    return if (depth != previous) "level ${depth + 1}" else null
}

/** Expand or Collapse, for a row with something under it; said once done. */
fun foldAction(core: Core, row: Shown<*>, key: String, folding: MutableState<Set<String>>): RowAction? {
    if (!row.parent) return null
    return if (row.collapsed) {
        RowAction("Expand") {
            folding.value = folding.value - key
            core.say("Expanded")
        }
    } else {
        RowAction("Collapse") {
            folding.value = folding.value + key
            core.say("Collapsed")
        }
    }
}

/** Something to do to a row: its name, as TalkBack and the long-press menu both say it. */
data class RowAction(val name: String, val run: () -> Unit)
