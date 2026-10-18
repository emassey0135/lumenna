package io.github.emassey0135.lumenna

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
import io.github.emassey0135.lumenna.records.SidebarGroup
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

/** Every kind of thing there is to browse, with how many of each. */
@Composable
fun BrowseScreen(core: Core, navigator: Navigator, changes: Long) {
    fun count(n: UInt, noun: String) = if (n == 1u) "1 $noun" else "$n ${noun}s"
    ItemListScreen(
        "Browse", core, navigator, changes = changes,
        load = {
            val l = core.lumenna
            listOf(
                Item("projects", "Projects", count(l.listProjects().count, "project")),
                Item("labels", "Labels", count(l.listLabels().count, "label")),
                Item("filters", "Saved filters", count(l.listFilters().count, "filter")),
                Item("blocks", "Blocks", count(l.listBlocks().count, "block")),
                Item("trash", "Trash", count(l.listTasks("deleted").count, "task")),
            ).let { Listing(it) }
        },
        open = { item ->
            navigator.push(
                when (item.key) {
                    "projects" -> Screen.Projects
                    "labels" -> Screen.Labels
                    "filters" -> Screen.Filters
                    "blocks" -> Screen.Blocks
                    else -> Screen.Tasks(io.github.emassey0135.lumenna.app.Place.Trash)
                },
            )
        },
    )
}

/** The project tree, with weights. Depth is said where it changes. */
@Composable
fun ProjectsScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    ItemListScreen(
        "Projects", core, navigator, changes = changes,
        load = {
            val rows = core.lumenna.listProjects()
            // The level is added as shown, against the row before it once folded.
            val items = rows.rows.map { row ->
                Item(
                    row.title, row.title, (listOfNotNull(row.value) + row.state).joinToString(", "),
                    row.depth.toInt(), RowSpeech.value(row, row.depth), row.actions,
                )
            }
            Listing(items, rows.announcement, rows.empty)
        },
        addLabel = "Add project",
        add = { core.addNew(SidebarGroup.PROJECTS, prompt) },
        open = { item -> navigator.push(Screen.Tasks(io.github.emassey0135.lumenna.app.Place.Project(item.title))) },
        openLabel = "Show its tasks",
        actions = { item -> core.offered(item.actions, prompt) },
    )
    prompt.Host()
}

/** Labels: a first-class axis, with its own list. */
@Composable
fun LabelsScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    ItemListScreen(
        "Labels", core, navigator, changes = changes,
        load = {
            val rows = core.lumenna.listLabels()
            rows.rows.map {
                Item(it.title, it.title, (listOfNotNull(it.value) + it.state).joinToString(", "), actions = it.actions)
            }.let { Listing(it, rows.announcement, rows.empty) }
        },
        addLabel = "Add label",
        add = { core.addNew(SidebarGroup.LABELS, prompt) },
        open = { item -> navigator.push(Screen.Tasks(io.github.emassey0135.lumenna.app.Place.Label(item.title))) },
        openLabel = "Show the tasks wearing it",
        actions = { item -> core.offered(item.actions, prompt) },
    )
    prompt.Host()
}

/** Saved filters: created, renamed, requeried, reordered and deleted, not only run. */
@Composable
fun FiltersScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    val queries = remember(changes) { mutableMapOf<String, String>() }
    ItemListScreen(
        "Saved filters", core, navigator, changes = changes,
        load = {
            val filters = core.lumenna.listFilters()
            queries.clear()
            filters.filters.forEach { queries[it.name] = it.query }
            Listing(filters.filters.map { Item(it.name, it.name, it.query, actions = it.actions) }, filters.announcement, filters.empty)
        },
        addLabel = "Add filter",
        add = { core.addNew(SidebarGroup.FILTERS, prompt) },
        open = { item -> navigator.push(Screen.Tasks(io.github.emassey0135.lumenna.app.Place.Filter(item.title, queries[item.key].orEmpty()))) },
        openLabel = "Show its tasks",
        actions = { item -> core.offered(item.actions, prompt) },
    )
    prompt.Host()
}

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
    AppQuestions(core) {}
}
