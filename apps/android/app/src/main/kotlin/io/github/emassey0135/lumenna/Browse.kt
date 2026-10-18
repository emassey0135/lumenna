package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.SidebarKind
import io.github.emassey0135.lumenna.app.SidebarGroup
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.ActionKind
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import io.github.emassey0135.lumenna.app.BlockEvent
import io.github.emassey0135.lumenna.app.ShellEvent
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import io.github.emassey0135.lumenna.records.Action
import io.github.emassey0135.lumenna.records.labelReference
import io.github.emassey0135.lumenna.records.projectReference

/** One line of a list of things: what identifies it, how it reads, and how deep it sits. */
data class Item(
    val key: String,
    val title: String,
    val detail: String = "",
    val depth: Int = 0,
    val speech: String = detail,
    /** What can be done to it: the core's. */
    val actions: List<Action> = emptyList(),
    /** Whether it is this device, for a device's row. */
    val thisDevice: Boolean = false,
)

/** What a list loads: its items, what is said of them above, and what it says with none. */
data class Listing(val items: List<Item>, val said: String = "", val empty: String = "")

/**
 * A list of things with what can be done to each: a heading's worth of count above, one
 * TalkBack stop per item, its actions custom actions and a long press.
 */
@Composable
fun ItemListScreen(
    title: String,
    core: Core,
    navigator: Navigator,
    load: () -> Listing,
    changes: Long,
    addLabel: String? = null,
    add: () -> Unit = {},
    open: ((Item) -> Unit)?,
    openLabel: String = "Open",
    actions: (Item) -> List<RowAction> = { emptyList() },
    header: (@Composable () -> Unit)? = null,
) {
    val loaded = remember(changes) { core.attempt(load) ?: Listing(emptyList()) }
    val all = loaded.items
    // An empty list says what is empty, in the core's words, in place of its count.
    val count = if (all.isEmpty() && loaded.empty.isNotEmpty()) loaded.empty else loaded.said
    val folding = rememberFolding()
    val folds = folded(all, { it.depth }, { it.key }, folding.value)
    val items = folds.map { it.item }
    val state = rememberLazyListState()
    val focus = rememberRowFocus(core, items.map { it.key }, state)
    ScreenFrame(title, core, navigator, actions = {
        if (addLabel != null) {
            IconButton(onClick = add) { Icon(Icons.Filled.Add, contentDescription = addLabel) }
        }
    }) {
        header?.invoke()
        if (count.isNotEmpty()) {
            Text(
                count.replaceFirstChar { it.uppercase() },
                style = MaterialTheme.typography.bodyMedium,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(items.size, 1) }, state = state) {
            itemsIndexed(items, key = { _, item -> item.key }) { index, item ->
                ListRow(
                    title = item.title,
                    detail = item.detail,
                    // The level is said against the row shown before, which folding can change.
                    speech = listOfNotNull(item.speech.ifEmpty { null }, folds[index].state, levelChange(folds, index))
                        .joinToString(", "),
                    index = index,
                    depth = item.depth,
                    actions = focus.actions(item.key, index, actions(item)) +
                        listOfNotNull(foldAction(core, folds[index], item.key, folding)),
                    open = open?.let { { it(item) } },
                    openLabel = openLabel,
                    focus = focus.requester(item.key),
                    key = item.key,
                    say = core::say,
                    thisDevice = item.thisDevice,
                )
                HorizontalDivider()
            }
        }
    }
}

/** Every kind of thing there is to browse, with how many of each: the app's places. */
@Composable
fun BrowseScreen(core: Core, navigator: Navigator) {
    val view by core.app.view.collectAsState()
    val rows = placeRows(view?.sidebar.orEmpty())
    fun count(n: Int, noun: String) = if (n == 1) "1 $noun" else "$n ${noun}s"
    fun under(group: SidebarGroup) = rows.groupUnder(group).count { it.depth == 1 }
    val trash = rows.firstOrNull { (it.entry.kind as? SidebarKind.Place)?.field0 == Place.Trash }
    val items = listOf(
        Item("projects", "Projects", count(rows.groupUnder(SidebarGroup.PROJECTS).size, "project")),
        Item("labels", "Labels", count(under(SidebarGroup.LABELS), "label")),
        Item("filters", "Saved filters", count(under(SidebarGroup.FILTERS), "filter")),
        Item("blocks", "Blocks", ""),
        Item("trash", "Trash", trash?.detail.orEmpty()),
    )
    ItemListScreen(
        "Browse", core, navigator, changes = 0L,
        load = { Listing(items) },
        open = { item ->
            navigator.push(
                when (item.key) {
                    "projects" -> Screen.Projects
                    "labels" -> Screen.Labels
                    "filters" -> Screen.Filters
                    "blocks" -> Screen.Blocks
                    else -> Screen.Tasks(Place.Trash)
                },
            )
        },
    )
}

/** The rows under a group's heading in the app's places, down to the next heading or place. */
private fun List<PlaceRow>.groupUnder(group: SidebarGroup): List<PlaceRow> {
    val heading = indexOfFirst { (it.entry.kind as? SidebarKind.Group)?.field0 == group }
    if (heading < 0) return emptyList()
    return drop(heading + 1).takeWhile { it.depth > 0 }
}

/**
 * The places under one of the app's headings — the project tree, the labels, the saved
 * filters — with the heading's own action (New Project, …) as the add button, and each one's
 * actions the app's. Folding within it is the app's; the heading's own fold is the sidebar's.
 */
@Composable
private fun GroupScreen(core: Core, navigator: Navigator, group: SidebarGroup, title: String, addLabel: String, openLabel: String) {
    val app = core.app
    val view by app.view.collectAsState()
    val entries = view?.sidebar.orEmpty()
    val all = placeRows(entries)
    val heading = all.firstOrNull { (it.entry.kind as? SidebarKind.Group)?.field0 == group }
    // Folded within the group as the app has it; the heading's own fold left out.
    val rows = placeRows(all.groupUnder(group).map { it.entry.copy(depth = it.entry.depth - 1u) })
    ScreenFrame(title, core, navigator, actions = {
        heading?.entry?.actions?.firstOrNull { it.kind == ActionKind.NEW }?.let { new ->
            IconButton(onClick = { app.send(ShellEvent.Perform(new)) }) { Icon(Icons.Filled.Add, contentDescription = addLabel) }
        }
    }) {
        PlaceRows(
            core, rows, version = entries,
            open = { row -> (row.entry.kind as? SidebarKind.Place)?.field0?.let { place -> { navigator.push(Screen.Tasks(place)) } } },
            openLabel = { openLabel },
        )
    }
}

/** The project tree. Depth is said where it changes. */
@Composable
fun ProjectsScreen(core: Core, navigator: Navigator) =
    GroupScreen(core, navigator, SidebarGroup.PROJECTS, "Projects", "Add project", "Show its tasks")

/** Labels: a first-class axis, with its own list. */
@Composable
fun LabelsScreen(core: Core, navigator: Navigator) =
    GroupScreen(core, navigator, SidebarGroup.LABELS, "Labels", "Add label", "Show the tasks wearing it")

/** Saved filters: created, renamed, requeried, reordered and deleted, not only run. */
@Composable
fun FiltersScreen(core: Core, navigator: Navigator) =
    GroupScreen(core, navigator, SidebarGroup.FILTERS, "Saved filters", "Add filter", "Show its tasks")

/**
 * Every block series, as the app lists them (its Blocks place): said as a task row is, when in
 * this device's clock ("every weekday at 9:00 AM"), then how long. Opening one changes every
 * occurrence, in the app's block form.
 */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Open(io.github.emassey0135.lumenna.app.Place.Blocks)) }
    val list = view?.takeIf { it.place == io.github.emassey0135.lumenna.app.Place.Blocks }?.list
    FollowBlockForm(core, navigator)
    ScreenFrame("Blocks", core, navigator, actions = {
        IconButton(onClick = { app.send(ShellEvent.Block(BlockEvent.New(null, null))) }) {
            Icon(Icons.Filled.Add, contentDescription = "Add block")
        }
    }) {
        val count = list?.let { if (it.items.isEmpty() && it.empty.isNotEmpty()) it.empty else it.announcement }.orEmpty()
        if (count.isNotEmpty()) {
            Text(
                count.replaceFirstChar { it.uppercase() },
                style = MaterialTheme.typography.bodyMedium,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        AppTaskRows(core, list?.items.orEmpty().filter { !it.hidden }, open = { row ->
            row.actions.firstOrNull { it.kind == io.github.emassey0135.lumenna.app.ActionKind.EDIT }?.let { edit ->
                app.send(ShellEvent.Focus(row.id))
                app.send(ShellEvent.Perform(edit))
            }
        }, openLabel = "Edit every occurrence")
    }
}
