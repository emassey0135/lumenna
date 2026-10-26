package dev.emassey0135.lumenna.wear

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.hideFromAccessibility
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.wear.compose.material3.AlertDialog
import androidx.wear.compose.material3.AlertDialogDefaults
import androidx.wear.compose.material3.AppScaffold
import androidx.wear.compose.material3.Icon
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.wear.compose.material3.Text
import androidx.wear.compose.navigation.SwipeDismissableNavHost
import androidx.wear.compose.navigation.composable
import androidx.wear.compose.navigation.rememberSwipeDismissableNavController
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.Launched
import dev.emassey0135.lumenna.Reminded
import dev.emassey0135.lumenna.Said
import dev.emassey0135.lumenna.launch
import dev.emassey0135.lumenna.replan
import dev.emassey0135.lumenna.sayInTurn
import dev.emassey0135.lumenna.app.Place
import kotlinx.coroutines.delay

/** What can be shown: each screen of the app, and each list of choices it offers. */
sealed interface Screen {
    data object Places : Screen
    data class Tasks(val place: Place) : Screen
    /** The day planner: a reminder's block on its day, in hand, else the day the app shows. */
    data class Day(val block: dev.emassey0135.lumenna.BlockOn? = null) : Screen
    data object Blocks : Screen
    data class Task(val id: String) : Screen
    /** Quick add: the app's line. */
    data object AddTask : Screen
    /** The app's block form, while it has one open. */
    data object BlockForm : Screen
    data object Settings : Screen
    /** One of the core's pages of settings, by its id: Planning, Backups, General. */
    data class SettingsPage(val id: String) : Screen
    data object Devices : Screen
    /** A task's or a block series' reminders: the app's list, while it has one open. */
    data object Reminders : Screen
    /** Reminders on this watch. */
    data object ReminderSettings : Screen
    /** Replan from Now or Plan the Day: the app's proposal, to accept in part, while it has one open. */
    data object Proposal : Screen
    /**
     * A list to choose one from: an action, a project, a task, a length. Left without a choice,
     * [cancelled] says so.
     */
    data class Choose(
        val title: String,
        val message: String?,
        val choices: List<Option>,
        val chosen: (Option) -> Unit,
        val cancelled: () -> Unit = {},
    ) : Screen {
        var answered = false
    }
}

/**
 * Opens screens over the one before, as swiping right closes each: Wear's own navigation, by
 * route, with each route standing for the screen opened under it.
 */
class Navigator(private val controller: NavHostController) {
    private val opened = mutableMapOf<String, Screen>()
    private var next = 0

    fun open(screen: Screen) {
        val route = "screen/${next++}"
        opened[route] = screen
        controller.navigate(route)
    }

    /** Back to the screen before. */
    fun back() {
        controller.popBackStack()
    }

    /** This screen in place of the one shown. */
    fun replace(screen: Screen) {
        back()
        open(screen)
    }

    /** The screen shown now. */
    fun current(): Screen = screen(controller.currentBackStackEntry?.arguments?.getString("n")?.let { "screen/$it" })

    /** A screen gone from the back stack for good, by back or a swipe: nothing will show it again. */
    fun forget(route: String) {
        opened.remove(route)
    }

    /** Offers `choices`; `chosen` runs once the list has closed, `cancelled` if it closed without one. */
    fun choose(title: String, choices: List<Option>, message: String? = null, cancelled: () -> Unit = {}, chosen: (Option) -> Unit) {
        lateinit var screen: Screen.Choose
        screen = Screen.Choose(title, message, choices, { choice ->
            screen.answered = true
            back()
            chosen(choice)
        }, cancelled)
        open(screen)
    }

    /** Offers a row's actions as a list, for a long press. */
    fun actions(title: String, actions: List<dev.emassey0135.lumenna.RowAction>) {
        choose(title, actions.mapIndexed { index, action -> Option(index.toString(), action.name) }) { chosen ->
            actions[chosen.key.toInt()].run()
        }
    }

    fun screen(route: String?): Screen = route?.let { opened[it] } ?: Screen.Places

    /** The confirmation being asked, shown as Wear's own AlertDialog over the screen. */
    var confirming by mutableStateOf<Confirming?>(null)
        private set

    /** Asks [title] before something that cannot be taken back; [yes] runs once confirmed, [dismissed] if not. */
    fun confirm(title: String, message: String, yes: String, dismissed: () -> Unit = {}, confirmed: () -> Unit) {
        confirming = Confirming(title, message, yes, confirmed, dismissed)
    }

    fun answered() {
        confirming = null
    }

}

/** A question before something that cannot be taken back: the core's title, message and button. */
data class Confirming(val title: String, val message: String, val yes: String, val confirmed: () -> Unit, val dismissed: () -> Unit = {})

/**
 * Wear OS's own confirmation: the question, the message, then a confirm and a dismiss button.
 * Each button is named in words, the confirm one by the core's ("Delete label"): the
 * platform's icons alone say only "Confirm" and "Dismiss".
 */
@Composable
private fun Confirmation(navigator: Navigator) {
    val asked = navigator.confirming
    AlertDialog(
        visible = asked != null,
        onDismissRequest = {
            navigator.answered()
            asked?.dismissed?.invoke()
        },
        title = { Text(asked?.title.orEmpty()) },
        text = asked?.message?.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
        confirmButton = {
            AlertDialogDefaults.ConfirmButton(onClick = {
                navigator.answered()
                asked?.confirmed?.invoke()
            }) { Icon(glyph(CHECK), contentDescription = asked?.yes) }
        },
        dismissButton = {
            AlertDialogDefaults.DismissButton(onClick = {
                navigator.answered()
                asked?.dismissed?.invoke()
            }) {
                Icon(glyph(CLOSE), contentDescription = dev.emassey0135.lumenna.quickWords.cancel)
            }
        },
    )
}

private const val CHECK = "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z"
private const val CLOSE = "M19,6.41L17.59,5 12,10.59 6.41,5 5,6.41 10.59,12 5,17.59 6.41,19 12,13.41 17.59,19 19,17.59 13.41,12z"

/** One of Material's icon paths, drawn in the button's own colour. */
private fun glyph(path: String) = ImageVector.Builder(
    defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f,
).addPath(pathData = addPathNodes(path), fill = SolidColor(Color.Black)).build()

@Composable
fun WearApp(
    core: Core,
    entry: TextEntry? = null,
    opened: kotlinx.coroutines.flow.StateFlow<Pair<Long, dev.emassey0135.lumenna.ReminderOpened>?>? = null,
    launched: kotlinx.coroutines.flow.StateFlow<Pair<Long, String>?>? = null,
) {
    val controller = rememberSwipeDismissableNavController()
    val navigator = remember(controller) { Navigator(controller) }
    // A reminder's notification, tapped: its task, or its block on its day, in hand, as the
    // phone opens it.
    val tapped = opened?.collectAsState()?.value
    LaunchedEffect(tapped) {
        when (val reminded = tapped?.second?.reminded) {
            is Reminded.Task -> navigator.open(Screen.Task(reminded.id))
            is Reminded.Block -> navigator.open(Screen.Day(reminded.on))
            is Reminded.Replan -> core.app.replan(reminded)
            null -> {}
        }
    }
    // A way in from outside the app (the tile, a complication): the app does what it says, and
    // the screens follow it there once it is still, with quick add when the app opened its line.
    val launch = launched?.collectAsState()?.value
    LaunchedEffect(launch) {
        val id = launch?.second ?: return@LaunchedEffect
        when (val went = core.app.launch(id)) {
            Launched.QuickAdd -> navigator.open(Screen.AddTask)
            is Launched.At -> went.place?.let { navigator.open(placeScreen(it)) }
            null -> {}
        }
    }
    val changes by core.changes.collectAsState()
    // The moment proposals are made against: a test's, or now.
    val plannerNow = dev.emassey0135.lumenna.LocalPlannerNow.current
    LaunchedEffect(plannerNow) { core.app.send(dev.emassey0135.lumenna.app.ShellEvent.PlanFrom(plannerNow)) }
    // What changes say, one at a time and in turn, each held long enough to be heard: set as
    // they came, each overwrote the one before before TalkBack had said it.
    var said by remember { mutableStateOf<Said?>(null) }
    LaunchedEffect(core) { sayInTurn(core.announcements) { said = it } }
    LaunchedEffect(said) {
        if (said != null) {
            delay(Said.CLEARED)
            said = null
        }
    }
    // AppScaffold shows the time at the top of every screen, as Wear OS asks of an app; each
    // screen's ScreenScaffold (`WearList`) moves it out of the way as its list scrolls.
    CompositionLocalProvider(LocalTextEntry provides (entry ?: rememberSystemTextEntry())) {
        AppScaffold {
            Box(Modifier.fillMaxSize()) {
                SwipeDismissableNavHost(navController = controller, startDestination = "places") {
                    composable("places") { PlacesScreen(core, navigator) }
                    composable("screen/{n}") { backStack ->
                        val route = "screen/${backStack.arguments?.getString("n")}"
                        // Popped by back or a swipe, a screen is forgotten: the routes otherwise
                        // kept every screen ever opened.
                        remember(backStack) {
                            backStack.lifecycle.addObserver(
                                androidx.lifecycle.LifecycleEventObserver { _, event ->
                                    if (event == androidx.lifecycle.Lifecycle.Event.ON_DESTROY) navigator.forget(route)
                                },
                            )
                        }
                        when (val screen = navigator.screen(route)) {
                            Screen.Places -> PlacesScreen(core, navigator)
                            is Screen.Tasks -> TasksScreen(core, navigator, screen.place)
                            is Screen.Day -> DayScreen(core, navigator, screen.block)
                            Screen.Blocks -> BlocksScreen(core, navigator)
                            is Screen.Task -> TaskScreen(core, navigator, screen)
                            Screen.AddTask -> AddTaskScreen(core, navigator)
                            Screen.BlockForm -> BlockFormScreen(core, navigator)
                            Screen.Settings -> SettingsScreen(core, navigator)
                            is Screen.SettingsPage -> SettingsPageScreen(core, screen.id)
                            Screen.Devices -> DevicesScreen(core, navigator)
                            Screen.Reminders -> RemindersScreen(core, navigator)
                            Screen.ReminderSettings -> ReminderSettingsScreen(core, changes)
                            is Screen.Choose -> ChooseScreen(screen)
                            Screen.Proposal -> ProposalScreen(core, navigator)
                        }
                    }
                }
                Confirmation(navigator)
                // The question the app has open, and the screens it opens, followed here.
                WearQuestions(core, navigator, LocalTextEntry.current)
                Follow(core, navigator)
            // What changes say, read by TalkBack as a polite live region. Not shown: on a
                // watch's screen it covered the rows beneath it, and the accessibility check found
                // a button only a third visible.
                Box(
                    Modifier.align(Alignment.Center).size(1.dp).semantics {
                        liveRegion = LiveRegionMode.Polite
                        // Out of the tree while there is nothing to say: an empty one is unlabelled.
                        said.let { if (it == null) hideFromAccessibility() else contentDescription = it.description }
                    },
                )
            }
        }
    }
}

/**
 * The screens the app opens itself — its block form, a proposal, a reminder list — opened over
 * whatever is shown while the app has one open; each goes back when the app closes it.
 */
@Composable
private fun Follow(core: Core, navigator: Navigator) {
    val view by core.app.view.collectAsState()
    val form = view?.blockForm != null
    val proposal = view?.proposal != null
    val reminders = view?.reminders != null
    LaunchedEffect(form) { if (form) navigator.open(Screen.BlockForm) }
    LaunchedEffect(proposal) { if (proposal) navigator.open(Screen.Proposal) }
    LaunchedEffect(reminders) { if (reminders) navigator.open(Screen.Reminders) }
}
