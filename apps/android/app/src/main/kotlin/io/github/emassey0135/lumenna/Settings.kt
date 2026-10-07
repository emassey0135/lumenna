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

/** Every setting, by key. */
private fun settings(core: Core): Map<String, String> =
    core.attempt { core.lumenna.settings(null).settings.associate { it.key to it.value } }.orEmpty()

/** Changes a setting, saying what happened. */
private fun set(core: Core, key: String, value: String) {
    core.change { it.setSetting(key, value) }
}

/** A setting's name and its value, opening whatever changes it. */
@Composable
private fun SettingRow(name: String, value: String, change: () -> Unit) {
    Column(
        Modifier
            .fillMaxWidth()
            .clickable(role = Role.Button, onClick = change)
            .semantics(mergeDescendants = true) {}
            .padding(horizontal = 16.dp, vertical = 12.dp),
    ) {
        Text(name, style = MaterialTheme.typography.bodyLarge)
        Text(value, style = MaterialTheme.typography.bodyMedium, color = quiet())
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

private val weekdays = listOf("monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday")

/** What syncs to every device: the day, the week, subtasks, how much is said. */
@Composable
fun PlanningScreen(core: Core, navigator: Navigator, changes: Long) {
    val values = remember(changes) { settings(core) }
    val prompt = rememberPrompter()
    fun time(key: String, name: String) {
        prompt.show {
            AskText(name, "Time", "Set", initial = values[key].orEmpty(), example = "9am", hint = "Such as 8am or 21:30.", dismiss = prompt::close) {
                prompt.close()
                set(core, key, it.trim())
            }
        }
    }
    ScreenFrame("Planning", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            val cascade = values["cascade-complete-subtasks"] == "true"
            Row(
                Modifier
                    .fillMaxWidth()
                    .toggleable(cascade, role = Role.Switch) { set(core, "cascade-complete-subtasks", if (it) "true" else "false") }
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("Completing a task completes its subtasks", Modifier.weight(1f))
                Switch(checked = cascade, onCheckedChange = null)
            }
            SettingRow("Day starts", values["day-start"].orEmpty().let(Clock::time)) { time("day-start", "Day starts") }
            SettingRow("Day ends", values["day-end"].orEmpty().let(Clock::time)) { time("day-end", "Day ends") }
            SettingRow("All-day reminders at", values["all-day-reminder-hour"].orEmpty().let(Clock::time)) {
                time("all-day-reminder-hour", "All-day reminders at")
            }
            SettingRow("Announcements", if (values["verbosity"] == "terse") "Terse" else "Full sentences") {
                prompt.show {
                    ChooseOne("Announcements", listOf("Full sentences" to "full", "Terse" to "terse"), prompt::close) {
                        prompt.close()
                        set(core, "verbosity", it)
                    }
                }
            }
            SettingRow("Week starts on", values["week-start"].orEmpty().replaceFirstChar { it.uppercase() }) {
                prompt.show {
                    ChooseOne("Week starts on", weekdays.map { it.replaceFirstChar { c -> c.uppercase() } to it }, prompt::close) {
                        prompt.close()
                        set(core, "week-start", it)
                    }
                }
            }
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
    val values = remember(changes) { settings(core) }
    val prompt = rememberPrompter()
    val every = mapOf("12h" to "Every 12 hours", "1d" to "Every day", "7d" to "Every week", "off" to "Off")
    ScreenFrame("Backups", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState())) {
            SettingRow("Automatic backups", every[values["backup-every"]] ?: values["backup-every"].orEmpty()) {
                prompt.show {
                    ChooseOne("Automatic backups", every.map { (value, name) -> name to value }, prompt::close) {
                        prompt.close()
                        set(core, "backup-every", it)
                    }
                }
            }
            SettingRow("Backups kept", values["backup-keep"].orEmpty()) {
                prompt.show {
                    AskText("Backups kept", "How many", "Set", initial = values["backup-keep"].orEmpty(), number = true, hint = "The oldest beyond this are removed.", dismiss = prompt::close) {
                        prompt.close()
                        set(core, "backup-keep", it.trim())
                    }
                }
            }
            Button(
                modifier = Target.padding(16.dp),
                onClick = { core.attempt { core.lumenna.backup(null) }?.let { core.say(sentence(it.announcement, it.notices)) } },
            ) { Text("Back Up Now") }
            Text(
                "A backup holds your whole history, including every task you deleted, so the store can be rebuilt " +
                    "from it. It stays on this device, as these settings do. Restore one from Export and Import.",
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
private fun deviceName(context: Context): String =
    android.provider.Settings.Global.getString(context.contentResolver, android.provider.Settings.Global.DEVICE_NAME)
        ?: Build.MODEL

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
                Item(device.nodeId, device.name, detail)
            } to status.announcement
        },
        addLabel = "Pair a device",
        add = { navigator.push(Screen.Pairing) },
        open = null,
        actions = { item ->
            // This device cannot unpair itself, so that is not offered on its own row.
            val self = item.detail.contains("this device")
            listOfNotNull(
                RowAction("Sync Now") {
                    core.say("Syncing")
                    core.syncNow { result ->
                        result.fold(
                            { core.changed(); core.say(sentence(it.announcement, it.notices)) },
                            { core.say((it as? LumennaException)?.sentence ?: it.message.orEmpty()) },
                        )
                    }
                },
                RowAction("Rename") {
                    prompt.show {
                        AskText("Rename ${item.title}", "Name", "Rename", initial = item.title, dismiss = prompt::close) { name ->
                            prompt.close()
                            core.change { it.renameDevice(item.key, name.trim()) }
                        }
                    }
                },
                if (self) null else RowAction("Stop Syncing With It") {
                    prompt.show {
                        Confirm(
                            "Stop syncing with ${item.title}?",
                            "It keeps what it already has: this is for a device you replaced, not one that was stolen.",
                            "Stop Syncing",
                            prompt::close,
                        ) {
                            prompt.close()
                            core.change { it.unpairDevice(item.key) }
                        }
                    }
                },
            )
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
    var status by remember {
        mutableStateOf(
            "On the same network, start pairing on both devices and they find each other. " +
                "On different networks, one shows a code and the other enters it.",
        )
    }
    var code by remember { mutableStateOf<String?>(null) }
    var entered by remember { mutableStateOf("") }
    var asked by remember { mutableStateOf<Pair<List<String>, (Boolean) -> Unit>?>(null) }
    val cancelled = remember { AtomicBoolean(false) }
    var running by remember { mutableStateOf(false) }
    // A code entered while this phone waits to be found: joined with once the wait has ended.
    var nextCode by remember { mutableStateOf<String?>(null) }
    var waiting by remember { mutableStateOf(false) }

    // Leaving the screen gives up, which ends the wait for the other device.
    DisposableEffect(Unit) { onDispose { cancelled.set(true) } }

    fun start(given: String?) {
        if (running) return
        running = true
        waiting = given == null
        cancelled.set(false)
        status = if (given == null) "Opening a pairing session." else "Connecting to the other device."
        core.say(status)
        val main = android.os.Handler(android.os.Looper.getMainLooper())
        // The pairing thread calls these and waits; the screen answers on the main thread.
        val showCode: (String) -> Unit = { shown ->
            code = shown
            (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                .setPrimaryClip(ClipData.newPlainText("Pairing code", shown))
            status = "Waiting for the other device. On this network it finds this one by itself. On another " +
                "network, enter this code there. Waiting up to ten minutes. The code is copied, so it can be pasted."
            core.say(status)
        }
        val prompt = object : PairingPrompt {
            override fun showCode(code: String) {
                main.post { showCode(code) }
            }

            override fun confirm(words: List<String>): Boolean {
                val answered = CountDownLatch(1)
                var matched = false
                main.post {
                    asked = words to { yes: Boolean ->
                        matched = yes
                        answered.countDown()
                    }
                }
                answered.await()
                return matched
            }

            override fun isCancelled(): Boolean = cancelled.get()
        }
        val wifi = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        thread(name = "lumenna-pairing") {
            // Local discovery hears multicast only while the app holds this lock.
            val lock = wifi.createMulticastLock("lumenna-pairing").apply { setReferenceCounted(false); acquire() }
            val result = runCatching { core.lumenna.pair(given, Reach.INTERNET, deviceName(context), "android", prompt) }
            lock.release()
            main.post {
                running = false
                asked = null
                nextCode?.let { next ->
                    nextCode = null
                    code = null
                    start(next)
                    return@post
                }
                result.fold(
                    { paired: PairedWith ->
                        core.changed()
                        core.say(sentence(paired.announcement, paired.notices))
                        navigator.back()
                    },
                    { error ->
                        code = null
                        status = (error as? LumennaException)?.sentence ?: error.message.orEmpty()
                        core.say(status)
                    },
                )
            }
        }
    }

    ScreenFrame("Pair a Device", core, navigator) {
        Column(Modifier.verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Text(status, Modifier.semantics { liveRegion = LiveRegionMode.Polite })
            Button(modifier = Target, onClick = { start(null) }, enabled = !running) { Text("Wait for the Other Device") }
            code?.let {
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
            Button(modifier = Target, onClick = {
                var given = entered.trim()
                if (given.isEmpty()) {
                    given = (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                        .primaryClip?.getItemAt(0)?.text?.toString()?.trim().orEmpty()
                    // This phone's own code, copied while it waits, is never the other device's.
                    if (given == code) given = ""
                    entered = given
                }
                when {
                    given.isEmpty() -> core.say("Type or paste the code the other device shows.")
                    // Entering a code while waiting means choosing the other way: give up the
                    // wait, and join with the code once it has ended.
                    running -> {
                        nextCode = given
                        status = "Stopping the wait, then connecting with this code."
                        core.say(status)
                        cancelled.set(true)
                    }
                    else -> start(given)
                }
            }, enabled = nextCode == null && (!running || waiting)) { Text("Pair With This Code") }
        }
    }

    asked?.let { (shown, reply) ->
        AlertDialog(
            onDismissRequest = {},
            title = { Text("Do these words match?") },
            text = { Text("${shown.joinToString(", ")}. Say yes only if the other device shows the same three words.") },
            confirmButton = { TextButton(modifier = Target, onClick = { asked = null; reply(true) }) { Text("Yes, They Match") } },
            dismissButton = { TextButton(modifier = Target, onClick = { asked = null; reply(false) }) { Text("No") } },
        )
    }
}
