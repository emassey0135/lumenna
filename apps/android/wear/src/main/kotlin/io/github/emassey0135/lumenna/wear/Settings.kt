package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.rowActions
import io.github.emassey0135.lumenna.appButton
import io.github.emassey0135.lumenna.app.pairingWords
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.SettingsEvent
import io.github.emassey0135.lumenna.app.SettingView
import io.github.emassey0135.lumenna.app.SettingKind
import io.github.emassey0135.lumenna.app.PairEvent
import io.github.emassey0135.lumenna.app.DevicesEvent
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.collectAsState
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
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.PairingSession
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.backups
import io.github.emassey0135.lumenna.button
import io.github.emassey0135.lumenna.on
import io.github.emassey0135.lumenna.planning
import io.github.emassey0135.lumenna.said
import io.github.emassey0135.lumenna.sentence
import io.github.emassey0135.lumenna.set
import io.github.emassey0135.lumenna.settings
import androidx.wear.compose.foundation.lazy.ScalingLazyListScope
import androidx.wear.compose.material3.RadioButton

/** Settings, as the phone groups them: what syncs, the devices, backups. */
@Composable
fun SettingsScreen(navigator: Navigator) {
    WearList {
        item { Heading("Settings") }
        item { Button(onClick = { navigator.open(Screen.Planning) }, modifier = Modifier.fillMaxWidth(), label = { Text("Planning") }) }
        item { Button(onClick = { navigator.open(Screen.Devices) }, modifier = Modifier.fillMaxWidth(), label = { Text("Devices and sync") }) }
        item { Button(onClick = { navigator.open(Screen.ReminderSettings) }, modifier = Modifier.fillMaxWidth(), label = { Text("Reminders") }) }
    }
}

/** The app's settings, open while a page showing them is. */
@Composable
fun appSettings(core: Core): List<SettingView> {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Settings(SettingsEvent.Open)) }
    return view?.settings?.settings.orEmpty()
}

/**
 * What syncs to every device, as the phone shows it; and this watch's backups. Each setting's
 * name, options and what it does are the core's; applying it is the app's.
 */
@Composable
fun PlanningScreen(core: Core) {
    val all = appSettings(core)
    WearList {
        item { Heading("Planning") }
        all.filter { it.setting.syncs }.forEach { setting(core, it) }
        item { Text("These sync to all your devices.") }
        item { Heading("Backups") }
        all.filter { !it.setting.syncs && it.setting.key.startsWith("backup-") && it.setting.kind != SettingKind.FOLDER }.forEach { setting(core, it) }
        item {
            // Into this watch's backups, beside the others.
            Button(onClick = { core.attempt { core.lumenna.backup(null) }?.let { core.say(sentence(it.announcement, it.notices)) } }, modifier = Modifier.fillMaxWidth(), label = { Text("Back up now") })
        }
    }
}

/**
 * The paired devices, the app's, how syncing with each last went, Sync Now, and pairing
 * another — a Wear OS watch is a peer of its own. Each device's actions are the app's.
 */
@Composable
fun DevicesScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Devices(DevicesEvent.Show)) }
    val devices = view?.devices
    WearList {
        item { Heading("Devices and sync") }
        devices?.status?.takeIf { it.isNotEmpty() }?.let { item { Text(it) } }
        item { Button(onClick = { syncNow(core) }, modifier = Modifier.fillMaxWidth(), label = { Text("Sync now") }) }
        devices?.list?.devices.orEmpty().forEach { device ->
            item {
                // The core's: Rename, and Unpair on every row but this watch's own. Sync Now is
                // with every device at once, so it is the screen's button above.
                val actions = app.rowActions(device.nodeId, device.actions)
                RowButton(
                    device.name,
                    detail = (listOf(device.platform) + device.status).joinToString(", "),
                    actions = actions,
                    onLongClick = { navigator.actions(device.name, actions) },
                ) { navigator.actions(device.name, actions) }
            }
        }
        item { PairingButtons(core) }
    }
}

/** A round with every paired device now, on the UniFFI core while it holds the endpoint. */
private fun syncNow(core: Core) {
    core.say("Syncing")
    core.syncNow { result ->
        result.fold(
            { report -> core.changed(); core.say(sentence(report.announcement, report.peers.mapNotNull { peer -> peer.error?.let { "${peer.name}: $it" } })) },
            { error -> core.say(error.message.orEmpty()) },
        )
    }
}

/**
 * Pairing, as the app runs it: wait to be found, or join with a code; then compare the words.
 * Every sentence and button is the core's (`pairingWords`), buttons in sentence case. The code
 * is asked for as a field is, by the system's input screen, and joined with once given.
 */
@Composable
private fun PairingButtons(core: Core) {
    val app = core.app
    val view by app.view.collectAsState()
    val pairing = view?.devices?.pairing
    val pair = { event: PairEvent -> app.send(ShellEvent.Devices(DevicesEvent.Pair(event))) }
    val context = LocalContext.current
    val words = remember { pairingWords("this watch", true) }
    LaunchedEffect(Unit) { pair(PairEvent.Open(PairingSession.deviceName(context))) }
    DisposableEffect(Unit) { onDispose { pair(PairEvent.Cancel) } }
    androidx.compose.foundation.layout.Column {
        Heading(appButton(words.title))
        val status = pairing?.problem ?: when {
            pairing?.mine != null -> words.waiting
            pairing?.running == true -> words.connecting
            else -> words.introSentence.ifEmpty { words.intro }
        }
        Text(status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        pairing?.mine?.let { Text("${words.myCode}: $it") }
        Button(onClick = { pair(PairEvent.Wait) }, enabled = pairing?.running != true, modifier = Modifier.fillMaxWidth(), label = { Text(appButton(words.wait)) })
        if (pairing?.running != true || pairing.waiting) {
            TextFieldButton(words.theirCode, "") { code ->
                pair(PairEvent.Code(code))
                pair(PairEvent.Join)
            }
        }
        pairing?.words?.let { three ->
            Text("${appButton(words.matchTitle)} ${words.matchMessage} ${three.joinToString(", ")}.", Modifier.semantics { liveRegion = LiveRegionMode.Assertive })
            Button(onClick = { pair(PairEvent.Confirm(true)) }, modifier = Modifier.fillMaxWidth(), label = { Text(appButton(words.matchYes)) })
            Button(onClick = { pair(PairEvent.Confirm(false)) }, modifier = Modifier.fillMaxWidth(), label = { Text(appButton(words.matchNo)) })
        }
    }
}

/** One setting, by the control its kind asks for; applied through the app. */
private fun ScalingLazyListScope.setting(core: Core, shown: SettingView) {
    val setting = shown.setting
    val set = { value: String -> core.app.send(ShellEvent.Settings(SettingsEvent.Set(setting.key, value))) }
    when (setting.kind) {
        SettingKind.TOGGLE -> item {
            SwitchButton(
                checked = setting.value == "true",
                onCheckedChange = { set(if (it) "true" else "false") },
                modifier = Modifier.fillMaxWidth(),
                label = { Text(setting.title) },
            )
        }
        SettingKind.CHOICE -> {
            item { Heading(setting.title) }
            setting.options.forEach { option ->
                item {
                    RadioButton(
                        selected = setting.value == option.id,
                        onSelect = { set(option.id) },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(option.title) },
                    )
                }
            }
        }
        SettingKind.TIME, SettingKind.NUMBER, SettingKind.FOLDER, SettingKind.LINE -> item {
            val said = if (setting.kind == SettingKind.TIME) Clock.time(setting.value) else setting.value
            TextFieldButton(setting.title, said) { set(it.trim()) }
        }
    }
    // A refused line, beside why, as the app keeps it.
    shown.problem?.let { problem -> item { Text(problem) } }
    if (setting.hint.isNotEmpty()) item { Text(setting.hint) }
}
