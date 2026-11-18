package com.extra.han.lumenna

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import android.provider.Settings
import android.view.KeyEvent
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import com.extra.han.lumenna.ui.Host
import com.extra.han.lumenna.ui.Shell
import dev.emassey0135.braillesense.edit.SystemClipboard
import dev.emassey0135.braillesense.files.FilePicker
import dev.emassey0135.braillesense.files.FileRequest
import dev.emassey0135.braillesense.files.PickKind
import dev.emassey0135.braillesense.files.PickedFile
import dev.emassey0135.braillesense.session.BrailleSenseSession
import dev.emassey0135.braillesense.ui.SettingsReadings
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Front
import dev.emassey0135.lumenna.Launching
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.ReminderOpened
import dev.emassey0135.lumenna.SyncWorker
import dev.emassey0135.lumenna.app.SaveFile
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.storeUnopened
import dev.emassey0135.lumenna.Reminded
import dev.emassey0135.lumenna.replan
import dev.emassey0135.lumenna.deviceName
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * The one activity: the device's braille keys and QWERTY keyboard in, speech and cells out, over
 * the Crux app, through the BrailleSense SDK's session. Everything shown on the screen is for a
 * sighted helper; the person reads the cells and listens. Taking the keys and the display is what
 * `resumed`/`paused` do, as HIMS' own apps take them.
 */
class MainActivity : ComponentActivity(), Host {
    private val application get() = super.getApplication() as BrailleSenseApplication
    private val core get() = application.core

    private val device by lazy { BrailleSenseSession(this, BuildConfig.VERSION_NAME, trace = BuildConfig.DEBUG, tag = "LumennaOutput") }
    private var shell: Shell? = null
    private val main = Handler(Looper.getMainLooper())
    private lateinit var screen: TextView

    /** Coming to the front and leaving it, as far as syncing and the store are concerned. */
    private val syncing by lazy { core.getOrNull()?.let { Front(this, it, PLATFORM) } }

    // What a file chosen in the File Manager goes to, and the folder the last one was in.
    private var picking: ((PickedFile) -> Unit)? = null
    private var folder: String? = null
    private val picker = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val then = picking
        picking = null
        val file = FilePicker.picked(result.resultCode, result.data) ?: return@registerForActivityResult
        folder = file.parent
        then?.invoke(file)
    }

    // Android's own picker, only where the File Manager cannot be started.
    private var saving: SaveFile? = null

    private val save = registerForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
        val file = saving ?: return@registerForActivityResult
        saving = null
        if (uri == null) return@registerForActivityResult
        lifecycleScope.launch(Dispatchers.IO) { runCatching { contentResolver.openOutputStream(uri)?.use { it.write(file.bytes) } } }
    }

    private val open = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri == null) return@registerForActivityResult
        lifecycleScope.launch(Dispatchers.IO) {
            val name = runCatching {
                contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                    ?.use { if (it.moveToFirst()) it.getString(0) else null }
            }.getOrNull() ?: "import"
            val bytes = runCatching { contentResolver.openInputStream(uri)?.use { it.readBytes() } }.getOrNull() ?: return@launch
            core.getOrNull()?.app?.send(ShellEvent.Settings(SettingsEvent.Import(name, bytes)))
        }
    }

    private val permission = registerForActivityResult(ActivityResultContracts.RequestPermission()) { }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        screen = TextView(this).apply {
            textSize = 28f
            setPadding(32, 32, 32, 32)
        }
        setContentView(screen)
        device.onKey = { shell?.press(it) }
        device.onTaken = {
            shell?.front()
            main.post(looking)
        }
        device.onReleased = { main.removeCallbacks(looking) }
        // A box's text is read in the table it was typed in before the table can change (the G
        // chord, or Global Options while away), and written again in the new one after.
        device.beforeSettings = { shell?.settle() }
        device.onTableChanged = { shell?.retable() }
        device.onSettingsChanged = { shell?.redraw() }
        device.created()
        val opened = core.getOrElse { error ->
            device.display.say(storeUnopened(error.message.orEmpty()))
            screen.text = storeUnopened(error.message.orEmpty())
            return
        }
        Clock.update(this)
        val settings = device.settings
        val shown = Shell(
            opened, device.display, device.translator, this, SettingsReadings(settings),
            SystemClipboard(this, applicationInfo.loadLabel(packageManager)),
            messageMillis = { settings.messageMillis }, opensOnLetter = { settings.opensOnLetter }, echoSetting = { settings.echo },
        ).apply {
            reminderAccess = { Reminders.access(this@MainActivity) }
        }
        shell = shown
        syncing?.created()
        shown.drawn = { screen.text = it }
        lifecycleScope.launch { opened.announcements.collect { shown.announce(it) } }
        lifecycleScope.launch { opened.app.files.collect(::saveFile) }
        handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    private fun handle(intent: Intent?) {
        val app = core.getOrNull()?.app ?: return
        when (val reminded = Reminders.opened(this, intent)?.reminded) {
            is Reminded.Task -> shell?.openTask(reminded.id)
            is Reminded.Block -> reminded.on?.let { app.send(ShellEvent.ShowBlock(it.series, it.date)) }
            is Reminded.Replan -> app.replan(reminded)
            null -> {}
        }
        Launching.id(intent)?.let { app.send(ShellEvent.Launch(it)) }
    }

    override fun onResume() {
        super.onResume()
        device.resumed()
        Clock.update(this)
        syncing?.resumed()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        device.windowFocus(hasFocus)
    }

    /** What is due is looked for while in front; a reminder away from it comes as a notification. */
    private val looking = object : Runnable {
        override fun run() {
            shell?.look()
            main.postDelayed(this, 15_000)
        }
    }

    override fun onPause() {
        // First of all: whatever else leaving does, the device is the next app's.
        device.paused()
        syncing?.paused() ?: SyncWorker.syncOnLeaving(this)
        super.onPause()
    }

    override fun onDestroy() {
        device.destroyed()
        super.onDestroy()
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean = device.keyEvent(event) || super.dispatchKeyEvent(event)

    override fun exit() = finishAffinity()

    // ---- files: chosen in the device's own File Manager (the SDK's `FilePicker`), which reads itself
    // on the cells and in speech; Android's picker only where it cannot be started.

    /** Asks the File Manager for [request], then hands what was chosen to [then]; false where it cannot be asked. */
    private fun pick(request: FileRequest, then: (PickedFile) -> Unit): Boolean {
        picking = then
        val started = runCatching { FilePicker.launch(request) { picker.launch(it) } }.getOrDefault(false)
        if (!started) picking = null
        return started
    }

    /** Each file the app makes is saved where the person chooses, written off the main thread. */
    private fun saveFile(file: SaveFile) {
        val chosen = pick(FileRequest(PickKind.SAVE_FILE, suggestedName = file.name, startIn = folder)) { to ->
            lifecycleScope.launch(Dispatchers.IO) { runCatching { to.openOutput(this@MainActivity).use { it.write(file.bytes) } } }
        }
        if (!chosen) {
            saving = file
            save.launch(file.name)
        }
    }

    override fun importFile() {
        val chosen = pick(FileRequest(PickKind.OPEN_FILE, startIn = folder)) { file ->
            lifecycleScope.launch(Dispatchers.IO) {
                val bytes = runCatching { file.openInput(this@MainActivity).use { it.readBytes() } }.getOrNull() ?: return@launch
                core.getOrNull()?.app?.send(ShellEvent.Settings(SettingsEvent.Import(file.name, bytes)))
            }
        }
        if (!chosen) open.launch(arrayOf("*/*"))
    }

    override fun chooseFolder(key: String, current: String) {
        pick(FileRequest(PickKind.OPEN_FOLDER, startIn = current.ifEmpty { folder })) { chosen ->
            core.getOrNull()?.app?.send(ShellEvent.Settings(SettingsEvent.Set(key, chosen.path)))
        }
    }

    override fun requestNotifications() {
        if (Build.VERSION.SDK_INT >= 33) {
            permission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    override fun openNotificationSettings() {
        startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, packageName))
    }

    override fun deviceName(): String = deviceName(this)

    companion object {
        const val PLATFORM = "android"
    }
}
