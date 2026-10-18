package io.github.emassey0135.lumenna.wear

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.SwitchButton
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.Reminders
import io.github.emassey0135.lumenna.name
import io.github.emassey0135.lumenna.on
import io.github.emassey0135.lumenna.perform
import io.github.emassey0135.lumenna.reminders
import io.github.emassey0135.lumenna.set
import io.github.emassey0135.lumenna.settings
import io.github.emassey0135.lumenna.records.Action
import io.github.emassey0135.lumenna.records.ActionKind

/**
 * A task's or a block's reminders, as the phone shows them: the core's announcement as the
 * heading, each reminder in its words, Remove reminder on one of its own (a tap or a long
 * press), and Add reminder at the foot, a phrase from the system's input screen.
 */
@Composable
fun RemindersScreen(core: Core, navigator: Navigator, target: String, changes: Long) {
    val list = remember(changes) { core.attempt { core.lumenna.remindersOf(target) } }
    val entry = LocalTextEntry.current
    val ask = rememberNotificationAsk()
    WearList {
        item { Heading(list?.announcement?.replaceFirstChar { it.uppercase() } ?: "Reminders") }
        list?.reminders.orEmpty().forEach { reminder ->
            item {
                val words = reminder.words.replaceFirstChar { it.uppercase() }
                if (reminder.own) {
                    val actions = core.offered(reminder.actions, navigator, entry)
                    RowButton(words, actions = actions, onLongClick = { navigator.actions(words, actions) }) {
                        navigator.actions(words, actions)
                    }
                } else {
                    // A default is changed in Planning, so there is nothing to do to it here.
                    Text("$words, default", Modifier.fillMaxWidth())
                }
            }
        }
        list?.actions.orEmpty().forEach { action ->
            item {
                // Adding the first reminder is when notifications are asked for.
                Button(
                    onClick = { core.perform(action, WearAsker(navigator, entry), form = {}) { ask() } },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(action.name) },
                )
            }
        }
    }
}

/**
 * Reminders on this watch: whether it gives them (the core's setting, its own), then what
 * Wear OS allows — notifications, and alarms at an exact time — with the system's page for
 * each where it has one.
 */
@Composable
fun ReminderSettingsScreen(core: Core, changes: Long) {
    val context = LocalContext.current
    val shown = remember(changes) { settings(core).reminders() }
    val access = io.github.emassey0135.lumenna.LocalReminderAccess.current
    val (allowed, exact, grant) = remember(changes) { access(context) }
    val ask = rememberNotificationAsk()
    WearList {
        item { Heading("Reminders") }
        shown.forEach { setting ->
            item {
                SwitchButton(
                    checked = setting.on,
                    onCheckedChange = { on -> if (core.set(setting, if (on) "true" else "false") != null && on) ask() },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(setting.title) },
                )
            }
            if (setting.hint.isNotEmpty()) item { Text(setting.hint) }
        }
        if (!allowed) {
            item { Text("Notifications are off for Lumenna, so no reminder can be shown on this watch.") }
            item {
                Button(
                    onClick = { if (Reminders.shouldAsk(context)) ask() else context.startActivity(Reminders.notificationSettings(context)) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text("Allow notifications") },
                )
            }
        }
        if (!exact) {
            item { Text("Reminders may come a few minutes late: Lumenna is not allowed to set alarms for an exact time.") }
            grant?.let { intent ->
                item { Button(onClick = { context.startActivity(intent) }, modifier = Modifier.fillMaxWidth(), label = { Text("Allow exact alarms") }) }
            }
        }
        item { Text("When reminders come is set in Planning, and on a task or block in its Reminders.") }
    }
}

/** Asks for notifications, from Wear OS 4, the first time: as the phone does. */
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
fun Navigator.formOr(form: (Action) -> Unit): (Action) -> Unit = { action ->
    if (action.kind == ActionKind.REMINDERS) open(Screen.Reminders(action.target)) else form(action)
}
