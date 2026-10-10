package io.github.emassey0135.lumenna.wear

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.SwitchButton
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.Choice
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.DeviceAction
import io.github.emassey0135.lumenna.PairingSession
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.sentence

/** Settings, as the phone groups them: what syncs, the devices, backups. */
@Composable
fun SettingsScreen(navigator: Navigator) {
    WearList {
        item { Heading("Settings") }
        item { Button(onClick = { navigator.open(Screen.Planning) }, modifier = Modifier.fillMaxWidth(), label = { Text("Planning") }) }
        item { Button(onClick = { navigator.open(Screen.Devices) }, modifier = Modifier.fillMaxWidth(), label = { Text("Devices and Sync") }) }
    }
}

private fun settings(core: Core): Map<String, String> =
    core.attempt { core.lumenna.settings(null).settings.associate { it.key to it.value } }.orEmpty()

private val weekdays = listOf("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")

/** What syncs to every device, as the phone shows it; and this watch's backups. */
@Composable
fun PlanningScreen(core: Core, changes: Long) {
    val values = remember(changes) { settings(core) }
    val entry = LocalTextEntry.current
    val set = { key: String, value: String -> core.change { it.setSetting(key, value) } }
    val time = { key: String, name: String -> entry?.ask(name) { set(key, it.trim()) } }
    WearList {
        item { Heading("Planning") }
        item {
            SwitchButton(
                checked = values["cascade-complete-subtasks"] == "true",
                onCheckedChange = { set("cascade-complete-subtasks", if (it) "true" else "false") },
                modifier = Modifier.fillMaxWidth(),
                label = { Text("Completing a task completes its subtasks") },
            )
        }
        item { TextFieldButton("Day starts", values["day-start"].orEmpty().let(Clock::time)) { set("day-start", it.trim()) } }
        item { TextFieldButton("Day ends", values["day-end"].orEmpty().let(Clock::time)) { set("day-end", it.trim()) } }
        item {
            TextFieldButton("All-day reminders at", values["all-day-reminder-hour"].orEmpty().let(Clock::time)) {
                set("all-day-reminder-hour", it.trim())
            }
        }
        item { Heading("Announcements") }
        listOf("Full sentences" to "full", "Terse" to "terse").forEach { (name, value) ->
            item {
                androidx.wear.compose.material3.RadioButton(
                    selected = values["verbosity"] == value,
                    onSelect = { set("verbosity", value) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(name) },
                )
            }
        }
        item { Heading("Week starts on") }
        weekdays.forEach { value ->
            item {
                androidx.wear.compose.material3.RadioButton(
                    selected = values["week-start"] == value,
                    onSelect = { set("week-start", value) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(value.replaceFirstChar { it.uppercase() }) },
                )
            }
        }
        item { Text("These sync to all your devices.") }
        item { Heading("Backups") }
        val every = mapOf("12h" to "Every 12 hours", "1d" to "Every day", "7d" to "Every week", "off" to "Off")
        every.forEach { (value, name) ->
            item {
                androidx.wear.compose.material3.RadioButton(
                    selected = values["backup-every"] == value,
                    onSelect = { set("backup-every", value) },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(name) },
                )
            }
        }
        item {
            Button(onClick = { core.attempt { core.lumenna.backup(null) }?.let { core.say(sentence(it.announcement, it.notices)) } }, modifier = Modifier.fillMaxWidth(), label = { Text("Back Up Now") })
        }
        item { Text("A backup holds your whole history, including every task you deleted. It stays on this watch.") }
    }
}

/**
 * The paired devices, how syncing with each last went, Sync Now, and pairing another — a
 * Wear OS watch is a peer of its own. Each device's actions are the phone's (`DeviceAction`).
 */
@Composable
fun DevicesScreen(core: Core, navigator: Navigator, changes: Long) {
    val status = remember(changes) { core.attempt { core.lumenna.syncStatus() } }
    val entry = LocalTextEntry.current
    val syncNow = {
        core.say("Syncing")
        core.syncNow { result ->
            result.fold(
                { report -> core.changed(); core.say(sentence(report.announcement, report.peers.mapNotNull { peer -> peer.error?.let { "${peer.name}: $it" } })) },
                { error -> core.say((error as? io.github.emassey0135.lumenna.core.LumennaException)?.let { io.github.emassey0135.lumenna.sentence(it.message.orEmpty(), emptyList()) } ?: error.message.orEmpty()) },
            )
        }
    }
    WearList {
        item { Heading("Devices and Sync") }
        status?.let { item { Text(it.announcement) } }
        item { Button(onClick = { syncNow() }, modifier = Modifier.fillMaxWidth(), label = { Text("Sync Now") }) }
        status?.devices.orEmpty().forEach { device ->
            item {
                val actions = DeviceAction.of(thisDevice = device.thisDevice).map { action ->
                    when (action) {
                        DeviceAction.SYNC_NOW -> RowAction(action.title) { syncNow() }
                        DeviceAction.RENAME -> RowAction(action.title) {
                            entry?.ask("Rename ${device.name}") { name -> core.change { it.renameDevice(device.nodeId, name.trim()) } }
                        }
                        DeviceAction.STOP_SYNCING -> RowAction(action.title) {
                            navigator.choose("Stop syncing with ${device.name}?", listOf(Choice("stop", "Stop Syncing")), DeviceAction.STOPPING) {
                                core.change { it.unpairDevice(device.nodeId) }
                            }
                        }
                    }
                }
                RowButton(
                    device.name,
                    detail = (listOf(device.platform) + device.status).joinToString(", "),
                    actions = actions,
                    onLongClick = { navigator.actions(device.name, actions) },
                ) { navigator.actions(device.name, actions) }
            }
        }
        item { PairingButtons(core, navigator) }
    }
}

/**
 * Pairing, as the phone runs it (`PairingSession`): wait to be found, or join with a code;
 * then compare the words.
 */
@Composable
private fun PairingButtons(core: Core, navigator: Navigator) {
    val context = LocalContext.current
    val entry = LocalTextEntry.current
    val session = remember { PairingSession(core, context) { core.changed() } }
    DisposableEffect(Unit) { onDispose { session.cancel() } }
    androidx.compose.foundation.layout.Column {
        Heading("Pair a Device")
        Text(session.status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        session.code?.let { Text("Pairing code: $it") }
        Button(onClick = { session.waitToBeFound() }, enabled = !session.running, modifier = Modifier.fillMaxWidth(), label = { Text("Wait for the Other Device") })
        Button(
            onClick = { entry?.ask("Code from the other device") { session.join(it) } },
            enabled = session.nextCode == null && (!session.running || session.waiting),
            modifier = Modifier.fillMaxWidth(),
            label = { Text("Pair With a Code") },
        )
        session.asked?.let { words ->
            Text("Do these words match? ${PairingSession.matchQuestion(words)}", Modifier.semantics { liveRegion = LiveRegionMode.Assertive })
            Button(onClick = { session.answer(true) }, modifier = Modifier.fillMaxWidth(), label = { Text("Yes, They Match") })
            Button(onClick = { session.answer(false) }, modifier = Modifier.fillMaxWidth(), label = { Text("No") })
        }
    }
}
