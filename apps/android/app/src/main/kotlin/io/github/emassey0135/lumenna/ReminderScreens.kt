package io.github.emassey0135.lumenna

import io.github.emassey0135.lumenna.app.Side
import io.github.emassey0135.lumenna.app.ShellEvent
import androidx.compose.material3.HorizontalDivider
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.Icons
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.getValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * A task's or a block's reminders: the app's list (`ViewModel.reminders`), open while it has
 * one: its announcement as the list's heading ("1 reminder, the defaults"), each reminder in
 * the core's words, and its actions — Remove reminder on one of its own, and the list's Add
 * reminder, a phrase the core reads, whose refusal stays in its dialog with the core's sentence.
 */
@Composable
fun RemindersScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    val list = view?.reminders
    var shown by remember { mutableStateOf(false) }
    LaunchedEffect(list == null) { if (list != null) shown = true else if (shown) navigator.back() }
    DisposableEffect(Unit) { onDispose { if (app.view.value?.reminders != null) app.send(ShellEvent.CloseReminders) } }
    val ask = rememberNotificationAsk()
    // Adding the first reminder is when notifications are asked for.
    val add = list?.actions.orEmpty().firstOrNull()
    ScreenFrame("Reminders", core, navigator, actions = {
        if (add != null) {
            IconButton(onClick = {
                ask()
                app.send(ShellEvent.Perform(add))
            }) { Icon(Icons.Filled.Add, contentDescription = add.name) }
        }
    }) {
        if (list == null) return@ScreenFrame
        Text(
            list.announcement.replaceFirstChar { c -> c.uppercase() },
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
        )
        val rows = list.reminders
        val state = rememberLazyListState()
        val focus = rememberSideLanding(core, Side.REMINDERS, rows.map { it.id }, state)
        val lines by app.lines.collectAsState()
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(rows.size, 1) }, state = state) {
            itemsIndexed(rows, key = { _, reminder -> reminder.id }) { index, reminder ->
                val line = lines?.reminders?.getOrNull(index)
                val detail = line?.detail.orEmpty()
                ListRow(
                    title = line?.title ?: reminder.words,
                    detail = detail,
                    speech = detail,
                    index = index,
                    actions = app.rowActions(reminder.id, reminder.actions, side = Side.REMINDERS),
                    open = null,
                    openLabel = "Open",
                    focus = focus.requester(reminder.id),
                    key = reminder.id,
                    say = core::say,
                )
                HorizontalDivider()
            }
        }
    }
}

/** The app's reminder list: shown over [navigator]'s screen while the app has one open. */
@Composable
fun FollowReminders(core: Core, navigator: Navigator) {
    val view by core.app.view.collectAsState()
    val open = view?.reminders != null
    LaunchedEffect(open) { if (open && navigator.current != Screen.Reminders) navigator.push(Screen.Reminders) }
}

/**
 * Asks for notifications, from Android 13, the first time it is called: when the person first
 * adds a reminder or turns reminders on. Afterwards the Reminders settings page says they are
 * off and opens the system's page.
 */
@Composable
fun rememberNotificationAsk(): () -> Unit {
    val context = LocalContext.current
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    return remember(launcher) {
        {
            if (Reminders.shouldAsk(context)) {
                Reminders.asked(context)
                launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }
}
