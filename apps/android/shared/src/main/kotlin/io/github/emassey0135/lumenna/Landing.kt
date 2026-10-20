package io.github.emassey0135.lumenna

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
import io.github.emassey0135.lumenna.app.Side

/**
 * A list's rows' focus requesters, for landing on the row the app puts in hand: given input
 * focus, for a keyboard, and TalkBack's too — which does not follow input focus under touch,
 * so it is asked for explicitly. Then what the change did is said, after the row rather than
 * cut off by it, as on the iPhone.
 */
class RowFocus internal constructor() {
    private val requesters = HashMap<String, FocusRequester>()

    /** The row's own focus requester. */
    fun requester(key: String): FocusRequester = requesters.getOrPut(key) { FocusRequester() }
}

/**
 * A list on the app's rows: whenever the app moves the row in hand itself ([ViewModel.landing]
 * goes up, past what opening the place moved), focus goes to that row — input focus and
 * TalkBack's — and then what the turn said is said. [keys] are the rows as now listed, and
 * [scrollTo] brings one on screen: the phone's list and the watch's each scroll their own way.
 */
@Composable
fun rememberLanding(core: Core, keys: List<String>, scrollTo: suspend (Int) -> Unit): RowFocus {
    val focus = remember { RowFocus() }
    val view = LocalView.current
    val shown by core.app.view.collectAsState()
    val opened by core.app.opened.collectAsState()
    val landing = shown?.landing
    val first = remember { landing }
    LaunchedEffect(landing) {
        if (landing == null || landing == first || landing <= opened) return@LaunchedEffect
        val place = shown?.focus?.let(keys::indexOf)?.takeIf { it >= 0 }
        if (place != null) {
            scrollTo(place)
            // The row has to be composed, and laid out, before it can take focus.
            withFrameNanos {}
            withFrameNanos {}
            runCatching { focus.requester(keys[place]).requestFocus() }
            view.moveAccessibilityFocus(keys[place])
        }
        core.release()
    }
    return focus
}

/**
 * A list beside the main one — the places, a reminder list, the devices — on the app's row in
 * hand for it ([side]): whenever the app moves that row itself (its `landing` goes up after a
 * change made from the list), focus goes to it, input focus and TalkBack's, and then what the
 * turn said is said. [keys] are the rows as now listed.
 */
@Composable
fun rememberSideLanding(core: Core, side: Side, keys: List<String>, scrollTo: suspend (Int) -> Unit): RowFocus {
    val focus = remember { RowFocus() }
    val view = LocalView.current
    val shown by core.app.view.collectAsState()
    val held = shown?.let {
        when (side) {
            Side.SIDEBAR -> it.sidebarHeld
            Side.REMINDERS -> it.remindersHeld
            Side.DEVICES -> it.devicesHeld
        }
    }
    val landing = held?.landing
    val first = remember { landing }
    LaunchedEffect(landing) {
        if (landing == null || landing == first) return@LaunchedEffect
        val place = held.focus?.let { keys.indexOf(it) }?.takeIf { it >= 0 }
        if (place != null) {
            scrollTo(place)
            withFrameNanos {}
            withFrameNanos {}
            runCatching { focus.requester(keys[place]).requestFocus() }
            view.moveAccessibilityFocus(keys[place])
        }
        core.release()
    }
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
internal fun View.moveAccessibilityFocus(key: String) {
    val owner = (this as? ViewRootForTest)?.semanticsOwner ?: return
    fun find(node: SemanticsNode): SemanticsNode? =
        if (node.config.getOrNull(RowKey) == key) node else node.children.firstNotNullOfOrNull(::find)
    val node = find(owner.rootSemanticsNode) ?: return
    accessibilityNodeProvider?.performAction(node.id, AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS, null)
}
