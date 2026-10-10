package io.github.emassey0135.lumenna

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import android.view.View
import android.view.accessibility.AccessibilityNodeInfo
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.ViewRootForTest
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.SemanticsPropertyReceiver
import androidx.compose.ui.semantics.getOrNull

/**
 * Where focus goes after a row's action changes the list: the same row if it is still
 * listed, else whichever now holds its place — not wherever a redraw happens to leave it.
 *
 * The row is given input focus, for a keyboard, and TalkBack's focus too — which does not
 * follow input focus under touch, so it is asked for explicitly. Then what the change did is
 * said, after the row rather than cut off by it, as on the iPhone.
 */
class RowFocus internal constructor(private val core: Core) {
    internal var target: Pair<String, Int>? = null
    private val requesters = HashMap<String, FocusRequester>()

    /** The row's own focus requester. */
    fun requester(key: String): FocusRequester = requesters.getOrPut(key) { FocusRequester() }

    /** The row's actions, each noting the row before it runs, so focus can come back to it. */
    fun actions(key: String, index: Int, actions: List<RowAction>): List<RowAction> = actions.map { action ->
        action.copy {
            target = key to index
            core.hold()
            action.run()
        }
    }
}

/**
 * A list's [RowFocus]. [keys] are the rows as now listed, in order; when the store changes
 * after a row's action, focus goes back to that row, or to the one now in its place.
 */
@Composable
fun rememberRowFocus(core: Core, keys: List<String>, state: LazyListState): RowFocus {
    val focus = remember { RowFocus(core) }
    val view = LocalView.current
    val changes by core.changes.collectAsState()
    LaunchedEffect(changes) {
        val (key, index) = focus.target ?: return@LaunchedEffect
        focus.target = null
        if (keys.isNotEmpty()) {
            val place = keys.indexOf(key).takeIf { it >= 0 } ?: index.coerceIn(0, keys.lastIndex)
            state.scrollToItem(place)
            // The row has to be composed, and laid out, before it can take focus.
            withFrameNanos {}
            withFrameNanos {}
            runCatching { focus.requester(keys[place]).requestFocus() }
            view.moveAccessibilityFocus(keys[place])
        }
        core.release()
    }
    // Leaving the list says whatever was held for it.
    DisposableEffect(Unit) { onDispose { core.release() } }
    return focus
}

/** Which row a node is, so its node can be found in the accessibility tree. */
val RowKey = SemanticsPropertyKey<String>("RowKey")
var SemanticsPropertyReceiver.rowKey by RowKey

/** A row's title alone, for finding it; what TalkBack says is the whole description. */
val RowTitle = SemanticsPropertyKey<String>("RowTitle")
var SemanticsPropertyReceiver.rowTitle by RowTitle

/**
 * Puts TalkBack's focus on the row whose key is [key], as TalkBack itself does: by performing
 * the accessibility-focus action on its node. Nothing happens when no screen reader is on.
 */
private fun View.moveAccessibilityFocus(key: String) {
    val owner = (this as? ViewRootForTest)?.semanticsOwner ?: return
    fun find(node: SemanticsNode): SemanticsNode? =
        if (node.config.getOrNull(RowKey) == key) node else node.children.firstNotNullOfOrNull(::find)
    val node = find(owner.rootSemanticsNode) ?: return
    accessibilityNodeProvider?.performAction(node.id, AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS, null)
}
