package io.github.emassey0135.lumenna

import android.content.ClipData
import android.content.ClipboardManager
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
import androidx.compose.foundation.selection.toggleable
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
import io.github.emassey0135.lumenna.core.ExportFormat
import io.github.emassey0135.lumenna.core.Imported
import io.github.emassey0135.lumenna.core.Setting
import io.github.emassey0135.lumenna.core.SettingKind
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.PairedWith
import io.github.emassey0135.lumenna.core.PairingPrompt
import io.github.emassey0135.lumenna.core.Reach
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * Settings: a short list of pages — what syncs, what is this device's alone, and getting
 * data in and out — so backups are reached without swiping past every weekday.
 */
@Composable
fun SettingsScreen(core: Core, navigator: Navigator, changes: Long) {
    ItemListScreen(
        "Settings", core, navigator, changes = changes,
        load = {
            listOf(
                Item("devices", "Devices and Sync", core.lumenna.syncStatus().announcement),
                Item("planning", "Planning", "The day, the week, completing subtasks, announcements"),
                Item("backups", "Backups", "On this device only"),
                Item("export", "Export and Import", "JSON, Markdown, org, calendar"),
            ) to ""
        },
        open = { item ->
            navigator.push(
                when (item.key) {
                    "devices" -> Screen.Devices
                    "planning" -> Screen.Planning
                    "backups" -> Screen.Backups
                    else -> Screen.Export
                },
            )
        },
    )
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

/** A choice among a few, asked as a list of buttons. */
@Composable
private fun ChooseOne(title: String, choices: List<Pair<String, String>>, dismiss: () -> Unit, chosen: (String) -> Unit) {
    AlertDialog(
        onDismissRequest = dismiss,
        title = { Text(title) },
        text = {
            Column {
                choices.forEach { (name, value) ->
                    TextButton(modifier = Target.fillMaxWidth(), onClick = { chosen(value) }) { Text(name) }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(modifier = Target, onClick = dismiss) { Text("Cancel") } },
    )
}

/**
 * One setting, by the control its kind asks for: a switch, a choice among its options, a time
 * or a number typed. Its name, options and what it does are the core's.
 */
@Composable
private fun SettingControl(core: Core, prompt: Prompter, setting: Setting) {
    when (setting.kind) {
        SettingKind.TOGGLE -> Column(Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            Row(
                Modifier
                    .fillMaxWidth()
                    .toggleable(setting.on, role = Role.Switch) { core.set(setting, if (it) "true" else "false") },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(setting.title, Modifier.weight(1f))
                Switch(checked = setting.on, onCheckedChange = null)
            }
            if (setting.hint.isNotEmpty()) Text(setting.hint, style = MaterialTheme.typography.bodySmall, color = quiet())
        }
        SettingKind.CHOICE -> SettingRow(setting.title, setting.said, setting.hint) {
            prompt.show {
                ChooseOne(setting.title, setting.options.map { it.title to it.id }, prompt::close) {
                    prompt.close()
                    core.set(setting, it)
                }
            }
        }
        SettingKind.TIME, SettingKind.NUMBER, SettingKind.FOLDER -> SettingRow(setting.title, setting.said, setting.hint) {
            val time = setting.kind == SettingKind.TIME
            prompt.show {
                AskText(
                    setting.title, if (time) "Time" else setting.title, "Set",
                    initial = setting.value,
                    example = if (time) "9am" else "",
                    hint = if (time) "Such as 8am or 21:30." else null,
                    number = setting.kind == SettingKind.NUMBER,
                    dismiss = prompt::close,
                ) {
                    // A refused value stays in its dialog, and the core says why.
                    if (core.set(setting, it.trim()) != null) prompt.close()
                }
            }
        }
    }
}

/** What syncs to every device: the core's settings that sync, in its order. */
@Composable
fun PlanningScreen(core: Core, navigator: Navigator, changes: Long) {
    val shown = remember(changes) { settings(core).planning() }
    val prompt = rememberPrompter()
    ScreenFrame("Planning", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { SettingControl(core, prompt, it) }
            Text(
                "These sync to all your devices.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
                modifier = Modifier.padding(16.dp),
            )
        }
    }
    prompt.Host()
}

/** Backups, which are this device's alone. */
@Composable
fun BackupsScreen(core: Core, navigator: Navigator, changes: Long) {
    val shown = remember(changes) { settings(core).backups() }
    val prompt = rememberPrompter()
    ScreenFrame("Backups", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            shown.forEach { SettingControl(core, prompt, it) }
            Button(
                modifier = Target.padding(16.dp),
                onClick = { core.attempt { core.lumenna.backup(null) }?.let { core.say(sentence(it.announcement, it.notices)) } },
            ) { Text("Back Up Now") }
            Text(
                "These settings are this device's own. Restore a backup from Export and Import.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
                modifier = Modifier.padding(horizontal = 16.dp),
            )
        }
    }
    prompt.Host()
}

/**
 * Getting data out and back in, through Android's own file picker, so an export can go to
 * Drive or anywhere else a provider offers. An export is the present state, nothing from the
 * trash; importing one, or restoring a backup, adds what this device lacks and removes nothing.
 */
@Composable
fun ExportScreen(core: Core, navigator: Navigator) {
    val context = LocalContext.current
    var pending by remember { mutableStateOf<ExportFormat?>(null) }
    val save = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
        val format = pending ?: return@rememberLauncherForActivityResult
        pending = null
        if (uri == null) return@rememberLauncherForActivityResult
        core.attempt { core.lumenna.export(format, null, false) }?.let { done ->
            context.contentResolver.openOutputStream(uri)?.use { it.write(done.content.orEmpty().toByteArray()) }
            core.say(sentence(done.announcement, done.notices))
        }
    }
    val open = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        // The core reads a path; a document from a provider is copied to one first.
        val copy = File(context.cacheDir, "import")
        context.contentResolver.openInputStream(uri)?.use { input -> copy.outputStream().use { input.copyTo(it) } }
        core.attempt { core.lumenna.import(copy.absolutePath) }?.let { result ->
            core.changed()
            core.say(
                when (result) {
                    is Imported.Export -> sentence(result.done.announcement, result.done.notices)
                    is Imported.Backup -> sentence(result.done.announcement, result.done.notices)
                },
            )
        }
        copy.delete()
    }
    fun export(format: ExportFormat, extension: String) {
        pending = format
        save.launch("Lumenna ${Clock.today()}.$extension")
    }
    ScreenFrame("Export and Import", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(modifier = Target, onClick = { export(ExportFormat.JSON, "json") }) { Text("Export JSON, Complete, Can Be Imported…") }
            OutlinedButton(modifier = Target, onClick = { export(ExportFormat.MARKDOWN, "md") }) { Text("Export a Markdown Checklist…") }
            OutlinedButton(modifier = Target, onClick = { export(ExportFormat.ORG, "org") }) { Text("Export an Org Outline…") }
            OutlinedButton(modifier = Target, onClick = { export(ExportFormat.ICS, "ics") }) { Text("Export a Calendar File of Your Blocks…") }
            OutlinedButton(modifier = Target, onClick = { open.launch(arrayOf("*/*")) }) { Text("Import or Restore…") }
            Text(
                "An export is what you have now, with nothing from the trash. Importing a JSON export or restoring a " +
                    "backup adds what this device lacks and removes nothing.",
                style = MaterialTheme.typography.bodySmall,
                color = quiet(),
            )
        }
    }
}

/** This phone's name, as the other devices will list it. */

/** The paired devices, how syncing with each last went, and pairing another. */
@Composable
fun DevicesScreen(core: Core, navigator: Navigator, changes: Long) {
    val prompt = rememberPrompter()
    ItemListScreen(
        "Devices and Sync", core, navigator, changes = changes,
        load = {
            val status = core.lumenna.syncStatus()
            status.devices.map { device ->
                // How syncing with it is going, as the core words it for every app.
                val detail = (listOf(device.platform) + device.status).joinToString(", ")
                Item(device.nodeId, device.name, detail, actions = device.actions, thisDevice = device.thisDevice)
            } to status.announcement
        },
        addLabel = "Pair a device",
        add = { navigator.push(Screen.Pairing) },
        open = null,
        // The core's: Rename, and Unpair on every row but this device's own.
        actions = { item -> core.offered(item.actions, prompt) },
        // Syncing is with every device at once, so it is the screen's, not a row's.
        header = {
            OutlinedButton(modifier = Target.padding(horizontal = 16.dp), onClick = { syncNow(core) }) { Text("Sync Now") }
        },
    )
    prompt.Host()
}

/**
 * Pairing this phone with another of the person's devices. On one network the two find
 * each other; anywhere else, one shows a code and the other enters it. Either way both show the
 * same three words, and nothing is paired unless the person says they match on both.
 */
@Composable
fun PairingScreen(core: Core, navigator: Navigator) {
    val context = LocalContext.current
    // What pairing does is shared with the watch (`PairingSession`); this shows it.
    val session = remember { PairingSession(core, context) { navigator.back() } }
    var entered by remember { mutableStateOf("") }

    // Leaving the screen gives up, which ends the wait for the other device.
    DisposableEffect(Unit) { onDispose { session.cancel() } }

    ScreenFrame("Pair a Device", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(session.status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
            Button(modifier = Target, onClick = { session.waitToBeFound() }, enabled = !session.running) { Text("Wait for the Other Device") }
            session.code?.let {
                Text("Pairing code: $it", fontFamily = FontFamily.Monospace)
            }
            OutlinedTextField(
                value = entered,
                onValueChange = { entered = it },
                label = { Text("Code from the other device") },
                supportingText = { Text("Left empty, the code on the clipboard is used.") },
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false),
                modifier = Modifier.fillMaxWidth(),
            )
            Button(
                modifier = Target,
                onClick = { entered = session.join(entered) },
                enabled = session.nextCode == null && (!session.running || session.waiting),
            ) { Text("Pair With This Code") }
        }
    }

    session.asked?.let { shown ->
        AlertDialog(
            onDismissRequest = {},
            title = { Text("Do these words match?") },
            text = { Text(PairingSession.matchQuestion(shown)) },
            confirmButton = { TextButton(modifier = Target, onClick = { session.answer(true) }) { Text("Yes, They Match") } },
            dismissButton = { TextButton(modifier = Target, onClick = { session.answer(false) }) { Text("No") } },
        )
    }
}
