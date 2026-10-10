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
import io.github.emassey0135.lumenna.core.Place
import io.github.emassey0135.lumenna.core.SidebarGroup
import io.github.emassey0135.lumenna.core.SidebarKind
import io.github.emassey0135.lumenna.core.placeQuery
import io.github.emassey0135.lumenna.core.placeQuickAddPrefix
import io.github.emassey0135.lumenna.core.placeTitle

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
        Place.Tasks -> Screen.Tasks()
        Place.Blocks -> Screen.Blocks
        Place.Trash -> Screen.Tasks(title = "Trash", query = placeQuery(place), trash = true)
        else -> Screen.Tasks(placeTitle(place), placeQuery(place), placeQuickAddPrefix(place))
    }
}

/** What identifies a row across a reload: a project and a label can share a name. */
private fun key(kind: SidebarKind): String = when (kind) {
    is SidebarKind.Group -> "group:${kind.v1}"
    is SidebarKind.Place -> key(kind.v1)
}

private fun key(place: Place): String = when (place) {
    is Place.Project -> "project:${place.v1}"
    is Place.Label -> "label:${place.v1}"
    is Place.Filter -> "filter:${place.name}"
    else -> place.toString()
}

private fun key(destination: Destination): String = when (destination) {
    Destination.Settings -> "settings"
    is Destination.At -> key(destination.place)
}

/**
 * The places, in a wide window, as the desktop apps and the iPad list them: Today, Tasks, the
 * project tree, labels, saved filters, blocks, the trash (`Lumenna.places`), then Settings.
 *
 * Headings and projects with subprojects fold, saying "expanded" or "collapsed", with Expand
 * and Collapse among their actions; a heading's and a place's actions are the core's, as
 * Browse offers them. The place shown is said as selected.
 */
@Composable
fun Sidebar(core: Core, changes: Long, current: Destination, choose: (Destination) -> Unit, modifier: Modifier = Modifier) {
    val prompt = rememberPrompter()
    val entries = remember(changes) { core.attempt { core.lumenna.places().entries }.orEmpty() }
    val byKey = entries.associateBy { key(it.kind) }
    val all = entries.map { entry ->
        val title = when (val kind = entry.kind) {
            is SidebarKind.Group -> when (kind.v1) {
                SidebarGroup.PROJECTS -> "Projects"
                SidebarGroup.LABELS -> "Labels"
                SidebarGroup.FILTERS -> "Saved Filters"
            }
            is SidebarKind.Place -> placeTitle(kind.v1)
        }
        // The core's line is the title, then what is in it: shown beneath it here.
        Item(key(entry.kind), title, entry.text.removePrefix(title).removePrefix(", "), entry.depth.toInt())
    } + Item("settings", "Settings")
    val folding = rememberFolding()
    val folds = folded(all, { it.depth }, { it.key }, folding.value)
    val items = folds.map { it.item }
    val state = rememberLazyListState()
    val focus = rememberRowFocus(core, items.map { it.key }, state)
    val selected = key(current)

    // The core's, for headings (New Project, New Label, New Saved Filter) and places alike.
    fun actions(item: Item): List<RowAction> = core.offered(byKey[item.key]?.actions.orEmpty(), prompt)

    Column(modifier.semantics { paneTitle = "Places" }) {
        Text(
            "Places",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(16.dp).semantics { heading() },
        )
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(items.size, 1) }, state = state) {
            itemsIndexed(items, key = { _, item -> item.key }) { index, item ->
                val entry = byKey[item.key]
                val group = entry?.kind is SidebarKind.Group
                val fold = foldAction(core, folds[index], item.key, folding)
                ListRow(
                    title = item.title,
                    detail = item.detail,
                    speech = listOfNotNull(item.detail.ifEmpty { null }, folds[index].state, levelChange(folds, index))
                        .joinToString(", "),
                    index = index,
                    depth = item.depth,
                    actions = focus.actions(item.key, index, actions(item)) + listOfNotNull(fold),
                    // A heading is not somewhere to go: choosing it folds or unfolds it.
                    open = when {
                        group -> fold?.run
                        item.key == "settings" -> ({ choose(Destination.Settings) })
                        else -> (entry?.kind as? SidebarKind.Place)?.let { place -> { choose(Destination.At(place.v1)) } }
                    },
                    openLabel = if (group) fold?.name ?: "Show" else "Show",
                    focus = focus.requester(item.key),
                    key = item.key,
                    heading = group,
                    selected = item.key == selected,
                    say = core::say,
                )
            }
        }
    }
    prompt.Host()
}
