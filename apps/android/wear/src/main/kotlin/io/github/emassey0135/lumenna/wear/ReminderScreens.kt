package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.rowActions
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.SettingsEvent
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.collectAsState
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

/**
 * A task's or a block's reminders: the app's list, as the phone shows it: the core's
 * announcement as the heading, each reminder in its words, Remove reminder on one of its own
 * (a tap or a long press), and Add reminder at the foot, a phrase from the system's input
 * screen, asked as the app's question.
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
    WearList {
        item { Heading(list?.announcement?.replaceFirstChar { it.uppercase() } ?: "Reminders") }
        list?.reminders.orEmpty().forEach { reminder ->
            item {
                val words = reminder.words.replaceFirstChar { it.uppercase() }
                if (reminder.own) {
                    val actions = app.rowActions(reminder.id, reminder.actions)
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
                    onClick = {
                        ask()
                        app.send(ShellEvent.Perform(action))
                    },
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
    val shown = appSettings(core).filter { it.setting.key == "reminders" }
    val access = io.github.emassey0135.lumenna.LocalReminderAccess.current
    val (allowed, exact, grant) = remember(changes) { access(context) }
    val ask = rememberNotificationAsk()
    WearList {
        item { Heading("Reminders") }
        shown.forEach { view ->
            val setting = view.setting
            item {
                SwitchButton(
                    checked = setting.value == "true",
                    onCheckedChange = { on ->
                        core.app.send(ShellEvent.Settings(SettingsEvent.Set(setting.key, if (on) "true" else "false")))
                        if (on) ask()
                    },
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
