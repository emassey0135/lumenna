package dev.emassey0135.lumenna

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import dev.emassey0135.lumenna.app.Side

// The phone's lists land as the watch's do (`shared`'s `Landing.kt`), scrolling a LazyColumn.

/** [rememberLanding] for a phone list. */
@Composable
fun rememberLanding(core: Core, keys: List<String>, state: LazyListState): RowFocus =
    rememberLanding(core, keys) { state.scrollToItem(it) }

/** [rememberSideLanding] for a phone list. */
@Composable
fun rememberSideLanding(core: Core, side: Side, keys: List<String>, state: LazyListState): RowFocus =
    rememberSideLanding(core, side, keys) { state.scrollToItem(it) }
