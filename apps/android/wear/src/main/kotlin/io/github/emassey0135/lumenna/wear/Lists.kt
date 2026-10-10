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
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.perform
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.RowSpeech
import io.github.emassey0135.lumenna.folded
import io.github.emassey0135.lumenna.levelChange
import io.github.emassey0135.lumenna.core.ActionKind
import io.github.emassey0135.lumenna.core.SidebarEntry
import io.github.emassey0135.lumenna.core.Subject
import io.github.emassey0135.lumenna.core.Place
import io.github.emassey0135.lumenna.core.SidebarGroup
import io.github.emassey0135.lumenna.core.SidebarKind
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
            if (row.item.kind is SidebarKind.Group && !row.collapsed) {
                item { NewPlace(core, navigator, entry, row.item) }
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

/** What a heading adds under it, as every app's sidebar has it: the core's New Project and so on. */
@Composable
private fun NewPlace(core: Core, navigator: Navigator, entry: TextEntry?, heading: SidebarEntry) {
    core.offered(heading.actions, navigator, entry, form = { action ->
        // A new saved filter's form: its name, then its query.
        if (action.subject == Subject.FILTER && action.kind == ActionKind.NEW) {
            entry?.ask("New filter's name") { name ->
                entry.ask("Query for $name") { query -> core.change { it.addFilter(name.trim(), query.trim()) } }
            }
        }
    }).forEach { action ->
        Button(onClick = action.run, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) })
    }
}

/**
 * A place's tasks, each row as the phone says it: the title, then what the core says of it.
 * Its actions are the core's — TalkBack's custom actions, and a long press. Subtasks fold. A
 * project, label or filter's own actions are at the foot, the core's too.
 */
@Composable
fun TasksScreen(core: Core, navigator: Navigator, place: Place, changes: Long) {
    val trash = place == Place.Trash
    val listing = remember(changes, place) { core.attempt { core.lumenna.listTasks(placeQuery(place)) } }
    var collapsed by rememberSaveable { mutableStateOf(emptySet<String>()) }
    val entry = LocalTextEntry.current
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
                // The core's, in its order: in the trash, Restore and Delete from Trash.
                val actions = core.offered(task.actions, navigator, entry, form = { navigator.open(Screen.Task(it.target)) }) +
                    if (row.parent) listOf(RowAction(if (row.collapsed) "Expand" else "Collapse") {
                        collapsed = if (task.id in collapsed) collapsed - task.id else collapsed + task.id
                        core.say(if (task.id in collapsed) "Collapsed" else "Expanded")
                    }) else emptyList()
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

/** A project's, label's or filter's actions, the core's, as buttons at the foot of its list. */
private fun ScalingLazyListScope.placeActions(core: Core, navigator: Navigator, place: Place) {
    val heading = when (place) {
        is Place.Project -> "Project"
        is Place.Label -> "Label"
        is Place.Filter -> "Saved filter"
        else -> return
    }
    val entry = core.attempt { core.lumenna.places() }?.entries?.firstOrNull { (it.kind as? SidebarKind.Place)?.v1 == place } ?: return
    item { Heading(heading) }
    entry.actions.forEach { action ->
        item {
            val text = LocalTextEntry.current
            Button(
                onClick = {
                    // After a rename, a merge or a delete, the list stands for something gone.
                    core.perform(action, WearAsker(navigator, text), form = {}) {
                        if (action.kind in leaving) navigator.back()
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                label = { Text(action.title) },
            )
        }
    }
}

private val leaving = setOf(ActionKind.RENAME, ActionKind.DELETE, ActionKind.MERGE_INTO, ActionKind.CHANGE_QUERY)

/** Every block series, each opening the form for every occurrence; Delete asks first. */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator, changes: Long) {
    val listing = remember(changes) { core.attempt { core.lumenna.listBlocks() } }
    WearList {
        item { Heading("Blocks") }
        if (listing != null && listing.rows.isEmpty()) item { Text("No blocks") }
        listing?.rows.orEmpty().forEach { row ->
            item {
                val entry = LocalTextEntry.current
                val actions = core.offered(row.actions, navigator, entry, form = {
                    navigator.open(Screen.BlockForm(io.github.emassey0135.lumenna.BlockPurpose.Series(it.target)))
                })
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
