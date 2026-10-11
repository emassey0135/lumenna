package io.github.emassey0135.lumenna

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
import io.github.emassey0135.lumenna.core.ActionKind

/**
 * A task's or a block's reminders, as the core lists them (`remindersOf`): its announcement as
 * the list's heading ("1 reminder, the defaults"), each reminder in the core's words, and its
 * actions — Remove reminder on one of its own, and the list's Add reminder, a phrase the core
 * reads, whose refusal stays in its dialog with the core's sentence.
 */
@Composable
fun RemindersScreen(core: Core, navigator: Navigator, target: String, changes: Long) {
    val prompt = rememberPrompter()
    val ask = rememberNotificationAsk()
    val list = remember(changes) { core.attempt { core.lumenna.remindersOf(target) } }
    // Adding the first reminder is when notifications are asked for.
    val add = list?.actions.orEmpty().let { core.offered(it, prompt, done = { ask() }) }.firstOrNull()
    ItemListScreen(
        "Reminders", core, navigator, changes = changes,
        load = {
            Listing(
                list?.reminders.orEmpty().map { reminder ->
                    Item(
                        reminder.id,
                        reminder.words.replaceFirstChar { it.uppercase() },
                        if (reminder.own) "" else "Default",
                        actions = reminder.actions,
                    )
                },
            )
        },
        addLabel = add?.name,
        add = { add?.run?.invoke() },
        open = null,
        actions = { item -> core.offered(item.actions, prompt) },
        header = {
            list?.let {
                Text(
                    it.announcement.replaceFirstChar { c -> c.uppercase() },
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp).semantics { heading() },
                )
            }
        },
    )
    prompt.Host()
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

/** Opens a task's or block's reminders for the core's Reminders action, else [form]. */
fun Navigator.formOr(form: (io.github.emassey0135.lumenna.core.Action) -> Unit): (io.github.emassey0135.lumenna.core.Action) -> Unit = { action ->
    if (action.kind == ActionKind.REMINDERS) push(Screen.Reminders(action.target)) else form(action)
}
