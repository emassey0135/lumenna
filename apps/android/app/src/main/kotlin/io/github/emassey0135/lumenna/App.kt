package io.github.emassey0135.lumenna

import androidx.activity.compose.BackHandler
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

/** Going somewhere, and coming back. */
class Navigator(private val stack: SnapshotStateList<Screen>) {
    val current: Screen get() = stack.last()
    val canGoBack: Boolean get() = stack.size > 1

    fun push(screen: Screen) {
        stack.add(screen)
    }

    fun back() {
        if (canGoBack) stack.removeAt(stack.lastIndex)
    }
}

@Composable
fun LumennaApp(core: Core, newTask: StateFlow<Long>? = null) {
    var tab by rememberSaveable { mutableStateOf(Tab.TODAY) }
    val stacks = remember { Tab.entries.associateWith { mutableStateListOf(it.root) } }
    val navigator = remember(tab) { Navigator(stacks.getValue(tab)) }
    val snackbar = remember { SnackbarHostState() }

    // The core's sentence for each change, said politely: a snackbar is a live region, and
    // Material gives it the time the person's accessibility settings ask for.
    LaunchedEffect(core) {
        core.announcements.collect { snackbar.showSnackbar(it) }
    }
    BackHandler(enabled = navigator.canGoBack) { navigator.back() }

    // New Task from outside the app: the Tasks tab, with quick add over its list.
    val requested = newTask?.collectAsState()?.value ?: 0L
    LaunchedEffect(requested) {
        if (requested > 0) {
            tab = Tab.TASKS
            stacks.getValue(Tab.TASKS).apply {
                retainAll(listOf(Tab.TASKS.root))
                add(Screen.QuickAdd())
            }
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        bottomBar = {
            NavigationBar {
                Tab.entries.forEach { each ->
                    NavigationBarItem(
                        selected = each == tab,
                        onClick = { if (each == tab) stacks.getValue(each).retainAll(listOf(each.root)) else tab = each },
                        icon = { Icon(each.icon, contentDescription = null) },
                        label = { Text(each.title) },
                    )
                }
            }
        },
    ) { padding ->
        Box(Modifier.padding(padding).fillMaxSize()) {
            val changes by core.changes.collectAsState()
            Screens(core, navigator, navigator.current, changes)
        }
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
