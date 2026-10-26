package dev.emassey0135.lumenna.wear

import dev.emassey0135.lumenna.app.foldState
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.rememberLanding
import androidx.wear.compose.foundation.lazy.rememberScalingLazyListState
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.actingOn
import dev.emassey0135.lumenna.rowActions
import dev.emassey0135.lumenna.LocalPlannerNow
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.DayMove
import dev.emassey0135.lumenna.app.DayKind
import dev.emassey0135.lumenna.app.BlockEvent
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
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.RowAction
import dev.emassey0135.lumenna.levelChange
import dev.emassey0135.lumenna.commandTitle
import java.time.LocalDate

/**
 * The day as lived, the app's (`ViewModel.day`), as the phone's day lists it: its summary, each
 * block with its sittings beneath it, free time, now, and what is cancelled for the day. A
 * row's actions are the app's: TalkBack's custom actions, and a long press or a tap. A block
 * with sittings folds. Another day is a button away, Go to Day the app's question.
 */
@Composable
fun DayScreen(core: Core, navigator: Navigator, block: dev.emassey0135.lumenna.BlockOn? = null) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) {
        // A reminder's block: its day, with it in hand, as the phone shows it.
        if (block != null) app.send(ShellEvent.ShowBlock(block.series, block.date))
        // Unless the app shows it already: coming back from a screen above it keeps the day shown.
        else if (app.view.value?.place !is Place.Today) app.send(ShellEvent.Open(Place.Today))
    }
    val day = view?.takeIf { it.place == Place.Today }?.day
    val worded by app.lines.collectAsState()
    val lines = worded?.takeIf { day != null }
    val rows = day?.items.orEmpty().mapIndexed { index, item -> item to lines?.day?.getOrNull(index) }
    val heading = rows.firstOrNull { it.first.kind == DayKind.SUMMARY }?.second?.title
    val shown = rows.filter { !it.first.hidden && it.first.kind != DayKind.SUMMARY }
    // The day's own actions, the app's: on its heading, and as buttons after its rows.
    val dayActions = day?.let { app.actingOn(ActingOn.Day, it.offered) }.orEmpty()
    val move = { step: DayMove -> app.send(ShellEvent.Day(step)) }
    val keys = listOf("\u0000heading") + shown.map { it.first.key }
    val state = rememberScalingLazyListState()
    // TalkBack goes to the row the app moved — now, when today opens — as on the phone.
    val focus = rememberLanding(core, keys) { state.scrollToItem(it) }
    WearList(state) {
        // The day's heading, as every app says it: "<day>. <summary>".
        item {
            Heading(
                heading ?: "Day",
                dayActions,
            )
        }
        shown.forEach { (item, line) ->
            item {
                val title = line?.title.orEmpty()
                val actions = app.rowActions(item.key, item.offered).sortedByDescending { it.primary }
                RowButton(
                    title,
                    detail = line?.detail.orEmpty(),
                    state = listOfNotNull(item.expanded?.let(::foldState), item.level).joinToString(", ").ifEmpty { null },
                    actions = actions,
                    onLongClick = { navigator.actions(title, actions) },
                    key = item.key,
                    focus = focus,
                    swatch = item.block?.takeIf { item.kind == DayKind.BLOCK }?.swatch,
                    onClick = { if (actions.isNotEmpty()) navigator.actions(title, actions) },
                )
            }
        }
        dayActions.forEach { action ->
            item { Button(onClick = action.run, modifier = Modifier.fillMaxWidth(), label = { Text(action.name) }) }
        }
        item { Button(onClick = { app.send(ShellEvent.Block(BlockEvent.New(null, null))) }, modifier = Modifier.fillMaxWidth(), label = { Text("Add block") }) }
        // Named as every app's commands for each move are.
        item { Button(onClick = { move(DayMove.PREVIOUS) }, modifier = Modifier.fillMaxWidth(), label = { Text(commandTitle("previous-day")) }) }
        if (day?.today == false) item { Button(onClick = { move(DayMove.TODAY) }, modifier = Modifier.fillMaxWidth(), label = { Text(commandTitle("go-today")) }) }
        item { Button(onClick = { move(DayMove.NEXT) }, modifier = Modifier.fillMaxWidth(), label = { Text(commandTitle("next-day")) }) }
        // As the phone asks: a day as it is said, read by the app.
        item { Button(onClick = { move(DayMove.ASK) }, modifier = Modifier.fillMaxWidth(), label = { Text(commandTitle("go-to-day")) }) }
    }
}
