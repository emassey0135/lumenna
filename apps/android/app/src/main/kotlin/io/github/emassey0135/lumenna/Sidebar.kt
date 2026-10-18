package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import io.github.emassey0135.lumenna.app.ActionKind
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.SidebarEntry
import io.github.emassey0135.lumenna.app.SidebarGroup
import io.github.emassey0135.lumenna.app.SidebarKind
import io.github.emassey0135.lumenna.app.Side

/** Somewhere a wide window's sidebar goes: one of the core's places, or Settings. */
sealed interface Destination {
    data class At(val place: Place) : Destination
    data object Settings : Destination
}

/** The screen a destination opens on. */
fun Destination.screen(): Screen = when (this) {
    Destination.Settings -> Screen.Settings
    is Destination.At -> when (val place = place) {
        Place.Today -> Screen.Day()
        Place.Blocks -> Screen.Blocks
        else -> Screen.Tasks(place)
    }
}

private fun key(destination: Destination): String = when (destination) {
    Destination.Settings -> "settings"
    is Destination.At -> placeKey(destination.place)
}

/**
 * Rows of the app's places, one TalkBack stop each, "expanded" or "collapsed" where something
 * is under them; their actions the app's, Expand and Collapse among them. Focus stays on a row
 * after its action. [extra] rows follow: the sidebar's Settings.
 */
@Composable
fun PlaceRows(
    core: Core,
    rows: List<PlaceRow>,
    open: (PlaceRow) -> (() -> Unit)?,
    openLabel: (PlaceRow) -> String,
    selected: String? = null,
    extra: List<Pair<String, () -> Unit>> = emptyList(),
) {
    val shown = rows.filter { !it.hidden }
    val state = rememberLazyListState()
    val keys = shown.map { it.key } + extra.map { it.first }
    val focus = rememberSideLanding(core, Side.SIDEBAR, keys, state)
    LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(keys.size, 1) }, state = state) {
        itemsIndexed(shown, key = { _, row -> row.key }) { index, row ->
            val previous = if (index > 0) shown[index - 1].depth else 0
            val level = if (row.depth != previous) "level ${row.depth + 1}" else null
            val group = row.entry.kind is SidebarKind.Group
            ListRow(
                title = row.title,
                detail = row.detail,
                speech = listOfNotNull(row.detail.ifEmpty { null }, row.expanded?.let { if (it) "expanded" else "collapsed" }, level)
                    .joinToString(", "),
                index = index,
                depth = row.depth,
                actions = core.app.rowActions(row.key, row.entry.actions, side = Side.SIDEBAR),
                open = open(row),
                openLabel = openLabel(row),
                focus = focus.requester(row.key),
                key = row.key,
                heading = group,
                selected = row.key == selected,
                say = core::say,
            )
        }
        itemsIndexed(extra, key = { _, (key, _) -> key }) { index, (key, choose) ->
            ListRow(
                title = key.replaceFirstChar { it.uppercase() },
                detail = "",
                speech = "",
                index = shown.size + index,
                actions = emptyList(),
                open = choose,
                openLabel = "Show",
                focus = focus.requester(key),
                key = key,
                selected = key == selected,
                say = core::say,
            )
        }
    }
}

/**
 * The places, in a wide window, as the desktop apps and the iPad list them: Today, Tasks, the
 * project tree, labels, saved filters, blocks, the trash (the app's sidebar), then Settings.
 *
 * Headings and projects with subprojects fold, saying "expanded" or "collapsed", with Expand
 * and Collapse among their actions; a heading's and a place's actions are the app's, as Browse
 * offers them. The place shown is said as selected.
 */
@Composable
fun Sidebar(core: Core, current: Destination, choose: (Destination) -> Unit, modifier: Modifier = Modifier) {
    val view by core.app.view.collectAsState()
    val entries = view?.sidebar.orEmpty()
    Column(modifier.semantics { paneTitle = "Places" }) {
        Text(
            "Places",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(16.dp).semantics { heading() },
        )
        PlaceRows(
            core, placeRows(entries),
            // A heading is not somewhere to go: choosing it folds or unfolds it.
            open = { row ->
                when (val kind = row.entry.kind) {
                    is SidebarKind.Group -> row.entry.actions.firstOrNull { it.kind == ActionKind.EXPAND || it.kind == ActionKind.COLLAPSE }
                        ?.let { fold -> { core.app.send(ShellEvent.Perform(fold)) } }
                    is SidebarKind.Place -> ({ choose(Destination.At(kind.field0)) })
                }
            },
            openLabel = { row ->
                if (row.entry.kind is SidebarKind.Group) (if (row.expanded == true) "Collapse" else "Expand") else "Show"
            },
            selected = key(current),
            extra = listOf("settings" to { choose(Destination.Settings) }),
        )
    }
}
