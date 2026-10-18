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

/** What identifies a place's row across a reload: a project and a label can share a name. */
fun placeKey(kind: SidebarKind): String = when (kind) {
    is SidebarKind.Group -> "group:${kind.field0}"
    is SidebarKind.Place -> placeKey(kind.field0)
}

fun placeKey(place: Place): String = when (place) {
    is Place.Project -> "project:${place.field0}"
    is Place.Label -> "label:${place.field0}"
    is Place.Filter -> "filter:${place.name}"
    else -> place.toString()
}

private fun key(destination: Destination): String = when (destination) {
    Destination.Settings -> "settings"
    is Destination.At -> placeKey(destination.place)
}

/** One of the app's places as a row: its name, what is in it, and whether it is folded away. */
data class PlaceRow(
    val key: String,
    val entry: SidebarEntry,
    val title: String,
    val detail: String,
    val depth: Int,
    /** Folded away under a collapsed row above it. */
    val hidden: Boolean,
    /** For a row with something under it: whether that is shown, as its Collapse says. */
    val expanded: Boolean?,
)

/** The app's sidebar [entries] as rows, folded as the app has them: a row is open while it offers Collapse. */
fun placeRows(entries: List<SidebarEntry>): List<PlaceRow> {
    var hiddenBelow: UInt? = null
    return entries.map { entry ->
        val title = when (val kind = entry.kind) {
            is SidebarKind.Group -> when (kind.field0) {
                SidebarGroup.PROJECTS -> "Projects"
                SidebarGroup.LABELS -> "Labels"
                SidebarGroup.FILTERS -> "Saved filters"
            }
            is SidebarKind.Place -> placeTitle(kind.field0)
        }
        // The core's line is the title, then what is in it: shown beneath it here. A heading
        // is in sentence case here, where the core's line has it in title case.
        val rest = if (entry.text.startsWith(title, ignoreCase = true)) entry.text.drop(title.length) else entry.text
        val below = hiddenBelow
        if (below != null && entry.depth <= below) hiddenBelow = null
        val hidden = hiddenBelow != null
        val expanded = entry.actions.firstNotNullOfOrNull {
            when (it.kind) {
                ActionKind.COLLAPSE -> true
                ActionKind.EXPAND -> false
                else -> null
            }
        }
        if (expanded == false && !hidden) hiddenBelow = entry.depth
        PlaceRow(placeKey(entry.kind), entry, title, rest.removePrefix(", "), entry.depth.toInt(), hidden, expanded)
    }
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
    version: Any?,
    open: (PlaceRow) -> (() -> Unit)?,
    openLabel: (PlaceRow) -> String,
    selected: String? = null,
    extra: List<Pair<String, () -> Unit>> = emptyList(),
) {
    val shown = rows.filter { !it.hidden }
    val state = rememberLazyListState()
    val keys = shown.map { it.key } + extra.map { it.first }
    val focus = rememberHeldFocus(core, keys, state, version)
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
                actions = focus.actions(row.key, index, core.app.rowActions(row.key, row.entry.actions)),
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
            core, placeRows(entries), version = entries,
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
