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
 * and Collapse among their actions; a project's, label's or filter's actions are the ones
 * Browse offers (`PlaceActions.kt`). The place shown is said as selected.
 */
@Composable
fun Sidebar(core: Core, changes: Long, current: Destination, choose: (Destination) -> Unit, modifier: Modifier = Modifier) {
    val prompt = rememberPrompter()
    val entries = remember(changes) { core.attempt { core.lumenna.places().entries }.orEmpty() }
    val projects = entries.mapNotNull { ((it.kind as? SidebarKind.Place)?.v1 as? Place.Project)?.v1 }
    val labels = entries.mapNotNull { ((it.kind as? SidebarKind.Place)?.v1 as? Place.Label)?.v1 }
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

    fun actions(item: Item): List<RowAction> {
        val entry = byKey[item.key] ?: return emptyList()
        return when (val kind = entry.kind) {
            is SidebarKind.Group -> when (kind.v1) {
                SidebarGroup.PROJECTS -> listOf(RowAction("New Project") { addProject(core, prompt) })
                SidebarGroup.LABELS -> listOf(RowAction("New Label") { addLabel(core, prompt) })
                SidebarGroup.FILTERS -> listOf(RowAction("New Saved Filter") { addFilter(core, prompt) })
            }
            is SidebarKind.Place -> when (val place = kind.v1) {
                is Place.Project -> projectActions(core, prompt, place.v1, entry.archived) { projects }
                is Place.Label -> labelActions(core, prompt, place.v1) { labels }
                is Place.Filter -> filterActions(core, prompt, place.name, place.query)
                else -> emptyList()
            }
        }
    }

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
                )
            }
        }
    }
    prompt.Host()
}
