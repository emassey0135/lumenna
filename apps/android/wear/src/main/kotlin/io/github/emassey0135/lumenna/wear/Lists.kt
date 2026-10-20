package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.rememberLanding
import io.github.emassey0135.lumenna.rememberSideLanding
import io.github.emassey0135.lumenna.app.Side
import androidx.wear.compose.foundation.lazy.ScalingLazyListState
import io.github.emassey0135.lumenna.rowActions
import io.github.emassey0135.lumenna.placeRows
import io.github.emassey0135.lumenna.shownRows
import io.github.emassey0135.lumenna.spoken
import io.github.emassey0135.lumenna.app.placeTitle
import io.github.emassey0135.lumenna.app.SidebarKind
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.ActionKind
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
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
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.levelChange
import io.github.emassey0135.lumenna.detailOf
import io.github.emassey0135.lumenna.sentence
import io.github.emassey0135.lumenna.name

/**
 * A screen's list: Wear's scaling list, which the rotary crown scrolls, with rows kept at
 * full size at the screen's edges. Scaled down there, a button fell under the 48dp a touch
 * target needs.
 */
@Composable
fun WearList(state: ScalingLazyListState = rememberScalingLazyListState(), content: ScalingLazyListScope.() -> Unit) {
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
 * The places, as every app's sidebar has them, from the app: Today, Tasks, the projects,
 * labels and saved filters, Blocks and Trash. Headings and project trees fold, through the
 * app; a heading's own action (New Project, …) beneath it. Then Undo, Redo and Settings.
 */
@Composable
fun PlacesScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val rows = placeRows(view?.sidebar.orEmpty().filter { !it.archived }).filter { !it.hidden }
    // Every item in the list's order, a row by its key, so a row's place is where to scroll.
    val keys = buildList {
        add("\u0000new")
        rows.forEach { row ->
            add(row.key)
            if (row.entry.kind is SidebarKind.Group && row.expanded != false) {
                row.entry.actions.filter { it.kind == ActionKind.NEW }.forEach { add("\u0000${it.name}") }
            }
        }
    }
    val state = rememberScalingLazyListState()
    val focus = rememberSideLanding(core, Side.SIDEBAR, keys) { state.scrollToItem(it) }
    WearList(state) {
        item { Button(onClick = { navigator.open(Screen.AddTask) }, modifier = Modifier.fillMaxWidth(), label = { Text("New task") }) }
        rows.forEachIndexed { index, row ->
            item {
                val previous = if (index > 0) rows[index - 1].depth else 0
                val level = if (row.depth != previous) "level ${row.depth + 1}" else null
                val fold = row.entry.actions.firstOrNull { it.kind == ActionKind.EXPAND || it.kind == ActionKind.COLLAPSE }
                val state = listOfNotNull(row.expanded?.let { if (it) "expanded" else "collapsed" }, level).joinToString(", ").ifEmpty { null }
                when (val kind = row.entry.kind) {
                    is SidebarKind.Place -> RowButton(
                        row.entry.text,
                        state = state,
                        actions = listOfNotNull(fold).let { app.rowActions(row.key, it) },
                        key = row.key,
                        focus = focus,
                        onClick = { navigator.open(placeScreen(kind.field0)) },
                    )
                    // A heading folds what is under it when pressed.
                    is SidebarKind.Group -> Button(
                        onClick = { fold?.let { app.send(ShellEvent.Perform(it)) } },
                        modifier = Modifier.fillMaxWidth().semantics {
                            heading()
                            if (state != null) stateDescription = state
                        },
                        // In sentence case, as the app words a heading for Wear OS.
                        label = { Text(row.entry.text) },
                    )
                }
            }
            // What a heading adds under it, as every app's sidebar has it: the core's New Project and so on.
            if (row.entry.kind is SidebarKind.Group && row.expanded != false) {
                row.entry.actions.filter { it.kind == ActionKind.NEW }.forEach { new ->
                    item { Button(onClick = { app.send(ShellEvent.Perform(new)) }, modifier = Modifier.fillMaxWidth(), label = { Text(new.name) }) }
                }
            }
        }
        item { Button(onClick = { app.send(ShellEvent.Undo) }, modifier = Modifier.fillMaxWidth(), label = { Text("Undo") }) }
        item { Button(onClick = { app.send(ShellEvent.Redo) }, modifier = Modifier.fillMaxWidth(), label = { Text("Redo") }) }
        item { Button(onClick = { navigator.open(Screen.Settings) }, modifier = Modifier.fillMaxWidth(), label = { Text("Settings") }) }
    }
}

private fun placeScreen(place: Place): Screen = when (place) {
    Place.Today -> Screen.Day
    Place.Blocks -> Screen.Blocks
    else -> Screen.Tasks(place)
}

/**
 * A place's tasks, the app's list, each row as the phone says it: the title, then what the core
 * says of it. Its actions are the app's — TalkBack's custom actions, and a long press — the
 * primary ones first. Subtasks fold. A project, label or filter's own actions are at the foot.
 */
@Composable
fun TasksScreen(core: Core, navigator: Navigator, place: Place) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(place) { app.send(ShellEvent.Open(place)) }
    val trash = place == Place.Trash
    val here = view?.takeIf { it.place == place }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    val items = shownRows(list?.items.orEmpty(), lines?.list.orEmpty())
    val keys = buildList {
        add("\u0000heading")
        if (!trash) add("\u0000new")
        if (list != null && items.isEmpty()) add("\u0000empty")
        items.forEach { (shown, _) -> add(shown.row.id) }
    }
    val state = rememberScalingLazyListState()
    // TalkBack goes to the row the app moved, as on the phone.
    val focus = rememberLanding(core, keys) { state.scrollToItem(it) }
    WearList(state) {
        item { Heading(placeTitle(place)) }
        if (!trash) {
            item { Button(onClick = { navigator.open(Screen.AddTask) }, modifier = Modifier.fillMaxWidth(), label = { Text("New task") }) }
        }
        if (list != null && items.isEmpty()) item { Text(lines?.readback.orEmpty()) }
        items.forEach { (shown, line) ->
            item {
                val task = shown.row
                val actions = app.rowActions(task.id, task.actions).sortedByDescending { it.primary }
                RowButton(
                    task.title,
                    detail = line?.detail?.ifEmpty { null },
                    state = spoken(shown, line).ifEmpty { null },
                    actions = actions,
                    onLongClick = { navigator.actions(task.title, actions) },
                    key = task.id,
                    focus = focus,
                    onClick = {
                        app.send(ShellEvent.Focus(task.id))
                        navigator.open(Screen.Task(task.id))
                    },
                )
            }
        }
        // A project's, label's or filter's actions, the app's, as buttons at the foot of its list.
        val heading = when (place) {
            is Place.Project -> "Project"
            is Place.Label -> "Label"
            is Place.Filter -> "Saved filter"
            else -> null
        }
        val entry = view?.sidebar.orEmpty().firstOrNull { (it.kind as? SidebarKind.Place)?.field0 == place }
        if (heading != null && entry != null) {
            item { Heading(heading) }
            entry.actions.filter { it.kind != ActionKind.EXPAND && it.kind != ActionKind.COLLAPSE }.forEach { action ->
                item {
                    Button(onClick = {
                        app.send(ShellEvent.Perform(action))
                        // After a rename, a merge or a delete, the list stands for something gone.
                        if (action.kind in leaving) navigator.back()
                    }, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) })
                }
            }
        }
    }
}

private val leaving = setOf(ActionKind.RENAME, ActionKind.DELETE, ActionKind.MERGE_INTO, ActionKind.CHANGE_QUERY)

/** Every block series, the app's Blocks place, each opening the app's form for every occurrence; Delete asks first. */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Open(Place.Blocks)) }
    val here = view?.takeIf { it.place == Place.Blocks }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    val keys = buildList {
        add("\u0000heading")
        if (list != null && list.items.isEmpty()) add("\u0000empty")
        list?.items.orEmpty().forEach { add(it.row.id) }
    }
    val state = rememberScalingLazyListState()
    val focus = rememberLanding(core, keys) { state.scrollToItem(it) }
    WearList(state) {
        item { Heading("Blocks") }
        if (list != null && list.items.isEmpty()) item { Text(list.empty) }
        list?.items.orEmpty().forEachIndexed { index, shown ->
            item {
                val row = shown.row
                val line = lines?.list?.getOrNull(index)
                val actions = app.rowActions(row.id, row.actions)
                val edit = row.actions.firstOrNull { it.kind == ActionKind.EDIT }
                // Said as a task row is: when, in this watch's clock, then how long.
                RowButton(row.title, detail = line?.detail?.ifEmpty { null }, actions = actions, onLongClick = { navigator.actions(row.title, actions) }, key = row.id, focus = focus) {
                    edit?.let {
                        app.send(ShellEvent.Focus(row.id))
                        app.send(ShellEvent.Perform(it))
                    }
                }
            }
        }
    }
}

/** A list to choose one from, each choice a button. */
@Composable
fun ChooseScreen(screen: Screen.Choose) {
    DisposableEffect(screen) { onDispose { if (!screen.answered) screen.cancelled() } }
    WearList {
        item { Heading(screen.title) }
        screen.message?.let { message -> item { Text(message) } }
        if (screen.choices.isEmpty()) item { Text("Nothing to choose from") }
        screen.choices.forEachIndexed { index, choice ->
            item {
                // In a tree, the level where it changes beside the detail, as a tree's rows say it.
                // Why it is offered ("Suggested because …") beneath, as its description.
                val detail = listOf(detailOf(screen.choices, index), choice.reason).filter { it.isNotEmpty() }.joinToString("\n")
                Button(
                    onClick = { screen.chosen(choice) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(choice.title) },
                    secondaryLabel = detail.takeIf { it.isNotEmpty() }?.let { { Text(it, maxLines = Int.MAX_VALUE) } },
                )
            }
        }
    }
}
