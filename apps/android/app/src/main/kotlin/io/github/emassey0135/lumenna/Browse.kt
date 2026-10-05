package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
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
import io.github.emassey0135.lumenna.core.Direction
import io.github.emassey0135.lumenna.core.Weight
import io.github.emassey0135.lumenna.core.labelReference
import io.github.emassey0135.lumenna.core.projectReference

/** One line of a list of things: what identifies it, how it reads, and how deep it sits. */
data class Item(
    val key: String,
    val title: String,
    val detail: String = "",
    val depth: Int = 0,
    val speech: String = detail,
)

/**
 * A list of things with what can be done to each (§16.1's management views): a heading's worth
 * of count above, one TalkBack stop per item, its actions custom actions and a long press.
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
) {
    val loaded = remember(changes) { core.attempt(load) ?: (emptyList<Item>() to "") }
    val (items, count) = loaded
    ScreenFrame(title, core, navigator, actions = {
        if (addLabel != null) {
            IconButton(onClick = add) { Icon(Icons.Filled.Add, contentDescription = addLabel) }
        }
    }) {
        if (count.isNotEmpty()) {
            Text(
                count.replaceFirstChar { it.uppercase() },
                style = MaterialTheme.typography.bodyMedium,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
        }
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(items.size, 1) }) {
            itemsIndexed(items, key = { _, item -> item.key }) { index, item ->
                ListRow(
                    title = item.title,
                    detail = item.detail,
                    speech = item.speech,
                    index = index,
                    depth = item.depth,
                    actions = actions(item),
                    open = open?.let { { it(item) } },
                    openLabel = openLabel,
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

/** The project tree, with weights (§3.4). Depth is said where it changes (§16.11). */
@Composable
fun ProjectsScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    val archived = remember { mutableSetOf<String>() }
    val all = remember(changes) { mutableListOf<Item>() }
    ItemListScreen(
        "Projects", core, navigator, changes = changes,
        load = {
            val rows = core.lumenna.listProjects()
            archived.clear()
            archived += rows.rows.filter { "archived" in it.state }.map { it.title }
            var previous: UInt? = null
            val items = rows.rows.map { row ->
                Item(
                    row.title, row.title, (listOfNotNull(row.value) + row.state).joinToString(", "),
                    row.depth.toInt(), RowSpeech.value(row, previous),
                ).also { previous = row.depth }
            }
            all.clear()
            all += items
            items to rows.announcement
        },
        addLabel = "Add project",
        add = {
            prompt.show {
                AskText("New Project", "Name", "Add", dismiss = prompt::close) { name ->
                    prompt.close()
                    core.change { it.addProject(name.trim(), null) }
                }
            }
        },
        open = { item ->
            val reference = projectReference(item.title)
            navigator.push(Screen.Tasks(item.title, reference, "$reference "))
        },
        openLabel = "Show its tasks",
        actions = { item ->
            listOf(
                RowAction("Rename") {
                    prompt.show {
                        AskText("Rename ${item.title}", "Name", "Rename", initial = item.title, dismiss = prompt::close) { name ->
                            prompt.close()
                            core.change { it.renameProject(item.key, name.trim()) }
                        }
                    }
                },
                RowAction("Move Up") { core.change { it.reorderProject(item.key, Direction.UP) } },
                RowAction("Move Down") { core.change { it.reorderProject(item.key, Direction.DOWN) } },
                RowAction("Move Under") {
                    prompt.show {
                        val choices = listOf(Choice("", "Top Level")) + all.filter { it.key != item.key }.map { Choice(it.key, it.title) }
                        Choose("Move ${item.title} under", choices, "", prompt::close) { parent ->
                            prompt.close()
                            core.change { it.moveProject(item.key, parent.key.ifEmpty { null }) }
                        }
                    }
                },
                RowAction("Add Project Inside") {
                    prompt.show {
                        AskText("New Project in ${item.title}", "Name", "Add", dismiss = prompt::close) { name ->
                            prompt.close()
                            core.change { it.addProject(name.trim(), item.key) }
                        }
                    }
                },
                RowAction("Weight") {
                    prompt.show {
                        AskText(
                            "Weight of ${item.title}", "Weight", "Set", example = "1.0",
                            hint = "How much this whole area matters now, roughly 0.5 to 2. Type inherit to take the parent's again.",
                            dismiss = prompt::close,
                        ) { text ->
                            prompt.close()
                            val weight = text.trim().toFloatOrNull()?.let { Weight.Value(it) } ?: Weight.Inherit
                            core.change { it.weighProject(item.key, weight) }
                        }
                    }
                },
                RowAction(if (item.key in archived) "Unarchive" else "Archive") {
                    core.change { it.archiveProject(item.key) }
                },
                RowAction("Delete") {
                    prompt.show {
                        AlertDialog(
                            onDismissRequest = prompt::close,
                            title = { Text("Delete ${item.title}?") },
                            text = { Text("Its tasks can go to the trash with it, or move to the Inbox.") },
                            confirmButton = {
                                Column {
                                    TextButton(modifier = Target, onClick = {
                                        prompt.close()
                                        core.change { it.deleteProject(item.key, false) }
                                    }) { Text("Delete and Trash Its Tasks") }
                                    TextButton(modifier = Target, onClick = {
                                        prompt.close()
                                        core.change { it.deleteProject(item.key, true) }
                                    }) { Text("Delete and Keep Its Tasks") }
                                    TextButton(modifier = Target, onClick = prompt::close) { Text("Cancel") }
                                }
                            },
                        )
                    }
                },
            )
        },
    )
    prompt.Host()
}

/** Labels: a first-class axis, with its own list (§16.1). */
@Composable
fun LabelsScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    val all = remember(changes) { mutableListOf<Item>() }
    ItemListScreen(
        "Labels", core, navigator, changes = changes,
        load = {
            val rows = core.lumenna.listLabels()
            val items = rows.rows.map { Item(it.title, it.title, (listOfNotNull(it.value) + it.state).joinToString(", ")) }
            all.clear()
            all += items
            items to rows.announcement
        },
        addLabel = "Add label",
        add = {
            prompt.show {
                AskText("New Label", "Name", "Add", dismiss = prompt::close) { name ->
                    prompt.close()
                    core.change { it.addLabel(name.trim()) }
                }
            }
        },
        open = { item ->
            val reference = labelReference(item.title)
            navigator.push(Screen.Tasks(item.title, reference, "$reference "))
        },
        openLabel = "Show the tasks wearing it",
        actions = { item ->
            listOf(
                RowAction("Rename") {
                    prompt.show {
                        AskText("Rename ${item.title}", "Name", "Rename", initial = item.title, dismiss = prompt::close) { name ->
                            prompt.close()
                            core.change { it.renameLabel(item.key, name.trim()) }
                        }
                    }
                },
                RowAction("Move Up") { core.change { it.reorderLabel(item.key, Direction.UP) } },
                RowAction("Move Down") { core.change { it.reorderLabel(item.key, Direction.DOWN) } },
                RowAction("Merge Into") {
                    prompt.show {
                        Choose("Merge ${item.title} into", all.filter { it.key != item.key }.map { Choice(it.key, it.title) }, "There are no other labels.", prompt::close) { into ->
                            prompt.close()
                            core.change { it.mergeLabels(item.key, into.key) }
                        }
                    }
                },
                RowAction("Colour") {
                    prompt.show {
                        AskText(
                            "Colour of ${item.title}", "Colour", "Set", example = "teal",
                            hint = "A colour name, such as teal or orange. Empty for none.", dismiss = prompt::close,
                        ) { colour ->
                            prompt.close()
                            core.change { it.recolourLabel(item.key, colour.trim().ifEmpty { null }) }
                        }
                    }
                },
                RowAction("Delete") {
                    prompt.show {
                        Confirm("Delete ${item.title}?", "Tasks wearing it stay; they just stop showing it.", "Delete", prompt::close) {
                            prompt.close()
                            core.change { it.deleteLabel(item.key) }
                        }
                    }
                },
            )
        },
    )
    prompt.Host()
}

/** Saved filters: created, renamed, requeried, reordered and deleted, not only run (§16.1). */
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
            filters.filters.map { Item(it.name, it.name, it.query) } to filters.announcement
        },
        addLabel = "Add filter",
        add = {
            prompt.show {
                AskText("New Filter", "Name", "Next", dismiss = prompt::close) { name ->
                    prompt.show {
                        AskText("Query for $name", "Query", "Save", example = "#Work & overdue", dismiss = prompt::close) { query ->
                            prompt.close()
                            core.change { it.addFilter(name.trim(), query.trim()) }
                        }
                    }
                }
            }
        },
        open = { item -> navigator.push(Screen.Tasks(item.title, queries[item.key].orEmpty())) },
        openLabel = "Show its tasks",
        actions = { item ->
            listOf(
                RowAction("Rename") {
                    prompt.show {
                        AskText("Rename ${item.title}", "Name", "Rename", initial = item.title, dismiss = prompt::close) { name ->
                            prompt.close()
                            core.change { it.editFilter(item.key, name.trim(), null) }
                        }
                    }
                },
                RowAction("Change Query") {
                    prompt.show {
                        AskText("Query for ${item.title}", "Query", "Save", initial = queries[item.key].orEmpty(), dismiss = prompt::close) { query ->
                            prompt.close()
                            core.change { it.editFilter(item.key, null, query.trim()) }
                        }
                    }
                },
                RowAction("Move Up") { core.change { it.reorderFilter(item.key, Direction.UP) } },
                RowAction("Move Down") { core.change { it.reorderFilter(item.key, Direction.DOWN) } },
                RowAction("Delete") {
                    prompt.show {
                        Confirm("Delete ${item.title}?", "The tasks it shows are not touched.", "Delete", prompt::close) {
                            prompt.close()
                            core.change { it.deleteFilter(item.key) }
                        }
                    }
                },
            )
        },
    )
    prompt.Host()
}

/** Every block series: for the ones not on any day near enough to find from the planner (§3.6). */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    ItemListScreen(
        "Blocks", core, navigator, changes = changes,
        load = {
            val rows = core.lumenna.listBlocks()
            rows.rows.map { Item(it.id, it.title, it.value.orEmpty()) } to rows.announcement
        },
        addLabel = "Add block",
        add = { navigator.push(Screen.BlockForm(BlockPurpose.Add())) },
        open = { item -> navigator.push(Screen.BlockForm(BlockPurpose.Series(item.key))) },
        openLabel = "Edit every occurrence",
        actions = { item ->
            listOf(
                RowAction("Edit") { navigator.push(Screen.BlockForm(BlockPurpose.Series(item.key))) },
                RowAction("Delete") {
                    val repeats = core.attempt { core.lumenna.showBlock(item.key).repeats } ?: false
                    prompt.show {
                        Confirm(
                            "Delete ${item.title}?",
                            if (repeats) "Every occurrence goes. To skip one day, cancel it from the day instead."
                            else "It goes with its assignments.",
                            "Delete",
                            prompt::close,
                        ) {
                            prompt.close()
                            core.change { it.deleteBlock(item.key) }
                        }
                    }
                },
            )
        },
    )
    prompt.Host()
}
