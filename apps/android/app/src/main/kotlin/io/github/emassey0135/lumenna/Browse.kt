package io.github.emassey0135.lumenna

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
import io.github.emassey0135.lumenna.core.Action
import io.github.emassey0135.lumenna.core.SidebarGroup
import io.github.emassey0135.lumenna.core.labelReference
import io.github.emassey0135.lumenna.core.projectReference

/** One line of a list of things: what identifies it, how it reads, and how deep it sits. */
data class Item(
    val key: String,
    val title: String,
    val detail: String = "",
    val depth: Int = 0,
    val speech: String = detail,
    /** What can be done to it: the core's. */
    val actions: List<Action> = emptyList(),
)

/**
 * A list of things with what can be done to each: a heading's worth of count above, one
 * TalkBack stop per item, its actions custom actions and a long press.
 */
@Composable
fun ItemListScreen(
    title: String,
    core: Core,
    navigator: Navigator,
    load: () -> Pair<List<Item>, String>,
    changes: Long,
    addLabel: String? = null,
    add: () -> Unit = {},
    open: ((Item) -> Unit)?,
    openLabel: String = "Open",
    actions: (Item) -> List<RowAction> = { emptyList() },
    header: (@Composable () -> Unit)? = null,
) {
    val loaded = remember(changes) { core.attempt(load) ?: (emptyList<Item>() to "") }
    val (all, count) = loaded
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
            ) to ""
        },
        open = { item ->
            navigator.push(
                when (item.key) {
                    "projects" -> Screen.Projects
                    "labels" -> Screen.Labels
                    "filters" -> Screen.Filters
                    "blocks" -> Screen.Blocks
                    else -> Screen.Tasks(title = "Trash", query = "deleted", trash = true)
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
            items to rows.announcement
        },
        addLabel = "Add project",
        add = { core.addNew(SidebarGroup.PROJECTS, prompt) },
        open = { item ->
            val reference = projectReference(item.title)
            navigator.push(Screen.Tasks(item.title, reference, "$reference "))
        },
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
            } to rows.announcement
        },
        addLabel = "Add label",
        add = { core.addNew(SidebarGroup.LABELS, prompt) },
        open = { item ->
            val reference = labelReference(item.title)
            navigator.push(Screen.Tasks(item.title, reference, "$reference "))
        },
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
        "Saved Filters", core, navigator, changes = changes,
        load = {
            val filters = core.lumenna.listFilters()
            queries.clear()
            filters.filters.forEach { queries[it.name] = it.query }
            filters.filters.map { Item(it.name, it.name, it.query, actions = it.actions) } to filters.announcement
        },
        addLabel = "Add filter",
        add = { core.addNew(SidebarGroup.FILTERS, prompt) },
        open = { item -> navigator.push(Screen.Tasks(item.title, queries[item.key].orEmpty())) },
        openLabel = "Show its tasks",
        actions = { item -> core.offered(item.actions, prompt) },
    )
    prompt.Host()
}

/** Every block series: for the ones not on any day near enough to find from the planner. */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    ItemListScreen(
        "Blocks", core, navigator, changes = changes,
        load = {
            val rows = core.lumenna.listBlocks()
            rows.rows.map { Item(it.id, it.title, it.value.orEmpty(), actions = it.actions) } to rows.announcement
        },
        addLabel = "Add block",
        add = { navigator.push(Screen.BlockForm(BlockPurpose.Add())) },
        open = { item -> navigator.push(Screen.BlockForm(BlockPurpose.Series(item.key))) },
        openLabel = "Edit every occurrence",
        actions = { item ->
            core.offered(item.actions, prompt, form = { navigator.push(Screen.BlockForm(BlockPurpose.Series(it.target))) })
        },
    )
    prompt.Host()
}
