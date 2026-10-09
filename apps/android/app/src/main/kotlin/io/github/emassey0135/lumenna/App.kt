package io.github.emassey0135.lumenna

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.LocalTextStyle
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.width
import io.github.emassey0135.lumenna.core.Place
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.onKeyEvent
import io.github.emassey0135.lumenna.core.LumennaException
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.List
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.flow.StateFlow

/** Somewhere to be: what a tab's back stack holds. */
sealed interface Screen {
    /** The day planner, on a date; today when null. */
    data class Day(val date: String? = null) : Screen

    /** Tasks matching a query, under a title; `prefix` starts a task added here. */
    data class Tasks(
        val title: String = "Tasks",
        val query: String = "",
        val prefix: String = "",
        val trash: Boolean = false,
    ) : Screen

    /** One task's details. */
    data class Task(val id: String) : Screen

    /** Adding a task in a line, starting from `prefix`. */
    data class QuickAdd(val prefix: String = "") : Screen

    /** A block, added or changed. */
    data class BlockForm(val purpose: BlockPurpose) : Screen

    /** Projects, labels, saved filters, blocks and the trash. */
    data object Browse : Screen

    /** The project tree. */
    data object Projects : Screen

    /** Every label. */
    data object Labels : Screen

    /** The saved filters. */
    data object Filters : Screen

    /** Every block series. */
    data object Blocks : Screen

    /** The settings pages. */
    data object Settings : Screen

    /** The paired devices and syncing. */
    data object Devices : Screen

    /** Pairing another device. */
    data object Pairing : Screen

    /** The day, the week, subtasks, announcements: what syncs. */
    data object Planning : Screen

    /** This device's backups. */
    data object Backups : Screen

    /** Export and import. */
    data object Export : Screen
}

/** The four tabs, as on the iPhone: each its own back stack. */
enum class Tab(val title: String, val icon: ImageVector, val root: Screen) {
    TODAY("Today", Icons.Filled.DateRange, Screen.Day()),
    TASKS("Tasks", Icons.Filled.CheckCircle, Screen.Tasks()),
    BROWSE("Browse", Icons.AutoMirrored.Filled.List, Screen.Browse),
    SETTINGS("Settings", Icons.Filled.Settings, Screen.Settings),
}

/**
 * Going somewhere, and coming back, from one place in a tab's stack: [depth], or the top when
 * null. Side by side, the parent's pane and the child's each have one, so a row chosen in
 * the parent replaces the child beside it rather than stacking a third screen.
 */
class Navigator(private val stack: SnapshotStateList<Screen>, private val depth: Int? = null) {
    private val at: Int get() = depth ?: stack.lastIndex
    val current: Screen get() = stack[at]
    val canGoBack: Boolean get() = at > 0

    fun push(screen: Screen) {
        while (stack.size > at + 1) stack.removeAt(stack.lastIndex)
        stack.add(screen)
    }

    /** Leaves this screen, and whatever was opened from it. */
    fun back() {
        if (canGoBack) while (stack.size > at) stack.removeAt(stack.lastIndex)
    }
}

/** From this width the tabs are a rail at the side, as Material lays out a medium window. */
private val RailWidth = 600.dp

/**
 * From this width the places are a sidebar, and a screen opened from another shows beside
 * it: Material's expanded window.
 */
private val SplitWidth = 840.dp

/** The sidebar's width, as Material's navigation drawer's. */
private val SidebarWidth = 300.dp

@Composable
fun LumennaApp(core: Core, newTask: StateFlow<Long>? = null, shortcuts: Shortcuts = remember { Shortcuts() }) {
    var tab by rememberSaveable { mutableStateOf(Tab.TODAY) }
    val stacks = remember { Tab.entries.associateWith { mutableStateListOf(it.root) } }
    // A wide window's: the place the sidebar chose, and what was opened from it.
    var destination by remember { mutableStateOf<Destination>(Destination.At(Place.Today)) }
    val placeStack = remember { mutableStateListOf<Screen>(Screen.Day()) }
    val snackbar = remember { SnackbarHostState() }
    // Each screen keeps what it holds — its scroll, a half-typed filter — while another is
    // over it, or as it moves between panes.
    val saved = rememberSaveableStateHolder()

    // The core's sentence for each change, said politely: a snackbar is a live region, and
    // Material gives it the time the person's accessibility settings ask for.
    LaunchedEffect(core) {
        core.announcements.collect { snackbar.showSnackbar(it) }
    }

    CompositionLocalProvider(LocalShortcuts provides shortcuts) {
        BoxWithConstraints(Modifier.fillMaxSize().onKeyEvent { shortcuts.handle(it.nativeKeyEvent) }) {
            // Wide, the places are a sidebar, as on the desktop apps and the iPad; narrower,
            // the tabs are a rail, then a bar along the bottom.
            val wide = maxWidth >= SplitWidth
            val rail = !wide && maxWidth >= RailWidth
            val stack = if (wide) placeStack else stacks.getValue(tab)

            /** The sidebar's [place], with [screens] opened from it. */
            fun show(place: Destination, vararg screens: Screen) {
                destination = place
                placeStack.clear()
                placeStack.add(place.screen())
                placeStack.addAll(screens)
            }

            /** A tab at its root, with [screens] opened on it; wide, the sidebar's place for it. */
            fun go(to: Tab, vararg screens: Screen) {
                if (wide) {
                    val first = screens.firstOrNull()
                    when {
                        to == Tab.TODAY -> show(Destination.At(Place.Today), *screens)
                        to == Tab.SETTINGS -> show(Destination.Settings, *screens)
                        first == Screen.Blocks -> show(Destination.At(Place.Blocks))
                        first is Screen.Tasks && first.trash -> show(Destination.At(Place.Trash))
                        else -> show(Destination.At(Place.Tasks), *screens)
                    }
                    return
                }
                tab = to
                stacks.getValue(to).apply {
                    while (size > 1) removeAt(lastIndex)
                    addAll(screens)
                }
            }

            BackHandler(enabled = stack.size > 1) { Navigator(stack).back() }

            // New Task from outside the app: the Tasks tab, with quick add over its list.
            val requested = newTask?.collectAsState()?.value ?: 0L
            LaunchedEffect(requested) {
                if (requested > 0) go(Tab.TASKS, Screen.QuickAdd())
            }

            // The commands any screen answers; a screen offering one of its own answers instead.
            Offer(Command.NEW_TASK) { go(Tab.TASKS, Screen.QuickAdd()) }
            Offer(Command.NEW_BLOCK) { go(Tab.TODAY, Screen.BlockForm(BlockPurpose.Add(date = null))) }
            Offer(Command.FILTER) {
                go(Tab.TASKS)
                shortcuts.filterAsked.value = true
            }
            Offer(Command.UNDO) { core.change { it.undo() } }
            Offer(Command.REDO) { core.change { it.redo() } }
            Offer(Command.GO_TODAY) { go(Tab.TODAY) }
            Offer(Command.GO_TASKS) { go(Tab.TASKS) }
            Offer(Command.GO_BLOCKS) { go(Tab.BROWSE, Screen.Blocks) }
            Offer(Command.GO_TRASH) { go(Tab.BROWSE, Screen.Tasks(title = "Trash", query = "deleted", trash = true)) }
            Offer(Command.SETTINGS) { go(Tab.SETTINGS) }
            Offer(Command.SYNC_NOW) { syncNow(core) }

            val split = wide && stack.size > 1
            // The tabs or the sidebar, then each pane shown: what F6 moves between, as on
            // Windows and GTK.
            val regions = remember { List(3) { FocusRequester() } }
            var inRegion by remember { mutableIntStateOf(-1) }
            val shownRegions = if (split) 3 else 2
            // Coming back to a pane lands on the row it was left on, not its first control.
            // Compose's own saveFocusedChild remembers only a pane's immediate child, which
            // gives focus to that child's first control.
            val panes = remember { List(3) { Pane() } }
            fun moveTo(index: Int) {
                val back = panes[index].row?.let { runCatching { it.requestFocus() }.isSuccess } == true
                if (!back) regions[index].tryFocus()
            }
            Offer(Command.NEXT_PANE) { moveTo((inRegion + 1).mod(shownRegions)) }
            Offer(Command.PREVIOUS_PANE) { moveTo((inRegion - 1).mod(shownRegions)) }
            fun Modifier.region(index: Int) = onFocusChanged { if (it.hasFocus) inRegion = index }
                .focusRequester(regions[index])
                .focusGroup()

            val choose = { each: Tab -> if (each == tab) go(each) else tab = each }
            val changes by core.changes.collectAsState()

            Scaffold(
                snackbarHost = { SnackbarHost(snackbar) },
                bottomBar = {
                    if (!wide && !rail) {
                        NavigationBar(Modifier.region(0)) {
                            Tab.entries.forEach { each ->
                                NavigationBarItem(
                                    selected = each == tab,
                                    onClick = { choose(each) },
                                    icon = { Icon(each.icon, contentDescription = null) },
                                    label = { TabLabel(each.title) },
                                )
                            }
                        }
                    }
                },
            ) { padding ->
                Row(Modifier.padding(padding).fillMaxSize()) {
                    if (wide) {
                        Sidebar(
                            core, changes, destination, choose = { show(it) },
                            modifier = Modifier.width(SidebarWidth).fillMaxHeight().region(0),
                        )
                        VerticalDivider()
                    } else if (rail) {
                        NavigationRail(Modifier.region(0)) {
                            Tab.entries.forEach { each ->
                                NavigationRailItem(
                                    selected = each == tab,
                                    onClick = { choose(each) },
                                    icon = { Icon(each.icon, contentDescription = null) },
                                    label = { TabLabel(each.title) },
                                )
                            }
                        }
                    }
                    @Composable
                    fun Pane(depth: Int, modifier: Modifier, region: Int) {
                        val screen = stack[depth]
                        val owner = if (wide) "place" else tab.name
                        Box(modifier.fillMaxHeight().region(region)) {
                            CompositionLocalProvider(LocalPane provides panes[region]) {
                                saved.SaveableStateProvider("$owner $depth $screen") {
                                    Screens(core, Navigator(stack, depth), screen, changes)
                                }
                            }
                        }
                    }
                    if (split) {
                        Pane(stack.lastIndex - 1, Modifier.weight(2f), 1)
                        VerticalDivider()
                        Pane(stack.lastIndex, Modifier.weight(3f), 2)
                        // A screen opened beside its parent takes focus, as it would replacing
                        // it — unless it has already put focus in itself, as quick add does.
                        LaunchedEffect(stack.size, stack.last()) {
                            withFrameNanos {}
                            withFrameNanos {}
                            if (inRegion != 2) regions[2].tryFocus()
                        }
                    } else {
                        Pane(stack.lastIndex, Modifier.weight(1f), 1)
                    }
                }
            }
        }
    }
}

/** One of the window's panes: the row focus was last on in it, for F6 to come back to. */
class Pane {
    var row: FocusRequester? = null
}

/** The pane a screen is shown in. */
val LocalPane = staticCompositionLocalOf<Pane?> { null }

/**
 * A tab's name on one line, shrunk only as far as it must be to fit. At the largest font in
 * a small phone a four-tab bar has no room for "Settings" at full size, and wrapped, it broke
 * mid-word ("Setting", "s"). TalkBack reads the whole name either way.
 */
@Composable
private fun TabLabel(title: String) {
    val style = LocalTextStyle.current
    val colour = LocalContentColor.current
    BasicText(
        title,
        style = style.copy(color = colour),
        maxLines = 1,
        softWrap = false,
        autoSize = TextAutoSize.StepBased(minFontSize = 8.sp, maxFontSize = style.fontSize),
    )
}

/** Focus into what [this] is attached to, if it is shown. */
private fun FocusRequester.tryFocus() {
    runCatching { requestFocus() }
}

/** A round with every paired device now, saying how it went. */
fun syncNow(core: Core) {
    core.say("Syncing")
    core.syncNow { result ->
        result.fold(
            { core.changed(); core.say(sentence(it.announcement, it.notices)) },
            { core.say((it as? LumennaException)?.sentence ?: it.message.orEmpty()) },
        )
    }
}

@Composable
private fun Screens(core: Core, navigator: Navigator, screen: Screen, changes: Long) {
    when (screen) {
        is Screen.Tasks -> TaskListScreen(core, navigator, screen, changes)
        is Screen.QuickAdd -> QuickAddScreen(core, navigator, screen)
        is Screen.Task -> TaskDetailScreen(core, navigator, screen, changes)
        is Screen.Day -> DayScreen(core, navigator, screen, changes)
        is Screen.BlockForm -> BlockFormScreen(core, navigator, screen.purpose)
        Screen.Browse -> BrowseScreen(core, navigator, changes)
        Screen.Projects -> ProjectsScreen(core, navigator, changes)
        Screen.Labels -> LabelsScreen(core, navigator, changes)
        Screen.Filters -> FiltersScreen(core, navigator, changes)
        Screen.Blocks -> BlocksScreen(core, navigator, changes)
        Screen.Settings -> SettingsScreen(core, navigator, changes)
        Screen.Devices -> DevicesScreen(core, navigator, changes)
        Screen.Pairing -> PairingScreen(core, navigator)
        Screen.Planning -> PlanningScreen(core, navigator, changes)
        Screen.Backups -> BackupsScreen(core, navigator, changes)
        Screen.Export -> ExportScreen(core, navigator)
    }
}

/**
 * A screen's frame: its title as a heading and the pane's name, a way back when there is one,
 * Undo and Redo, and the screen's own actions — all in the top bar, as on the iPhone.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ScreenFrame(
    title: String,
    core: Core,
    navigator: Navigator,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    Scaffold(
        // TalkBack says a new pane's title when it appears, as VoiceOver says a new screen's.
        modifier = Modifier.semantics { paneTitle = title },
        topBar = {
            TopAppBar(
                title = { Text(title, Modifier.semantics { heading() }) },
                navigationIcon = {
                    if (navigator.canGoBack) {
                        IconButton(onClick = navigator::back) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                        }
                    }
                },
                actions = {
                    IconButton(onClick = { core.change { it.undo() } }) {
                        Icon(Glyphs.undo, contentDescription = "Undo")
                    }
                    IconButton(onClick = { core.change { it.redo() } }) {
                        Icon(Glyphs.redo, contentDescription = "Redo")
                    }
                    actions()
                },
            )
        },
    ) { padding ->
        androidx.compose.foundation.layout.Column(
            Modifier.padding(padding).fillMaxSize(),
            content = content,
        )
    }
}

/** Icons the core Material set lacks, drawn from Material's own paths. */
object Glyphs {
    private fun glyph(name: String, data: String) = ImageVector.Builder(
        name = name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
    ).addPath(pathData = addPathNodes(data), fill = SolidColor(androidx.compose.ui.graphics.Color.Black)).build()

    val undo = glyph(
        "Undo",
        "M12.5,8c-2.65,0 -5.05,0.99 -6.9,2.6L2,7v9h9l-3.62,-3.62c1.39,-1.16 3.16,-1.88 5.12,-1.88 " +
            "3.54,0 6.55,2.31 7.6,5.5l2.37,-0.78C21.08,11.03 17.15,8 12.5,8z",
    )
    val circle = glyph(
        "Not done",
        "M12,2C6.48,2 2,6.48 2,12s4.48,10 10,10 10,-4.48 10,-10S17.52,2 12,2zM12,20c-4.42,0 -8,-3.58 " +
            "-8,-8s3.58,-8 8,-8 8,3.58 8,8 -3.58,8 -8,8z",
    )
    val redo = glyph(
        "Redo",
        "M18.4,10.6C16.55,8.99 14.15,8 11.5,8c-4.65,0 -8.58,3.03 -9.96,7.22L3.9,16c1.05,-3.19 " +
            "4.05,-5.5 7.6,-5.5 1.95,0 3.73,0.72 5.12,1.88L13,16h9V7l-3.6,3.6z",
    )
}

/**
 * A button's least height: 48dp, Android's minimum touch target. Material's text and outlined
 * buttons are 40dp, which the accessibility checks rightly flag.
 */
val Target: Modifier = Modifier.heightIn(min = 48.dp)

/** Quiet text, still above contrast: secondary lines and readbacks. */
@Composable
fun quiet() = MaterialTheme.colorScheme.onSurfaceVariant
