package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.name
import io.github.emassey0135.lumenna.rowActions
import io.github.emassey0135.lumenna.LocalPlannerNow
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.Place
import io.github.emassey0135.lumenna.app.DayMove
import io.github.emassey0135.lumenna.app.DayKind
import io.github.emassey0135.lumenna.app.BlockEvent
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
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.levelChange
import io.github.emassey0135.lumenna.words
import io.github.emassey0135.lumenna.said
import java.time.LocalDate

/**
 * The day as lived, the app's (`ViewModel.day`), as the phone's day lists it: its summary, each
 * block with its sittings beneath it, free time, now, and what is cancelled for the day. A
 * row's actions are the app's: TalkBack's custom actions, and a long press or a tap. A block
 * with sittings folds. Another day is a button away, Go to Day the app's question.
 */
@Composable
fun DayScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Open(Place.Today)) }
    val day = view?.takeIf { it.place == Place.Today }?.day
    val items = day?.items.orEmpty().filter { !it.hidden && it.kind != DayKind.SUMMARY }
    // The day's own actions, the app's: on its heading, and as buttons after its rows.
    val dayActions = day?.let { app.rowActions("summary", it.actions) }.orEmpty()
    val move = { step: DayMove -> app.send(ShellEvent.Day(step)) }
    WearList {
        // The day's heading, as every app says it: "<day>. <summary>".
        item {
            Heading(
                day?.let { d -> listOf(Clock.spokenDay(d.date), d.summary).filter { it.isNotEmpty() }.joinToString(". ") } ?: "Day",
                dayActions,
            )
        }
        items.forEachIndexed { index, item ->
            item {
                val (title, details) = words(item)
                val actions = app.rowActions(item.key, item.actions).sortedByDescending { it.primary }
                RowButton(
                    title,
                    detail = details.joinToString(", "),
                    state = listOfNotNull(item.expanded?.let { if (it) "expanded" else "collapsed" }, item.level).joinToString(", ").ifEmpty { null },
                    actions = actions,
                    onLongClick = { navigator.actions(title, actions) },
                    onClick = { if (actions.isNotEmpty()) navigator.actions(title, actions) },
                )
            }
        }
        dayActions.forEach { action ->
            item { Button(onClick = action.run, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) }) }
        }
        item { Button(onClick = { app.send(ShellEvent.Block(BlockEvent.New(null, null))) }, modifier = Modifier.fillMaxWidth(), label = { Text("Add block") }) }
        item { Button(onClick = { move(DayMove.PREVIOUS) }, modifier = Modifier.fillMaxWidth(), label = { Text("Previous day") }) }
        if (day?.today == false) item { Button(onClick = { move(DayMove.TODAY) }, modifier = Modifier.fillMaxWidth(), label = { Text("Today") }) }
        item { Button(onClick = { move(DayMove.NEXT) }, modifier = Modifier.fillMaxWidth(), label = { Text("Next day") }) }
        // As the phone asks: a day as it is said, read by the app.
        item { Button(onClick = { move(DayMove.ASK) }, modifier = Modifier.fillMaxWidth(), label = { Text("Go to day") }) }
    }
}
