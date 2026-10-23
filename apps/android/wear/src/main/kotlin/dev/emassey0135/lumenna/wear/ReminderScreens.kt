package dev.emassey0135.lumenna.wear

import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.rememberSideLanding
import dev.emassey0135.lumenna.app.Side
import androidx.wear.compose.foundation.lazy.rememberScalingLazyListState
import dev.emassey0135.lumenna.ShowingReminders
import dev.emassey0135.lumenna.rememberNotificationAsk
import dev.emassey0135.lumenna.rowActions
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.SettingsEvent
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
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.name

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
    val lines by app.lines.collectAsState()
    ShowingReminders(app, navigator::back)
    val ask = rememberNotificationAsk()
    val keys = listOf("\u0000heading") + list?.reminders.orEmpty().map { it.id }
    val state = rememberScalingLazyListState()
    // In the reminder list's own hand: a reminder added or removed here lands here.
    val focus = rememberSideLanding(core, Side.REMINDERS, keys) { state.scrollToItem(it) }
    WearList(state) {
        item { Heading(list?.announcement?.replaceFirstChar { it.uppercase() } ?: "Reminders") }
        list?.reminders.orEmpty().forEachIndexed { index, reminder ->
            item {
                val line = lines?.reminders?.getOrNull(index)
                val words = line?.title ?: reminder.words
                if (reminder.own) {
                    val actions = app.rowActions(reminder.id, reminder.offered, side = Side.REMINDERS)
                    RowButton(words, actions = actions, onLongClick = { navigator.actions(words, actions) }, key = reminder.id, focus = focus) {
                        navigator.actions(words, actions)
                    }
                } else {
                    // A default is changed in Planning, so there is nothing to do to it here.
                    Text(line?.text ?: words, Modifier.fillMaxWidth())
                }
            }
        }
        list?.offered.orEmpty().forEach { action ->
            item {
                // Adding the first reminder is when notifications are asked for.
                Button(
                    onClick = {
                        ask()
                        app.send(ShellEvent.ActOn(ActingOn.Reminders, action))
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
    val access = dev.emassey0135.lumenna.LocalReminderAccess.current
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
