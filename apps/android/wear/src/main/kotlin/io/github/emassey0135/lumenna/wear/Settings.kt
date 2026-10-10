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
import io.github.emassey0135.lumenna.Option
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.Core
import io.github.emassey0135.lumenna.PairingSession
import io.github.emassey0135.lumenna.RowAction
import io.github.emassey0135.lumenna.backups
import io.github.emassey0135.lumenna.on
import io.github.emassey0135.lumenna.planning
import io.github.emassey0135.lumenna.said
import io.github.emassey0135.lumenna.sentence
import io.github.emassey0135.lumenna.set
import io.github.emassey0135.lumenna.settings
import io.github.emassey0135.lumenna.core.Setting
import io.github.emassey0135.lumenna.core.SettingKind
import androidx.wear.compose.foundation.lazy.ScalingLazyListScope
import androidx.wear.compose.material3.RadioButton

/** Settings, as the phone groups them: what syncs, the devices, backups. */
@Composable
fun SettingsScreen(navigator: Navigator) {
    WearList {
        item { Heading("Settings") }
        item { Button(onClick = { navigator.open(Screen.Planning) }, modifier = Modifier.fillMaxWidth(), label = { Text("Planning") }) }
        item { Button(onClick = { navigator.open(Screen.Devices) }, modifier = Modifier.fillMaxWidth(), label = { Text("Devices and Sync") }) }
    }
}

/** What syncs to every device, as the phone shows it; and this watch's backups. Each setting's
 *  name, options and what it does are the core's. */
@Composable
fun PlanningScreen(core: Core, changes: Long) {
    val all = remember(changes) { settings(core) }
    WearList {
        item { Heading("Planning") }
        all.planning().forEach { setting(core, it) }
        item { Text("These sync to all your devices.") }
        item { Heading("Backups") }
        all.backups().forEach { setting(core, it) }
        item {
            Button(onClick = { core.attempt { core.lumenna.backup(null) }?.let { core.say(sentence(it.announcement, it.notices)) } }, modifier = Modifier.fillMaxWidth(), label = { Text("Back Up Now") })
        }
    }
}

/**
 * The paired devices, how syncing with each last went, Sync Now, and pairing another — a
 * Wear OS watch is a peer of its own. Each device's actions are the core's.
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
                // The core's: Rename, and Unpair on every row but this watch's own. Sync Now is
                // with every device at once, so it is the screen's button above.
                val actions = core.offered(device.actions, navigator, entry)
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
    val session = remember { PairingSession(core, context, PLATFORM) { core.changed() } }
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

/** One setting, by the control its kind asks for. */
private fun ScalingLazyListScope.setting(core: Core, setting: Setting) {
    when (setting.kind) {
        SettingKind.TOGGLE -> item {
            SwitchButton(
                checked = setting.on,
                onCheckedChange = { core.set(setting, if (it) "true" else "false") },
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
                        onSelect = { core.set(setting, option.id) },
                        modifier = Modifier.fillMaxWidth(),
                        label = { Text(option.title) },
                    )
                }
            }
        }
        SettingKind.TIME, SettingKind.NUMBER, SettingKind.FOLDER -> item {
            TextFieldButton(setting.title, setting.said) { core.set(setting, it.trim()) }
        }
    }
    if (setting.hint.isNotEmpty()) item { Text(setting.hint) }
}
