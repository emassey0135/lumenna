package dev.emassey0135.lumenna.wear

import dev.emassey0135.lumenna.rowKeys
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.app.foldState
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.rememberLanding
import dev.emassey0135.lumenna.rememberSideLanding
import dev.emassey0135.lumenna.app.Side
import androidx.wear.compose.foundation.lazy.ScalingLazyListState
import dev.emassey0135.lumenna.rowActions
import dev.emassey0135.lumenna.offeredOf
import dev.emassey0135.lumenna.arrangement
import dev.emassey0135.lumenna.app.RootRow
import dev.emassey0135.lumenna.app.SidebarEntry
import dev.emassey0135.lumenna.shownRows
import dev.emassey0135.lumenna.spoken
import dev.emassey0135.lumenna.app.placeTitle
import dev.emassey0135.lumenna.app.SidebarKind
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ActionKind
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
import androidx.compose.ui.semantics.contentDescription
import androidx.wear.compose.foundation.lazy.ScalingLazyColumn
import androidx.wear.compose.foundation.lazy.ScalingLazyColumnDefaults
import androidx.wear.compose.foundation.lazy.ScalingLazyListScope
import androidx.wear.compose.foundation.lazy.rememberScalingLazyListState
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ScreenScaffold
import androidx.wear.compose.material3.Text
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.RowAction
import dev.emassey0135.lumenna.detailOf
import dev.emassey0135.lumenna.sentence
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.AppModel
import dev.emassey0135.lumenna.app.ScreenAction

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
 * The watch's root list, the core's (`Arrangement.watch`): New task, every place as the sidebar
 * has it (headings and project trees folding through the app, a heading's own New beneath it
 * while it is unfolded), then the rest of Home's own actions a watch that syncs itself has: Undo,
 * Redo, What should I work on?, Sync now and Settings. An action is `ShellEvent.Key(id)`, named
 * by its title; what it opens is drawn where the app's is (quick add's line by `Follow`, What
 * should I work on?'s pick by `WearQuestions`, Sync now's sentences as every announcement), and
 * Settings is this watch's own screen, which asks the app for its settings.
 */
@Composable
fun PlacesScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val sidebar = view?.sidebar.orEmpty()
    val rows = remember(sidebar) { arrangement(sidebar).watch }
    /** A heading's New while what is under it is shown, as every app's sidebar has it. */
    fun newUnder(entry: SidebarEntry) =
        if (entry.kind is SidebarKind.Group && entry.expanded != false) entry.offered.filter { it.kind == ActionKind.NEW } else emptyList()
    // Every item in the list's order, a row by its key, so a row's place is where to scroll.
    val keys = buildList {
        rows.forEach { row ->
            when (row) {
                is RootRow.Action -> add("\u0000${row.field0.id}")
                is RootRow.Place -> {
                    add(row.field0.key)
                    newUnder(row.field0).forEach { add("\u0000${it.name}") }
                }
            }
        }
    }
    val state = rememberScalingLazyListState()
    val focus = rememberSideLanding(core, Side.SIDEBAR, keys) { state.scrollToItem(it) }
    WearList(state) {
        rows.forEach { root ->
            when (root) {
                is RootRow.Action -> item {
                    val action = root.field0
                    Button(
                        onClick = { if (action.id == "settings") navigator.open(Screen.Settings) else app.send(ShellEvent.Key(action.id)) },
                        enabled = action.enabled,
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(action.title) },
                    )
                }
                is RootRow.Place -> {
                    val entry = root.field0
                    item {
                        val fold = entry.offered.firstOrNull { it.kind == ActionKind.EXPAND || it.kind == ActionKind.COLLAPSE }
                        // Folding and the level where it changes, the core's words.
                        val state = listOfNotNull(entry.expanded?.let(::foldState), entry.level).joinToString(", ").ifEmpty { null }
                        when (val kind = entry.kind) {
                            is SidebarKind.Place -> RowButton(
                                entry.text,
                                state = state,
                                actions = listOfNotNull(fold).let { app.rowActions(entry.key, it, side = Side.SIDEBAR) },
                                key = entry.key,
                                focus = focus,
                                swatch = entry.swatch,
                                onClick = { navigator.open(placeScreen(kind.field0)) },
                            )
                            // A heading folds what is under it when pressed.
                            is SidebarKind.Group -> Button(
                                onClick = { fold?.let { app.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, entry.key), it)) } },
                                // One description, its name then whether it is folded, as a row's.
                                modifier = Modifier.fillMaxWidth().semantics {
                                    heading()
                                    contentDescription = listOfNotNull(entry.text, state).joinToString(", ")
                                },
                                // In sentence case, as the app words a heading for Wear OS.
                                label = { Text(entry.text) },
                            )
                        }
                    }
                    // What a heading adds under it: the core's New project and so on.
                    newUnder(entry).forEach { new ->
                        item { Button(onClick = { app.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, entry.key), new)) }, modifier = Modifier.fillMaxWidth(), label = { Text(new.name) }) }
                    }
                }
            }
        }
    }
}

/**
 * A screen's own actions, the core's (`ViewModel.screen_actions`), a button each in its order,
 * named by its title and disabled while it does nothing now; each sent as `ShellEvent.Key(id)`,
 * the app opening what it opens (quick add, the block form, a question). What is the shell's own
 * is drawn by [own], by `id` (the filter is its field), and left out where it has none.
 */
fun ScalingLazyListScope.screenActionItems(
    app: AppModel,
    actions: List<ScreenAction>,
    own: Map<String, @Composable () -> Unit> = emptyMap(),
) {
    shownActions(actions, own).forEach { action ->
        val drawn = own[action.id]
        item {
            if (action.shell && drawn != null) {
                drawn()
            } else {
                Button(
                    onClick = { app.send(ShellEvent.Key(action.id)) },
                    enabled = action.enabled,
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(action.title) },
                )
            }
        }
    }
}

/** What [screenActionItems] draws of [actions], one item each: the shell's own only where [own] draws it. */
fun shownActions(actions: List<ScreenAction>, own: Map<String, *>): List<ScreenAction> =
    actions.filter { !it.shell || it.id in own }

internal fun placeScreen(place: Place): Screen = when (place) {
    Place.Today -> Screen.Day()
    Place.Blocks -> Screen.Blocks
    else -> Screen.Tasks(place)
}

/**
 * A place's tasks, the app's list, each row as the phone says it: the title, then what the core
 * says of it. Its actions are the app's — TalkBack's custom actions, and a long press — the
 * primary ones first. Subtasks fold. The place's own actions (New task, the filter, Undo, Redo)
 * are the core's, at the top; a project, label or filter's own actions are at the foot.
 */
@Composable
fun TasksScreen(core: Core, navigator: Navigator, place: Place) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(place) { app.send(ShellEvent.Open(place)) }
    // Where a place's action leads is the app's (merged, deleted): when the app moves on from
    // the place shown, the screen goes with it, as the phone's does. Only while this screen is
    // the one shown, so a place opened over it is not taken for a move.
    val appPlace = view?.place
    var lastPlace by remember { mutableStateOf(appPlace) }
    LaunchedEffect(appPlace) {
        val was = lastPlace
        lastPlace = appPlace
        if (appPlace == null || was != place || appPlace == place || navigator.current() != Screen.Tasks(place)) return@LaunchedEffect
        // A day shown from the list (Upcoming's day or block) goes over it, so back comes back
        // to the list; any other place takes the one left, which may be gone.
        if (appPlace == Place.Today) navigator.open(placeScreen(appPlace)) else navigator.replace(placeScreen(appPlace))
    }
    // Narrowing the list, as the phone's field above it does: the app's filter, and why what
    // was typed does not read yet.
    var filter by rememberSaveable(place) { mutableStateOf("") }
    val here = view?.takeIf { it.place == place }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    val items = shownRows(list?.items.orEmpty(), lines?.list.orEmpty())
    // The place's own actions, the core's (New task, the filter, Undo, Redo), first: the filter
    // is the shell's own, the line the app reads as it is typed.
    val own: Map<String, @Composable () -> Unit> = mapOf(
        "filter" to {
            TextFieldButton(appWords.quickAdd.filter, filter, appWords.quickAdd.filterHint) {
                filter = it
                app.send(ShellEvent.Filter(it))
            }
        },
    )
    val actions = here?.screenActions.orEmpty()
    val keys = buildList {
        add("\u0000heading")
        shownActions(actions, own).forEach { add("\u0000${it.id}") }
        if (list?.refusal != null) add("\u0000refusal")
        if (list != null && items.isEmpty()) add("\u0000empty")
        items.forEach { (shown, _) -> add(shown.row.id) }
    }
    val state = rememberScalingLazyListState()
    // TalkBack goes to the row the app moved, as on the phone.
    val focus = rememberLanding(core, keys) { state.scrollToItem(it) }
    WearList(state) {
        item { Heading(placeTitle(place)) }
        screenActionItems(app, actions, own)
        list?.refusal?.let { item { Text(it) } }
        if (list != null && items.isEmpty()) item { Text(lines?.readback.orEmpty()) }
        items.forEach { (shown, line) ->
            item {
                val task = shown.row
                val actions = app.rowActions(task.id, list?.offeredOf(shown).orEmpty()).sortedByDescending { it.primary }
                // The line's first part, as the core words it: a task's title, a day as this
                // device says one, a block with its times (Upcoming's).
                val title = line?.title?.ifEmpty { null } ?: task.title
                RowButton(
                    title,
                    detail = line?.detail?.ifEmpty { null },
                    state = spoken(shown, line).ifEmpty { null },
                    actions = actions,
                    onLongClick = { navigator.actions(title, actions) },
                    key = task.id,
                    focus = focus,
                    swatch = task.swatch,
                    onClick = {
                        if (task.role == "task") {
                            app.send(ShellEvent.Focus(task.id))
                            navigator.open(Screen.Task(task.id))
                        } else {
                            // A day or a block of the days ahead: its Enter, the core's (Show the day).
                            app.rowKeys(task.id).key("open")
                        }
                    },
                )
            }
        }
        // A project's, label's or filter's actions, the app's, as buttons at the foot of its list.
        val heading = dev.emassey0135.lumenna.app.placeHeading(place, true).ifEmpty { null }
        val entry = view?.sidebar.orEmpty().firstOrNull { (it.kind as? SidebarKind.Place)?.field0 == place }
        if (heading != null && entry != null) {
            item { Heading(heading) }
            entry.offered.filter { it.kind != ActionKind.EXPAND && it.kind != ActionKind.COLLAPSE }.forEach { action ->
                item {
                    Button(onClick = {
                        app.send(ShellEvent.ActOn(ActingOn.Held(Side.SIDEBAR, entry.key), action))
                    }, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) })
                }
            }
        }
    }
}

/**
 * Every block series, the app's Blocks place, each opening the app's form for every occurrence;
 * Delete asks first. New block, the core's, opens the form for a new one.
 */
@Composable
fun BlocksScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Open(Place.Blocks)) }
    val here = view?.takeIf { it.place == Place.Blocks }
    val list = here?.list
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { here != null }
    // The place's own actions, the core's (New block, Undo, Redo), first.
    val actions = here?.screenActions.orEmpty()
    val keys = buildList {
        add("\u0000heading")
        shownActions(actions, emptyMap<String, Unit>()).forEach { add("\u0000${it.id}") }
        if (list != null && list.items.isEmpty()) add("\u0000empty")
        list?.items.orEmpty().forEach { add(it.row.id) }
    }
    val state = rememberScalingLazyListState()
    val focus = rememberLanding(core, keys) { state.scrollToItem(it) }
    WearList(state) {
        item { Heading(placeTitle(Place.Blocks)) }
        screenActionItems(app, actions)
        if (list != null && list.items.isEmpty()) item { Text(list.empty) }
        list?.items.orEmpty().forEachIndexed { index, shown ->
            item {
                val row = shown.row
                val line = lines?.list?.getOrNull(index)
                val actions = app.rowActions(row.id, list?.offeredOf(shown).orEmpty())
                // Said as a task row is: when, in this watch's clock, then how long.
                RowButton(row.title, detail = line?.detail?.ifEmpty { null }, actions = actions, onLongClick = { navigator.actions(row.title, actions) }, key = row.id, focus = focus, swatch = row.swatch) {
                    // Its own screen is its form, for every occurrence: its Edit, on the row in hand.
                    app.rowKeys(row.id).key("open")
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
        if (screen.choices.isEmpty()) item { Text(appWords.nothingToChoose) }
        screen.choices.forEachIndexed { index, choice ->
            item {
                // In a tree, the level where it changes beside the detail, as a tree's rows say it.
                // Why it is offered ("Suggested because …") beneath, as its description.
                val detail = listOf(detailOf(choice), choice.reason).filter { it.isNotEmpty() }.joinToString("\n")
                Button(
                    onClick = { screen.chosen(choice) },
                    modifier = Modifier.fillMaxWidth(),
                    icon = choice.swatch?.let { { SwatchDot(it) } },
                    label = { Text(choice.title) },
                    secondaryLabel = detail.takeIf { it.isNotEmpty() }?.let { { Text(it, maxLines = Int.MAX_VALUE) } },
                )
            }
        }
    }
}
