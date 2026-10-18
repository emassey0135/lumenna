package io.github.emassey0135.lumenna

import androidx.compose.material3.HorizontalDivider
import androidx.compose.ui.semantics.CollectionInfo
import androidx.compose.ui.semantics.collectionInfo
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.Icons
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.key
import io.github.emassey0135.lumenna.app.DevicesEvent
import io.github.emassey0135.lumenna.app.PairEvent
import io.github.emassey0135.lumenna.app.SaveFile
import io.github.emassey0135.lumenna.app.Setting
import io.github.emassey0135.lumenna.app.SettingKind
import io.github.emassey0135.lumenna.app.SettingView
import io.github.emassey0135.lumenna.app.SettingsEvent
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.pairingWords
import android.content.Context
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.RadioButton
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * Settings: a short list of pages — what syncs, what is this device's alone, and getting
 * data in and out — so backups are reached without swiping past every weekday. The settings
 * and the devices are the app's (`ViewModel.settings`, `devices`), open while their pages are.
 */
@Composable
fun SettingsScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Devices(DevicesEvent.Show)) }
    val status = view?.devices?.status.orEmpty()
    ItemListScreen(
        "Settings", core, navigator, changes = status.hashCode().toLong(),
        load = {
            listOf(
                Item("devices", "Devices and sync", status),
                Item("planning", "Planning", "The day, the week, completing subtasks, announcements"),
                Item("reminders", "Reminders", "On this device: notifications and alarms"),
                Item("backups", "Backups", "On this device only"),
                Item("export", "Export and import", "JSON, Markdown, org, calendar"),
            ).let { Listing(it) }
        },
        open = { item ->
            navigator.push(
                when (item.key) {
                    "devices" -> Screen.Devices
                    "planning" -> Screen.Planning
                    "reminders" -> Screen.ReminderSettings
                    "backups" -> Screen.Backups
                    else -> Screen.Export
                },
            )
        },
    )
}

/** The app's settings, open while a page showing them is: each as the store has it, or as typed. */
@Composable
private fun appSettings(core: Core): List<SettingView> {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Settings(SettingsEvent.Open)) }
    return view?.settings?.settings.orEmpty()
}

/** A setting's name and its value, opening whatever changes it; what it does beneath. */
@Composable
private fun SettingRow(name: String, value: String, hint: String, change: () -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .clickable(role = Role.Button, onClick = change)
            .semantics(mergeDescendants = true) {}
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge)
        Text(value, style = MaterialTheme.typography.bodyMedium, color = quiet())
        if (hint.isNotEmpty()) Text(hint, style = MaterialTheme.typography.bodySmall, color = quiet())
    }
}

/**
 * A choice among a few: Material's simple dialog, the current one selected, and choosing one
 * sets it and closes. As a list of plain buttons, nothing said which was set.
 */
@Composable
private fun ChooseOne(
    title: String,
    choices: List<Pair<String, String>>,
    current: String,
    dismiss: () -> Unit,
    chosen: (String) -> Unit,
) {
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text(title) },
        text = {
            Column(Modifier.selectableGroup()) {
                choices.forEach { (name, value) ->
                    Row(
                        Modifier
                            .fillMaxWidth()
                            .heightIn(min = 48.dp)
                            .selectable(value == current, role = Role.RadioButton) { chosen(value) }
                            .padding(vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        RadioButton(selected = value == current, onClick = null)
                        Text(name, Modifier.padding(start = 12.dp))
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") } },
    )
}

/** A setting's value as it is said: its option's name, a time in this device's clock. */
private val Setting.said: String
    get() = when (kind) {
        SettingKind.TIME -> Clock.time(value)
        SettingKind.TOGGLE, SettingKind.CHOICE -> options.firstOrNull { it.id == value }?.title ?: value
        else -> value
    }

/**
 * One setting, by the control its kind asks for: a switch, a choice among its options, a time
 * or a number typed. Its name, options and what it does are the core's; applying it, and why a
 * line was refused, are the app's: a refused line stays in its dialog, beside why.
 */
@Composable
private fun SettingControl(core: Core, shown: SettingView, turnedOn: () -> Unit = {}) {
    val app = core.app
    val setting = shown.setting
    val set = { value: String -> app.send(ShellEvent.Settings(SettingsEvent.Set(setting.key, value))) }
    var asking by remember { mutableStateOf(false) }
    var sent by remember { mutableStateOf<String?>(null) }
    // Applied as typed, the dialog closes; refused, it stays with the app's reason.
    LaunchedEffect(shown) {
        val value = sent ?: return@LaunchedEffect
        if (shown.problem == null && shown.setting.value == value) {
            asking = false
            sent = null
        }
    }
    when (setting.kind) {
        SettingKind.TOGGLE -> Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            val on = setting.value == "true"
            Row(
                Modifier
                    .fillMaxWidth()
                    .toggleable(on, role = Role.Switch) {
                        set(if (it) "true" else "false")
                        if (it) turnedOn()
                    },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(setting.title, Modifier.weight(1f))
                Switch(checked = on, onCheckedChange = null)
            }
            if (setting.hint.isNotEmpty()) Text(setting.hint, style = MaterialTheme.typography.bodySmall, color = quiet())
        }
        SettingKind.CHOICE -> {
            SettingRow(setting.title, setting.said, setting.hint) { asking = true }
            if (asking) {
                ChooseOne(setting.title, setting.options.map { it.title to it.id }, setting.value, { asking = false }) {
                    asking = false
                    set(it)
                }
            }
        }
        SettingKind.TIME, SettingKind.NUMBER, SettingKind.FOLDER, SettingKind.LINE -> {
            SettingRow(setting.title, setting.said, setting.hint) { asking = true }
            if (asking) {
                val time = setting.kind == SettingKind.TIME
                AskText(
                    setting.title, if (time) "Time" else setting.title, "Set",
                    initial = setting.value,
                    example = if (time) "9am" else "",
                    hint = shown.problem ?: if (time) "Such as 8am or 21:30." else null,
                    number = setting.kind == SettingKind.NUMBER,
                    dismiss = {
                        asking = false
                        sent = null
                    },
                ) {
                    val value = it.trim()
                    sent = value
                    set(value)
                }
            }
        }
    }
}

/** What syncs to every device: the core's settings that sync, in its order. */
@Composable
fun PlanningScreen(core: Core, navigator: Navigator) {
    val shown = appSettings(core).filter { it.setting.syncs }
    ScreenFrame("Planning", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { key(it.setting.key) { SettingControl(core, it) } }
            Text(
                "These sync to all your devices.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
                modifier = Modifier.padding(16.dp),
            )
        }
    }
}

/**
 * Reminders on this device: the core's setting for whether it gives them, then what Android
 * allows — notifications, and alarms at an exact time — each with the system's page that
 * grants it. When each reminder comes is set in Planning and on a task or block, and syncs.
 */
@Composable
fun ReminderSettingsScreen(core: Core, navigator: Navigator, changes: Long) {
    val context = LocalContext.current
    val shown = appSettings(core).filter { it.setting.key == "reminders" }
    // Read again whenever the app comes back, as from the system's settings (`changes` moves).
    val access = LocalReminderAccess.current
    val (allowed, exact, grant) = remember(changes) { access(context) }
    val ask = rememberNotificationAsk()
    ScreenFrame("Reminders", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { key(it.setting.key) { SettingControl(core, it, turnedOn = ask) } }
            if (!allowed) {
                Text(
                    "Notifications are off for Lumenna, so no reminder can be shown on this device.",
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
                OutlinedButton(
                    modifier = Target.padding(horizontal = 16.dp),
                    onClick = { if (Reminders.shouldAsk(context)) ask() else context.startActivity(Reminders.notificationSettings(context)) },
                ) { Text("Allow notifications") }
            }
            if (!exact) {
                Text(
                    "Reminders may come a few minutes late: Lumenna is not allowed to set alarms for an exact time.",
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
                grant?.let { intent ->
                    OutlinedButton(modifier = Target.padding(horizontal = 16.dp), onClick = { context.startActivity(intent) }) {
                        Text("Allow exact alarms")
                    }
                }
            }
            Text(
                "When reminders come is set in Planning, and on a task or block in its Reminders; those sync to all " +
                    "your devices. This page is this device's own.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
                modifier = Modifier.padding(16.dp),
            )
        }
    }
}

/**
 * Backups, which are this device's alone: its own backup settings, but for where they go,
 * which on Android is the app's own storage and not a folder to choose.
 */
@Composable
fun BackupsScreen(core: Core, navigator: Navigator) {
    val shown = appSettings(core).filter { !it.setting.syncs && it.setting.key.startsWith("backup-") && it.setting.kind != SettingKind.FOLDER }
    ScreenFrame("Backups", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { key(it.setting.key) { SettingControl(core, it) } }
            Button(
                modifier = Target.padding(16.dp),
                // Into this device's backups, beside the others: the app's backup is a file to keep elsewhere.
                onClick = { core.attempt { core.lumenna.backup(null) }?.let { core.say(sentence(it.announcement, it.notices)) } },
            ) { Text("Back up now") }
            Text(
                "These settings are this device's own. Restore a backup from Export and import.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
    }
}

/**
 * Getting data out and back in, through Android's own file picker, so an export can go to
 * Drive or anywhere else a provider offers. The app makes the file; the person chooses where
 * it goes. An export is the present state, nothing from the trash; importing one, or restoring
 * a backup, adds what this device lacks and removes nothing.
 */
@Composable
fun ExportScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val context = LocalContext.current
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Settings(SettingsEvent.Open)) }
    var made by remember { mutableStateOf<SaveFile?>(null) }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
        val file = made ?: return@rememberLauncherForActivityResult
        made = null
        if (uri == null) return@rememberLauncherForActivityResult
        context.contentResolver.openOutputStream(uri)?.use { it.write(file.bytes) }
    }
    // Each file the app makes is saved where the person chooses.
    LaunchedEffect(Unit) {
        app.files.collect { file ->
            made = file
            save.launch(file.name)
        }
    }
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        val name = context.contentResolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { if (it.moveToFirst()) it.getString(0) else null } ?: "import"
        val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: return@rememberLauncherForActivityResult
        app.send(ShellEvent.Settings(SettingsEvent.Import(name, bytes)))
    }
    ScreenFrame("Export and import", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            view?.settings?.exports.orEmpty().forEach { export ->
                OutlinedButton(modifier = Target, onClick = { app.send(ShellEvent.Settings(SettingsEvent.Export(export.format))) }) {
                    Text("${appButton(export.label)}…")
                }
            }
            OutlinedButton(modifier = Target, onClick = { open.launch(arrayOf("*/*")) }) { Text("Import or restore…") }
            Text(
                "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a " +
                    "backup adds what this device lacks and removes nothing.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
            )
        }
    }
}

/** The paired devices, how syncing with each last went, and pairing another: the app's. */
@Composable
fun DevicesScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Devices(DevicesEvent.Show)) }
    val devices = view?.devices
    ScreenFrame("Devices and sync", core, navigator, actions = {
        IconButton(onClick = { navigator.push(Screen.Pairing) }) { Icon(Icons.Filled.Add, contentDescription = "Pair a device") }
    }) {
        Text(
            devices?.status.orEmpty(),
            style = MaterialTheme.typography.bodyMedium,
            color = quiet(),
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        // Syncing is with every device at once, so it is the screen's, not a row's.
        OutlinedButton(modifier = Target.padding(horizontal = 16.dp), onClick = { syncNow(core) }) { Text("Sync now") }
        val list = devices?.list
        val rows = list?.devices.orEmpty()
        val state = rememberLazyListState()
        val focus = rememberHeldFocus(core, rows.map { it.nodeId }, state, rows)
        if (rows.isEmpty() && list != null) {
            Text(list.empty, Modifier.padding(16.dp), color = quiet())
        }
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(rows.size, 1) }, state = state) {
            itemsIndexed(rows, key = { _, device -> device.nodeId }) { index, device ->
                // How syncing with it is going, as the core words it for every app.
                val detail = (listOf(device.platform) + device.status).joinToString(", ")
                ListRow(
                    title = device.name,
                    detail = detail,
                    speech = detail,
                    index = index,
                    // The core's: Rename, and Unpair on every row but this device's own.
                    actions = focus.actions(device.nodeId, index, app.rowActions(device.nodeId, device.actions)),
                    open = null,
                    openLabel = "Open",
                    focus = focus.requester(device.nodeId),
                    key = device.nodeId,
                    say = core::say,
                    thisDevice = device.thisDevice,
                )
                HorizontalDivider()
            }
        }
    }
}

/**
 * Pairing this phone with another of the person's devices, as the app runs it. On one network
 * the two find each other; anywhere else, one shows a code and the other enters it. Either way
 * both show the same three words, and nothing is paired unless the person says they match on
 * both. Every sentence and button is the core's (`pairingWords`), the buttons in sentence case.
 */
@Composable
fun PairingScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val context = LocalContext.current
    val view by app.view.collectAsState()
    val name = remember { PairingSession.deviceName(context) }
    LaunchedEffect(Unit) {
        app.send(ShellEvent.Devices(DevicesEvent.Show))
        app.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Open(name))))
    }
    val pairing = view?.devices?.pairing
    var opened by remember { mutableStateOf(false) }
    // Paired, the app closes the pairing, and the screen goes back.
    LaunchedEffect(pairing == null) { if (pairing != null) opened = true else if (opened) navigator.back() }
    // Leaving the screen gives up, which ends the wait for the other device.
    DisposableEffect(Unit) { onDispose { app.send(ShellEvent.Devices(DevicesEvent.Pair(PairEvent.Cancel))) } }
    val pair = { event: PairEvent -> app.send(ShellEvent.Devices(DevicesEvent.Pair(event))) }
    val words = remember(name) { pairingWords(name, true) }
    var entered by remember { mutableStateOf("") }

    ScreenFrame(appButton(words.title), core, navigator) {
        if (pairing == null) return@ScreenFrame
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            val status = pairing.problem ?: when {
                pairing.mine != null -> words.waiting
                pairing.running -> words.connecting
                else -> words.introSentence.ifEmpty { words.intro }
            }
            Text(status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
            Button(modifier = Target, onClick = { pair(PairEvent.Wait) }, enabled = !pairing.running) { Text(appButton(words.wait)) }
            pairing.mine?.let { code ->
                Text("${words.myCode}: $code", fontFamily = FontFamily.Monospace)
                OutlinedButton(modifier = Target, onClick = {
                    val clipboard = context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                    clipboard.setPrimaryClip(android.content.ClipData.newPlainText(words.myCode, code))
                    core.say(words.copied)
                }) { Text(appButton(words.copyCode)) }
            }
            OutlinedTextField(
                value = entered,
                onValueChange = {
                    entered = it
                    pair(PairEvent.Code(it))
                },
                label = { Text(words.theirCode) },
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            Button(modifier = Target, onClick = { pair(PairEvent.Join) }) { Text(appButton(words.join)) }
        }
        pairing.words?.let { three ->
            AlertDialog(
                onDismissRequest = {},
                title = { Text(appButton(words.matchTitle)) },
                text = { Text("${words.matchMessage} ${three.joinToString(", ")}.") },
                confirmButton = { TextButton(modifier = Target, onClick = { pair(PairEvent.Confirm(true)) }) { Text(appButton(words.matchYes)) } },
                dismissButton = { TextButton(modifier = Target, onClick = { pair(PairEvent.Confirm(false)) }) { Text(appButton(words.matchNo)) } },
            )
        }
    }
}
