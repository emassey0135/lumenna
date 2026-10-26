package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.settingWords
import dev.emassey0135.lumenna.app.notificationWords
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
import dev.emassey0135.lumenna.app.ActingOn
import dev.emassey0135.lumenna.app.Side
import dev.emassey0135.lumenna.app.DevicesEvent
import dev.emassey0135.lumenna.app.PairEvent
import dev.emassey0135.lumenna.app.SaveFile
import dev.emassey0135.lumenna.app.Setting
import dev.emassey0135.lumenna.app.SettingKind
import dev.emassey0135.lumenna.app.SettingView
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.ShellEvent
import android.content.Context
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
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
    val listed = appSettings(core)
    val status = view?.devices?.status.orEmpty()
    // The core's pages, each with what is on it; the devices' with how syncing is going.
    val items = SettingsPages.shown(listed).map { page ->
        Item(page.id, page.title, if (page.id == "devices") status.ifEmpty { page.description } else page.description)
    }
    MenuScreen(
        Command.SETTINGS.title, core, navigator, items,
        open = { item ->
            navigator.push(
                when (item.key) {
                    "devices" -> Screen.Devices
                    "reminders" -> Screen.ReminderSettings
                    "backups" -> Screen.Backups
                    "export" -> Screen.Export
                    else -> Screen.SettingsPage(item.key)
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
    val words = remember { settingWords() }
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
                    // What a time or a number takes, and the button, in the core's words.
                    setting.title, setting.title, words.set,
                    initial = setting.value,
                    example = if (time) words.timeExample else "",
                    hint = shown.problem ?: when (setting.kind) {
                        SettingKind.TIME -> words.timeHint
                        SettingKind.NUMBER -> words.numberHint
                        else -> null
                    },
                    number = setting.kind == SettingKind.NUMBER,
                    time = time,
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

/** One of the core's pages of settings (Planning, General): its settings in its order, then what it says of them. */
@Composable
fun SettingsPageScreen(core: Core, navigator: Navigator, id: String) {
    val page = SettingsPages.of(id)
    val shown = appSettings(core).on(page)
    ScreenFrame(page?.title.orEmpty(), core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { key(it.setting.key) { SettingControl(core, it) } }
            Caption(page?.caption.orEmpty())
        }
    }
}

/** What a page says under its settings, the core's words. */
@Composable
private fun Caption(text: String) {
    if (text.isEmpty()) return
    Text(text, style = MaterialTheme.typography.bodySmall, color = quiet(), modifier = Modifier.padding(16.dp))
}

/** The notifications' words, as this device calls itself in them. */
val phoneNotificationWords by lazy { notificationWords("this device", true) }

/**
 * Reminders on this device: the core's setting for whether it gives them, then what Android
 * allows — notifications, and alarms at an exact time — each with the system's page that
 * grants it. When each reminder comes is set in Planning and on a task or block, and syncs.
 */
@Composable
fun ReminderSettingsScreen(core: Core, navigator: Navigator, changes: Long) {
    val context = LocalContext.current
    val page = SettingsPages.of("reminders")
    val shown = appSettings(core).on(page)
    val words = phoneNotificationWords
    // Read again whenever the app comes back, as from the system's settings (`changes` moves).
    val access = LocalReminderAccess.current
    val (allowed, exact, grant) = remember(changes) { access(context) }
    val ask = rememberNotificationAsk()
    ScreenFrame(page?.title.orEmpty(), core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { key(it.setting.key) { SettingControl(core, it, turnedOn = if (it.setting.key == "reminders") ask else ({})) } }
            if (!allowed) {
                Text(words.off, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                OutlinedButton(
                    modifier = Target.padding(horizontal = 16.dp),
                    onClick = { if (Reminders.shouldAsk(context)) ask() else context.startActivity(Reminders.notificationSettings(context)) },
                ) { Text(words.allow) }
            }
            if (!exact) {
                Text(words.late, modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp))
                grant?.let { intent ->
                    OutlinedButton(modifier = Target.padding(horizontal = 16.dp), onClick = { context.startActivity(intent) }) {
                        Text(words.allowExact)
                    }
                }
            }
            Caption(page?.caption.orEmpty())
        }
    }
}

/**
 * Backups, which are this device's alone: its own backup settings, but for where they go,
 * which on Android is the app's own storage and not a folder to choose.
 */
@Composable
fun BackupsScreen(core: Core, navigator: Navigator) {
    val page = SettingsPages.of("backups")
    val shown = appSettings(core).on(page).filter { it.setting.kind != SettingKind.FOLDER }
    val restore = rememberImport(core)
    ScreenFrame(page?.title.orEmpty(), core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { key(it.setting.key) { SettingControl(core, it) } }
            page?.buttons.orEmpty().forEach { button ->
                OutlinedButton(
                    modifier = Target.padding(horizontal = 16.dp, vertical = 4.dp),
                    onClick = {
                        when (button.id) {
                            // Into this device's backups, beside the others: the app's backup is a file to keep elsewhere.
                            "back-up" -> core.backUpNow()
                            // A backup is restored as an import is, from a file chosen.
                            else -> restore()
                        }
                    },
                ) { Text(if (button.asks) "${button.title}…" else button.title) }
            }
            Caption(page?.caption.orEmpty())
        }
    }
}

/**
 * Choosing a file to import or restore, through Android's own picker: read off the main thread,
 * since the provider chosen (Drive) can be slow, then handed to the app.
 */
@Composable
private fun rememberImport(core: Core): () -> Unit {
    val context = LocalContext.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val resolver = context.contentResolver
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch(Dispatchers.IO) {
            val name = runCatching {
                resolver.query(uri, arrayOf(android.provider.OpenableColumns.DISPLAY_NAME), null, null, null)
                    ?.use { if (it.moveToFirst()) it.getString(0) else null }
            }.getOrNull() ?: "import"
            val bytes = runCatching { resolver.openInputStream(uri)?.use { it.readBytes() } }.getOrNull() ?: return@launch
            core.app.send(ShellEvent.Settings(SettingsEvent.Import(name, bytes)))
        }
    }
    return { open.launch(arrayOf("*/*")) }
}

/**
 * Getting data out and back in, through Android's own file picker, so an export can go to
 * Drive or anywhere else a provider offers. The app makes the file; the person chooses where
 * it goes. An export is the present state, nothing from the trash; importing one, or restoring
 * a backup, adds what this device lacks and removes nothing. Beneath, what the core does to the
 * store as a whole (`SettingsView.offered`): erasing for good what was deleted, from history and
 * backups too, beside where the trash and backups are spoken of.
 */
@Composable
fun ExportScreen(core: Core, navigator: Navigator) {
    val app = core.app
    val context = LocalContext.current
    val view by app.view.collectAsState()
    LaunchedEffect(Unit) { app.send(ShellEvent.Settings(SettingsEvent.Open)) }
    var made by remember { mutableStateOf<SaveFile?>(null) }
    // Files are written and read off the main thread: a backup can be large, and the provider
    // chosen (Drive) can be slow.
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val resolver = context.contentResolver
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
        val file = made ?: return@rememberLauncherForActivityResult
        made = null
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch(Dispatchers.IO) { runCatching { resolver.openOutputStream(uri)?.use { it.write(file.bytes) } } }
    }
    // Each file the app makes is saved where the person chooses.
    LaunchedEffect(Unit) {
        app.files.collect { file ->
            made = file
            save.launch(file.name)
        }
    }
    val import = rememberImport(core)
    val page = SettingsPages.of("export")
    ScreenFrame(page?.title.orEmpty(), core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            view?.settings?.exports.orEmpty().forEach { export ->
                OutlinedButton(modifier = Target, onClick = { app.send(ShellEvent.Settings(SettingsEvent.Export(export.format))) }) {
                    Text("${export.label}…")
                }
            }
            // The page's own buttons, the core's: Import or restore, which asks for a file.
            page?.buttons.orEmpty().forEach { button ->
                OutlinedButton(modifier = Target, onClick = import) { Text(if (button.asks) "${button.title}…" else button.title) }
            }
            page?.caption?.takeIf { it.isNotEmpty() }?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = quiet()) }
            // What is done to the store as a whole, the core's (Erase everything deleted): the
            // app asks its question as any row's, at the root.
            view?.settings?.offered.orEmpty().forEach { action ->
                OutlinedButton(modifier = Target, onClick = { app.send(ShellEvent.ActOn(ActingOn.Settings, action)) }) {
                    Text(action.name, color = if (action.destructive) MaterialTheme.colorScheme.error else androidx.compose.ui.graphics.Color.Unspecified)
                }
            }
            // What they take, the core's words, as every app says it.
            view?.settings?.offeredCaption?.takeIf { it.isNotEmpty() }?.let { caption ->
                Text(caption, style = MaterialTheme.typography.bodySmall, color = quiet())
            }
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
    // The page's buttons, the core's: Pair a device in the bar, Sync now beneath the status.
    val page = SettingsPages.of("devices")
    val pair = page?.buttons?.firstOrNull { it.id == "pair" }
    val syncing = page?.buttons?.firstOrNull { it.id == "sync-now" }
    ScreenFrame(page?.title.orEmpty(), core, navigator, actions = {
        IconButton(onClick = { navigator.push(Screen.Pairing) }) { Icon(Icons.Filled.Add, contentDescription = pair?.title) }
    }) {
        Text(
            devices?.status.orEmpty(),
            style = MaterialTheme.typography.bodyMedium,
            color = quiet(),
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
        )
        // Syncing is with every device at once, so it is the screen's, not a row's.
        OutlinedButton(modifier = Target.padding(horizontal = 16.dp), onClick = { core.app.syncNow() }) { Text(syncing?.title ?: Command.SYNC_NOW.title) }
        val list = devices?.list
        val rows = list?.devices.orEmpty()
        val state = rememberLazyListState()
        val focus = rememberSideLanding(core, Side.DEVICES, rows.map { it.nodeId }, state)
        val lines by app.lines.collectAsState()
        if (rows.isEmpty() && list != null) {
            Text(list.empty, Modifier.padding(16.dp), color = quiet())
        }
        LazyColumn(Modifier.fillMaxSize().semantics { collectionInfo = CollectionInfo(rows.size, 1) }, state = state) {
            itemsIndexed(rows, key = { _, device -> device.nodeId }) { index, device ->
                // How syncing with it is going, as the core words it for every app.
                val line = lines?.devices?.getOrNull(index)
                val detail = line?.detail.orEmpty()
                ListRow(
                    title = line?.title ?: device.name,
                    detail = detail,
                    speech = detail,
                    index = index,
                    // The core's: Rename, and Unpair on every row but this device's own.
                    actions = app.rowActions(device.nodeId, device.offered, side = Side.DEVICES),
                    keys = app.rowKeys(device.nodeId, Side.DEVICES),
                    open = null,
                    openLabel = "Open",
                    focus = focus.requester(device.nodeId),
                    key = device.nodeId,
                    say = core::say,
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
    val name = remember { deviceName(context) }
    LaunchedEffect(Unit) {
        // What pairing's sentences call this phone, which finds others on its network.
        app.send(ShellEvent.PairingAs(name, true))
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
    // Pairing's words come with the Devices page, in this phone's capitals.
    val words = view?.devices?.words ?: return
    var entered by remember { mutableStateOf("") }

    ScreenFrame(words.title, core, navigator) {
        if (pairing == null) return@ScreenFrame
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            val status = pairing.problem ?: when {
                pairing.mine != null -> words.waiting
                pairing.running -> words.connecting
                else -> words.introSentence.ifEmpty { words.intro }
            }
            Text(status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
            Button(modifier = Target, onClick = { pair(PairEvent.Wait) }, enabled = !pairing.running) { Text(words.wait) }
            pairing.mine?.let { code ->
                // The core's line for it: "This device's code: 1234-5678".
                Text(pairing.mineLine ?: code, fontFamily = FontFamily.Monospace)
                OutlinedButton(modifier = Target, onClick = {
                    val clipboard = context.getSystemService(android.content.Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                    // Marked sensitive, so the system's copied preview and keyboard suggestions do
                    // not show it: whoever has the code can pair while it waits.
                    val clip = android.content.ClipData.newPlainText(words.myCode, code).apply {
                        description.extras = android.os.PersistableBundle().apply {
                            putBoolean(if (Build.VERSION.SDK_INT >= 33) android.content.ClipDescription.EXTRA_IS_SENSITIVE else "android.content.extra.IS_SENSITIVE", true)
                        }
                    }
                    clipboard.setPrimaryClip(clip)
                    core.say(words.copied)
                }) { Text(words.copyCode) }
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
            Button(modifier = Target, onClick = { pair(PairEvent.Join) }) { Text(words.join) }
        }
        pairing.words?.let { three ->
            AlertDialog(
                onDismissRequest = {},
                title = { Text(words.matchTitle) },
                text = { Text(pairing.matching ?: three.joinToString(", ")) },
                confirmButton = { TextButton(modifier = Target, onClick = { pair(PairEvent.Confirm(true)) }) { Text(words.matchYes) } },
                dismissButton = { TextButton(modifier = Target, onClick = { pair(PairEvent.Confirm(false)) }) { Text(words.matchNo) } },
            )
        }
    }
}
