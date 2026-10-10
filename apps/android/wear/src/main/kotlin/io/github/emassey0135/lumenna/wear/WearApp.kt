package io.github.emassey0135.lumenna.wear

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
import io.github.emassey0135.lumenna.BlockPurpose
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.core.Place
import kotlinx.coroutines.delay

/** What can be shown: each screen of the app, and each list of choices it offers. */
sealed interface Screen {
    data object Places : Screen
    data class Tasks(val place: Place) : Screen
    data object Day : Screen
    data object Blocks : Screen
    data class Task(val id: String) : Screen {
        /**
         * The task as its form opened and the fields being edited, kept with the screen: a
         * list chosen from (the project) is a screen of its own, and the form's composition
         * does not outlive going to it.
         */
        val shown = mutableStateOf<io.github.emassey0135.lumenna.core.TaskDetail?>(null)
        val fields = mutableStateOf<io.github.emassey0135.lumenna.core.TaskFields?>(null)
    }
    data class AddTask(val prefix: String) : Screen
    data class BlockForm(val purpose: BlockPurpose) : Screen
    data object Settings : Screen
    data object Planning : Screen
    data object Devices : Screen
    /** A list to choose one from: an action, a project, a task, a length. */
    data class Choose(val title: String, val message: String?, val choices: List<Option>, val chosen: (Option) -> Unit) : Screen
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

    /** Offers `choices`; `chosen` runs once the list has closed. */
    fun choose(title: String, choices: List<Option>, message: String? = null, chosen: (Option) -> Unit) {
        open(Screen.Choose(title, message, choices) { choice ->
            back()
            chosen(choice)
        })
    }

    /** Offers a row's actions as a list, for a long press. */
    fun actions(title: String, actions: List<io.github.emassey0135.lumenna.RowAction>) {
        choose(title, actions.mapIndexed { index, action -> Option(index.toString(), action.name) }) { chosen ->
            actions[chosen.key.toInt()].run()
        }
    }

    fun screen(route: String?): Screen = route?.let { opened[it] } ?: Screen.Places

    /** The confirmation being asked, shown as Wear's own AlertDialog over the screen. */
    var confirming by mutableStateOf<Confirming?>(null)
        private set

    /** Asks [title] before something that cannot be taken back; [yes] runs once confirmed. */
    fun confirm(title: String, message: String, yes: String, confirmed: () -> Unit) {
        confirming = Confirming(title, message, yes, confirmed)
    }

    fun answered() {
        confirming = null
    }
}

/** A question before something that cannot be taken back: the core's title, message and button. */
data class Confirming(val title: String, val message: String, val yes: String, val confirmed: () -> Unit)

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
        onDismissRequest = { navigator.answered() },
        title = { Text(asked?.title.orEmpty()) },
        text = asked?.message?.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
        confirmButton = {
            AlertDialogDefaults.ConfirmButton(onClick = {
                navigator.answered()
                asked?.confirmed?.invoke()
            }) { Icon(glyph(CHECK), contentDescription = asked?.yes) }
        },
        dismissButton = {
            AlertDialogDefaults.DismissButton(onClick = { navigator.answered() }) {
                Icon(glyph(CLOSE), contentDescription = "Cancel")
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
fun WearApp(core: Core, entry: TextEntry? = null) {
    val controller = rememberSwipeDismissableNavController()
    val navigator = remember(controller) { Navigator(controller) }
    val changes by core.changes.collectAsState()
    var said by remember { mutableStateOf("") }
    LaunchedEffect(core) {
        core.announcements.collect { text ->
            said = text
        }
    }
    LaunchedEffect(said) {
        if (said.isNotEmpty()) {
            delay(4_000)
            said = ""
        }
    }
    // AppScaffold shows the time at the top of every screen, as Wear OS asks of an app; each
    // screen's ScreenScaffold (`WearList`) moves it out of the way as its list scrolls.
    CompositionLocalProvider(LocalTextEntry provides (entry ?: rememberSystemTextEntry())) {
        AppScaffold {
            Box(Modifier.fillMaxSize()) {
                SwipeDismissableNavHost(navController = controller, startDestination = "places") {
                    composable("places") { PlacesScreen(core, navigator, changes) }
                    composable("screen/{n}") { backStack ->
                        when (val screen = navigator.screen("screen/${backStack.arguments?.getString("n")}")) {
                            Screen.Places -> PlacesScreen(core, navigator, changes)
                            is Screen.Tasks -> TasksScreen(core, navigator, screen.place, changes)
                            Screen.Day -> DayScreen(core, navigator, changes)
                            Screen.Blocks -> BlocksScreen(core, navigator, changes)
                            is Screen.Task -> TaskScreen(core, navigator, screen, changes)
                            is Screen.AddTask -> AddTaskScreen(core, navigator, screen.prefix)
                            is Screen.BlockForm -> BlockFormScreen(core, navigator, screen.purpose)
                            Screen.Settings -> SettingsScreen(navigator)
                            Screen.Planning -> PlanningScreen(core, changes)
                            Screen.Devices -> DevicesScreen(core, navigator, changes)
                            is Screen.Choose -> ChooseScreen(screen)
                        }
                    }
                }
                Confirmation(navigator)
            // What changes say, read by TalkBack as a polite live region. Not shown: on a
                // watch's screen it covered the rows beneath it, and the accessibility check found
                // a button only a third visible.
                Box(
                    Modifier.align(Alignment.Center).size(1.dp).semantics {
                        liveRegion = LiveRegionMode.Polite
                        // Out of the tree while there is nothing to say: an empty one is unlabelled.
                        if (said.isEmpty()) hideFromAccessibility() else contentDescription = said
                    },
                )
            }
        }
    }
}
