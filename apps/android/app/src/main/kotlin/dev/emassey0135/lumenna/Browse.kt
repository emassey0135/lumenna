package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.SidebarGroup
import dev.emassey0135.lumenna.app.groupCount
import dev.emassey0135.lumenna.app.groupEmpty
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ActionKind
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import dev.emassey0135.lumenna.app.BlockEvent
import dev.emassey0135.lumenna.app.ShellEvent
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
import dev.emassey0135.lumenna.app.Action

/** One entry of a menu: what identifies it, its name, and what it holds. */
data class Item(val key: String, val title: String, val detail: String = "")

/**
 * A fixed menu — Browse's kinds of thing, Settings' pages — one TalkBack stop per entry, each
 * opening its screen. Nothing here changes under a row, so there is no row to land on again.
 */
@Composable
fun MenuScreen(title: String, core: Core, navigator: Navigator, items: List<Item>, open: (Item) -> Unit) {
    ScreenFrame(title, core, navigator) {
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(items.size, 1) }) {
            itemsIndexed(items, key = { _, item -> item.key }) { index, item ->
                ListRow(
                    title = item.title,
                    detail = item.detail,
                    speech = item.detail,
                    index = index,
                    actions = emptyList(),
                    open = { open(item) },
                    openLabel = "Open",
                    key = item.key,
                    say = core::say,
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
    // How many, in the core's words for each group: "no projects", "1 label".
    fun count(group: SidebarGroup, n: Int) = groupCount(group, n.toUInt())
    fun under(group: SidebarGroup) = rows.groupUnder(group).count { it.depth == 1 }
    val trash = rows.firstOrNull { (it.entry.kind as? SidebarKind.Place)?.field0 == Place.Trash }
    val history = rows.firstOrNull { (it.entry.kind as? SidebarKind.Place)?.field0 == Place.History }
    val items = listOf(
        Item("projects", "Projects", count(SidebarGroup.PROJECTS, rows.groupUnder(SidebarGroup.PROJECTS).size)),
        Item("labels", "Labels", count(SidebarGroup.LABELS, under(SidebarGroup.LABELS))),
        Item("filters", "Saved filters", count(SidebarGroup.FILTERS, under(SidebarGroup.FILTERS))),
        Item("blocks", "Blocks", ""),
        Item("history", "History", history?.detail.orEmpty()),
        Item("trash", "Trash", trash?.detail.orEmpty()),
    )
    MenuScreen(
        "Browse", core, navigator, items,
        open = { item ->
            navigator.push(
                when (item.key) {
                    "projects" -> Screen.Projects
                    "labels" -> Screen.Labels
                    "filters" -> Screen.Filters
                    "blocks" -> Screen.Blocks
                    "history" -> Screen.Tasks(Place.History)
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
        heading?.entry?.offered?.firstOrNull { it.kind == ActionKind.NEW }?.let { new ->
            IconButton(onClick = { app.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, heading.key), new)) }) { Icon(Icons.Filled.Add, contentDescription = addLabel) }
        }
    }) {
        // Nothing in it yet, said in the core's words: "No labels."
        if (rows.isEmpty() && heading != null) {
            Text(groupEmpty(group), style = MaterialTheme.typography.bodyMedium, color = quiet(), modifier = Modifier.padding(16.dp))
        }
        PlaceRows(
            core, rows,
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
    LaunchedEffect(Unit) { app.send(ShellEvent.Open(dev.emassey0135.lumenna.app.Place.Blocks)) }
    val here = view?.takeIf { it.place == dev.emassey0135.lumenna.app.Place.Blocks }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    FollowBlockForm(core, navigator)
    ScreenFrame("Blocks", core, navigator, actions = {
        IconButton(onClick = { app.send(ShellEvent.Block(BlockEvent.New(null, null))) }) {
            Icon(Icons.Filled.Add, contentDescription = "Add block")
        }
    }) {
        val count = lines?.readback.orEmpty()
        if (count.isNotEmpty()) {
            Text(
                count,
                style = MaterialTheme.typography.bodyMedium,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        AppTaskRows(core, shownRows(list?.items.orEmpty(), lines?.list.orEmpty()), list, open = { row ->
            // A block's own screen is its form, for every occurrence: its Edit, on the row in hand.
            app.send(ShellEvent.Focus(row.id))
            app.send(ShellEvent.Run(listOf(dev.emassey0135.lumenna.app.ActionKind.EDIT)))
        }, openLabel = "Edit every occurrence")
    }
}
