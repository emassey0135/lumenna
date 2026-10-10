package io.github.emassey0135.lumenna.wear

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.wear.compose.foundation.lazy.ScalingLazyColumn
import androidx.wear.compose.foundation.lazy.ScalingLazyColumnDefaults
import androidx.wear.compose.foundation.lazy.ScalingLazyListScope
import androidx.wear.compose.foundation.lazy.rememberScalingLazyListState
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ScreenScaffold
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.Choice
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.PlaceAction
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.RowSpeech
import io.github.emassey0135.lumenna.folded
import io.github.emassey0135.lumenna.levelChange
import io.github.emassey0135.lumenna.core.Direction
import io.github.emassey0135.lumenna.core.Place
import io.github.emassey0135.lumenna.core.SidebarGroup
import io.github.emassey0135.lumenna.core.SidebarKind
import io.github.emassey0135.lumenna.core.parseWeight
import io.github.emassey0135.lumenna.core.placeQuery
import io.github.emassey0135.lumenna.core.placeQuickAddPrefix
import io.github.emassey0135.lumenna.core.placeTitle
import io.github.emassey0135.lumenna.sentence

/**
 * A screen's list: Wear's scaling list, which the rotary crown scrolls, with rows kept at
 * full size at the screen's edges. Scaled down there, a button fell under the 48dp a touch
 * target needs.
 */
@Composable
fun WearList(content: ScalingLazyListScope.() -> Unit) {
    val state = rememberScalingLazyListState()
    ScreenScaffold(scrollState = state) { padding ->
        ScalingLazyColumn(
            state = state,
            contentPadding = padding,
            modifier = Modifier.fillMaxWidth(),
            scalingParams = ScalingLazyColumnDefaults.scalingParams(edgeScale = 1f),
            content = content,
        )
    }
}

/**
 * The places, as every app's sidebar has them, from the core: Today, Tasks, the projects,
 * labels and saved filters, Blocks and Trash. Headings and project trees fold. Then Undo,
 * Redo and Settings.
 */
@Composable
fun PlacesScreen(core: Core, navigator: Navigator, changes: Long) {
    val entries = remember(changes) { core.lumenna.places().entries.filter { !it.archived } }
    var collapsed by rememberSaveable { mutableStateOf(emptySet<String>()) }
    val key = { kind: SidebarKind ->
        when (kind) {
            is SidebarKind.Place -> "place:${kind.v1}"
            is SidebarKind.Group -> "group:${kind.v1}"
        }
    }
    val shown = folded(entries, { it.depth.toInt() }, { key(it.kind) }, collapsed)
    val entry = LocalTextEntry.current
    WearList {
        item { Button(onClick = { navigator.open(Screen.AddTask("")) }, modifier = Modifier.fillMaxWidth(), label = { Text("New Task") }) }
        shown.forEachIndexed { index, row ->
            item {
                val place = row.item
                val fold = row.state
                val toggle = {
                    val k = key(place.kind)
                    collapsed = if (k in collapsed) collapsed - k else collapsed + k
                    core.say(if (k in collapsed) "Collapsed" else "Expanded")
                }
                val state = listOfNotNull(fold, levelChange(shown, index)).joinToString(", ").ifEmpty { null }
                when (val kind = place.kind) {
                    is SidebarKind.Place -> RowButton(
                        place.text,
                        state = state,
                        actions = if (row.parent) listOf(RowAction(if (row.collapsed) "Expand" else "Collapse") { toggle() }) else emptyList(),
                        onClick = { navigator.open(placeScreen(kind.v1)) },
                    )
                    // A heading folds what is under it when pressed.
                    is SidebarKind.Group -> Button(
                        onClick = { if (row.parent) toggle() },
                        modifier = Modifier.fillMaxWidth().semantics {
                            heading()
                            if (state != null) stateDescription = state
                        },
                        label = { Text(place.text) },
                    )
                }
            }
            val kind = row.item.kind
            if (kind is SidebarKind.Group && !row.collapsed) {
                item { NewPlace(core, entry, kind.v1) }
            }
        }
        item { Button(onClick = { core.change { it.undo() } }, modifier = Modifier.fillMaxWidth(), label = { Text("Undo") }) }
        item { Button(onClick = { core.change { it.redo() } }, modifier = Modifier.fillMaxWidth(), label = { Text("Redo") }) }
        item { Button(onClick = { navigator.open(Screen.Settings) }, modifier = Modifier.fillMaxWidth(), label = { Text("Settings") }) }
    }
}

private fun placeScreen(place: Place): Screen = when (place) {
    Place.Today -> Screen.Day
    Place.Blocks -> Screen.Blocks
    else -> Screen.Tasks(place)
}

/** What the phone's sidebar adds under each heading. */
@Composable
private fun NewPlace(core: Core, entry: TextEntry?, group: SidebarGroup) {
    val (title, ask) = when (group) {
        SidebarGroup.PROJECTS -> "New Project" to { entry?.ask("New project") { name -> core.change { it.addProject(name.trim(), null) } } }
        SidebarGroup.LABELS -> "New Label" to { entry?.ask("New label") { name -> core.change { it.addLabel(name.trim()) } } }
        SidebarGroup.FILTERS -> "New Saved Filter" to {
            entry?.ask("New filter's name") { name ->
                entry.ask("Query for $name") { query -> core.change { it.addFilter(name.trim(), query.trim()) } }
            }
        }
    }
    Button(onClick = { ask() }, modifier = Modifier.fillMaxWidth(), label = { Text(title) })
}

/**
 * A place's tasks, each row as the phone says it: the title, then what the core says of it.
 * Mark Done and Delete are its actions — TalkBack's custom actions, and a long press; in the
 * trash, Restore and Delete from Trash. Subtasks fold. A project, label or filter's own
 * actions are at the foot, as the phone offers them (`PlaceAction`).
 */
@Composable
fun TasksScreen(core: Core, navigator: Navigator, place: Place, changes: Long) {
    val trash = place == Place.Trash
    val listing = remember(changes, place) { core.attempt { core.lumenna.listTasks(placeQuery(place)) } }
    var collapsed by rememberSaveable { mutableStateOf(emptySet<String>()) }
    var erasing by remember { mutableStateOf<String?>(null) }
    val shown = folded(listing?.rows.orEmpty(), { it.depth.toInt() }, { it.id }, collapsed)
    WearList {
        item { Heading(placeTitle(place)) }
        if (!trash) {
            item {
                Button(onClick = { navigator.open(Screen.AddTask(placeQuickAddPrefix(place))) }, modifier = Modifier.fillMaxWidth(), label = { Text("New Task") })
            }
        }
        if (listing != null && listing.rows.isEmpty()) item { Text(listing.announcement.replaceFirstChar { it.uppercase() }) }
        shown.forEachIndexed { index, row ->
            item {
                val task = row.item
                val previous = if (index > 0) shown[index - 1].item.depth else null
                val actions = buildList {
                    if (trash) {
                        add(RowAction("Restore") { core.change { it.restoreTask(task.id) } })
                        add(RowAction("Delete from Trash") {
                            navigator.choose("Delete ${task.title} for good?", listOf(Choice("erase", "Delete for Good")), "This cannot be undone.") {
                                core.change { it.eraseTask(task.id) }
                            }
                        })
                    } else {
                        add(RowAction(if (task.checked == true) "Mark Not Done" else "Mark Done") {
                            core.change { if (task.checked == true) it.uncompleteTask(task.id) else it.completeTask(task.id) }
                        })
                        add(RowAction("Delete") { core.change { it.trashTask(task.id) } })
                    }
                    if (row.parent) {
                        add(RowAction(if (row.collapsed) "Expand" else "Collapse") {
                            collapsed = if (task.id in collapsed) collapsed - task.id else collapsed + task.id
                            core.say(if (task.id in collapsed) "Collapsed" else "Expanded")
                        })
                    }
                }
                RowButton(
                    task.title,
                    detail = RowSpeech.details(task),
                    state = RowSpeech.value(task, previous, row.state),
                    actions = actions,
                    onLongClick = { navigator.actions(task.title, actions) },
                    onClick = { navigator.open(Screen.Task(task.id)) },
                )
            }
        }
        placeActions(core, navigator, place)
    }
}

/** A project's, label's or filter's actions, as buttons at the foot of its list. */
private fun ScalingLazyListScope.placeActions(core: Core, navigator: Navigator, place: Place) {
    val (heading, actions) = when (place) {
        is Place.Project -> {
            val projects = core.attempt { core.lumenna.listProjects().rows }.orEmpty()
            val archived = projects.firstOrNull { it.title == place.v1 }?.state?.contains("archived") == true
            "Project" to PlaceAction.ofProject(archived)
        }
        is Place.Label -> "Label" to PlaceAction.ofLabel
        is Place.Filter -> "Saved filter" to PlaceAction.ofFilter
        else -> return
    }
    item { Heading(heading) }
    actions.forEach { action ->
        item {
            val entry = LocalTextEntry.current
            Button(
                onClick = { runPlaceAction(core, navigator, entry, place, action) },
                modifier = Modifier.fillMaxWidth(),
                label = { Text(action.title) },
            )
        }
    }
}

private fun runPlaceAction(core: Core, navigator: Navigator, entry: TextEntry?, place: Place, action: PlaceAction) {
    val lumenna = core.lumenna
    // After a rename or a delete the list stands for something gone: back to the places.
    val leave = { change: io.github.emassey0135.lumenna.core.Change? -> if (change != null) navigator.back() }
    when (place) {
        is Place.Project -> {
            val name = place.v1
            when (action) {
                PlaceAction.RENAME -> entry?.ask("Rename $name") { renamed -> leave(core.change { it.renameProject(name, renamed.trim()) }) }
                PlaceAction.MOVE_UP -> core.change { it.reorderProject(name, Direction.UP) }
                PlaceAction.MOVE_DOWN -> core.change { it.reorderProject(name, Direction.DOWN) }
                PlaceAction.MOVE_UNDER -> {
                    val others = core.attempt { lumenna.listProjects().rows }.orEmpty().map { it.title }.filter { it != name }
                    navigator.choose("Move $name under", listOf(Choice("", "Top Level")) + others.map { Choice(it, it) }) { parent ->
                        core.change { it.moveProject(name, parent.key.ifEmpty { null }) }
                    }
                }
                PlaceAction.ADD_PROJECT_INSIDE -> entry?.ask("New project in $name") { added -> core.change { it.addProject(added.trim(), name) } }
                PlaceAction.WEIGHT -> entry?.ask("Weight of $name") { text ->
                    val weight = core.attempt { parseWeight(text) } ?: return@ask
                    core.change { it.weighProject(name, weight) }
                }
                PlaceAction.ARCHIVE, PlaceAction.UNARCHIVE -> core.change { it.archiveProject(name) }
                PlaceAction.DELETE -> navigator.choose(
                    "Delete $name?",
                    listOf(Choice("trash", PlaceAction.DELETE_AND_TRASH), Choice("keep", PlaceAction.DELETE_AND_KEEP)),
                    PlaceAction.DELETING_PROJECT,
                ) { chosen -> leave(core.change { it.deleteProject(name, chosen.key == "keep") }) }
                else -> {}
            }
        }
        is Place.Label -> {
            val name = place.v1
            when (action) {
                PlaceAction.RENAME -> entry?.ask("Rename $name") { renamed -> leave(core.change { it.renameLabel(name, renamed.trim()) }) }
                PlaceAction.MOVE_UP -> core.change { it.reorderLabel(name, Direction.UP) }
                PlaceAction.MOVE_DOWN -> core.change { it.reorderLabel(name, Direction.DOWN) }
                PlaceAction.MERGE_INTO -> {
                    val others = core.attempt { lumenna.listLabels().rows }.orEmpty().map { it.title }.filter { it != name }
                    navigator.choose("Merge $name into", others.map { Choice(it, it) }) { into -> leave(core.change { it.mergeLabels(name, into.key) }) }
                }
                PlaceAction.COLOUR -> entry?.ask("Colour of $name") { colour -> core.change { it.recolourLabel(name, colour.trim().ifEmpty { null }) } }
                PlaceAction.DELETE -> navigator.choose("Delete $name?", listOf(Choice("delete", "Delete")), PlaceAction.DELETING_LABEL) {
                    leave(core.change { it.deleteLabel(name) })
                }
                else -> {}
            }
        }
        is Place.Filter -> {
            val name = place.name
            when (action) {
                PlaceAction.RENAME -> entry?.ask("Rename $name") { renamed -> leave(core.change { it.editFilter(name, renamed.trim(), null) }) }
                PlaceAction.CHANGE_QUERY -> entry?.ask("Query for $name") { changed -> leave(core.change { it.editFilter(name, null, changed.trim()) }) }
                PlaceAction.MOVE_UP -> core.change { it.reorderFilter(name, Direction.UP) }
                PlaceAction.MOVE_DOWN -> core.change { it.reorderFilter(name, Direction.DOWN) }
                PlaceAction.DELETE -> navigator.choose("Delete $name?", listOf(Choice("delete", "Delete")), PlaceAction.DELETING_FILTER) {
                    leave(core.change { it.deleteFilter(name) })
                }
                else -> {}
            }
        }
        else -> {}
    }
}

/** Every block series, each opening the form for every occurrence; Delete asks first. */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator, changes: Long) {
    val listing = remember(changes) { core.attempt { core.lumenna.listBlocks() } }
    WearList {
        item { Heading("Blocks") }
        if (listing != null && listing.rows.isEmpty()) item { Text("No blocks") }
        listing?.rows.orEmpty().forEach { row ->
            item {
                val actions = listOf(
                    RowAction("Edit") { navigator.open(Screen.BlockForm(io.github.emassey0135.lumenna.BlockPurpose.Series(row.id))) },
                    RowAction("Delete") {
                        navigator.choose("Delete ${row.title}?", listOf(Choice("delete", "Delete")), "Every occurrence goes, and its assignments.") {
                            core.change { it.deleteBlock(row.id) }
                        }
                    },
                )
                RowButton(row.title, detail = row.value, actions = actions, onLongClick = { navigator.actions(row.title, actions) }) {
                    navigator.open(Screen.BlockForm(io.github.emassey0135.lumenna.BlockPurpose.Series(row.id)))
                }
            }
        }
    }
}

/** A list to choose one from, each choice a button. */
@Composable
fun ChooseScreen(screen: Screen.Choose) {
    WearList {
        item { Heading(screen.title) }
        screen.message?.let { message -> item { Text(message) } }
        if (screen.choices.isEmpty()) item { Text("Nothing to choose from") }
        screen.choices.forEach { choice ->
            item {
                Button(
                    onClick = { screen.chosen(choice) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(choice.title) },
                    secondaryLabel = choice.detail.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                )
            }
        }
    }
}

/** What a change said, for a sheet that has none of its own. */
fun said(change: io.github.emassey0135.lumenna.core.Change) = sentence(change.announcement, change.notices)
