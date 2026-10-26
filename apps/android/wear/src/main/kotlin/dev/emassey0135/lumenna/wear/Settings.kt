package dev.emassey0135.lumenna.wear

import dev.emassey0135.lumenna.on
import dev.emassey0135.lumenna.SettingsPages
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.name
import dev.emassey0135.lumenna.deviceName
import dev.emassey0135.lumenna.rowActions
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.SettingView
import dev.emassey0135.lumenna.app.SettingKind
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.DevicesEvent
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
import androidx.wear.compose.material3.ButtonDefaults
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.SwitchButton
import androidx.wear.compose.material3.Text
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.commandTitle
import dev.emassey0135.lumenna.RowAction
import androidx.wear.compose.foundation.lazy.ScalingLazyListScope
import androidx.wear.compose.material3.RadioButton

/** Settings, as the phone groups them: what syncs, the devices, backups. */
@Composable
fun SettingsScreen(core: Core, navigator: Navigator) {
    val listed = appSettings(core)
    WearList {
        item { Heading(commandTitle("settings")) }
        // The core's pages, but Export and Import: a watch has nowhere to save a file or pick one.
        SettingsPages.shown(listed).filter { it.id != "export" }.forEach { page ->
            item {
                Button(
                    onClick = {
                        navigator.open(
                            when (page.id) {
                                "devices" -> Screen.Devices
                                "reminders" -> Screen.ReminderSettings
                                else -> Screen.SettingsPage(page.id)
                            },
                        )
                    },
                    modifier = Modifier.fillMaxWidth(),
                    label = { Text(page.title) },
                    secondaryLabel = { Text(page.description) },
                )
            }
        }
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
 * One of the core's pages of settings (Planning, Backups, General), as the phone shows it:
 * each setting's name, options and what it does are the core's; applying it is the app's.
 * Backups' Back Up Now is into this watch's backups, beside the others; restoring one is the
 * phone's, which can pick a file.
 */
@Composable
fun SettingsPageScreen(core: Core, id: String) {
    val page = SettingsPages.of(id)
    val all = appSettings(core)
    val view by core.app.view.collectAsState()
    WearList {
        item { Heading(page?.title.orEmpty()) }
        all.on(page).filter { it.setting.kind != SettingKind.FOLDER }.forEach { setting(core, it) }
        page?.buttons.orEmpty().filter { it.id == "back-up" }.forEach { button ->
            item { Button(onClick = { core.backUpNow() }, modifier = Modifier.fillMaxWidth(), label = { Text(button.title) }) }
        }
        page?.caption?.takeIf { it.isNotEmpty() }?.let { item { Text(it) } }
        if (id != "backups") return@WearList
        // What is done to the store as a whole, the core's (Erase everything deleted), beside the
        // backups it takes with it: a watch is a peer, its store whole. The app asks its question.
        view?.settings?.offered.orEmpty().forEach { action ->
            item {
                Button(
                    onClick = { core.app.send(ShellEvent.ActOn(ActingOn.Settings, action)) },
                    modifier = Modifier.fillMaxWidth(),
                    colors = if (action.destructive) {
                        ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.errorContainer, contentColor = MaterialTheme.colorScheme.onErrorContainer)
                    } else {
                        ButtonDefaults.buttonColors()
                    },
                    label = { Text(action.name) },
                )
            }
        }
        // What they take, the core's words, as every app says it.
        view?.settings?.offeredCaption?.takeIf { it.isNotEmpty() }?.let { caption -> item { Text(caption) } }
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
    val lines by app.lines.collectAsState()
    WearList {
        item { Heading(SettingsPages.of("devices")?.title.orEmpty()) }
        devices?.status?.takeIf { it.isNotEmpty() }?.let { item { Text(it) } }
        val syncing = SettingsPages.of("devices")?.buttons?.firstOrNull { it.id == "sync-now" }?.title ?: commandTitle("sync-now")
        item { Button(onClick = { app.syncNow() }, modifier = Modifier.fillMaxWidth(), label = { Text(syncing) }) }
        devices?.list?.devices.orEmpty().forEachIndexed { index, device ->
            item {
                // The core's: Rename, and Unpair on every row but this watch's own. Sync Now is
                // with every device at once, so it is the screen's button above.
                val actions = app.rowActions(device.nodeId, device.offered, side = Side.DEVICES)
                // How syncing with it is going, as the core words it for every app.
                val line = lines?.devices?.getOrNull(index)
                RowButton(
                    line?.title ?: device.name,
                    detail = line?.detail.orEmpty(),
                    actions = actions,
                    onLongClick = { navigator.actions(device.name, actions) },
                ) { navigator.actions(device.name, actions) }
            }
        }
        item { PairingButtons(core) }
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
    LaunchedEffect(Unit) {
        // What pairing's sentences call this watch, which finds others on its network.
        app.send(ShellEvent.PairingAs("this watch", true))
        app.send(ShellEvent.Devices(DevicesEvent.Show))
        pair(PairEvent.Open(deviceName(context)))
    }
    // Pairing's words come with the Devices page, in the watch's capitals.
    val words = view?.devices?.words ?: return
    DisposableEffect(Unit) { onDispose { pair(PairEvent.Cancel) } }
    androidx.compose.foundation.layout.Column {
        Heading(words.title)
        val status = pairing?.problem ?: when {
            pairing?.mine != null -> words.waiting
            pairing?.running == true -> words.connecting
            else -> words.introSentence.ifEmpty { words.intro }
        }
        Text(status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        pairing?.mineLine?.let { Text(it) }
        Button(onClick = { pair(PairEvent.Wait) }, enabled = pairing?.running != true, modifier = Modifier.fillMaxWidth(), label = { Text(words.wait) })
        if (pairing?.running != true || pairing.waiting) {
            TextFieldButton(words.theirCode, "") { code ->
                pair(PairEvent.Code(code))
                pair(PairEvent.Join)
            }
        }
        pairing?.words?.let { three ->
            Text("${words.matchTitle} ${pairing.matching ?: three.joinToString(", ")}", Modifier.semantics { liveRegion = LiveRegionMode.Assertive })
            Button(onClick = { pair(PairEvent.Confirm(true)) }, modifier = Modifier.fillMaxWidth(), label = { Text(words.matchYes) })
            Button(onClick = { pair(PairEvent.Confirm(false)) }, modifier = Modifier.fillMaxWidth(), label = { Text(words.matchNo) })
        }
    }
}

/** A time setting's example, the core's. */
private val timeExample by lazy { dev.emassey0135.lumenna.app.settingWords().timeExample }

/** One setting, by the control its kind asks for; applied through the app. */
internal fun ScalingLazyListScope.setting(core: Core, shown: SettingView) {
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
        SettingKind.TIME, SettingKind.NUMBER, SettingKind.FOLDER, SettingKind.LINE -> {
            item {
                val said = if (setting.kind == SettingKind.TIME) Clock.time(setting.value) else setting.value
                TextFieldButton(setting.title, said, if (setting.kind == SettingKind.TIME) timeExample else "") { set(it.trim()) }
            }
            // A time has the watch's own clock beside its line, named for the setting beneath, as
            // there are several.
            if (setting.kind == SettingKind.TIME) item { ChooseTime(setting.value, of = setting.title) { set(it) } }
        }
    }
    // A refused line, beside why, as the app keeps it.
    shown.problem?.let { problem -> item { Text(problem) } }
    if (setting.hint.isNotEmpty()) item { Text(setting.hint) }
}
