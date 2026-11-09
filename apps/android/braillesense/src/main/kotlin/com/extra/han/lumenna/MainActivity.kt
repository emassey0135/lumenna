package com.extra.han.lumenna

import android.Manifest
import android.content.Intent
import android.database.ContentObserver
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
import com.extra.han.lumenna.device.BrailleSettings
import com.extra.han.lumenna.device.Chords
import com.extra.han.lumenna.device.HanInputClient
import com.extra.han.lumenna.device.HanOutputClient
import com.extra.han.lumenna.device.Key
import com.extra.han.lumenna.device.Louis
import com.extra.han.lumenna.device.Qwerty
import com.extra.han.lumenna.ui.CoreReadings
import com.extra.han.lumenna.ui.Host
import com.extra.han.lumenna.ui.Shell
import com.extra.han.lumenna.ui.Speaker
import dev.emassey0135.lumenna.braille.Claim
import dev.emassey0135.lumenna.braille.KeyDelivery
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
 * the Crux app. Everything shown on the screen is for a sighted helper; the person reads the
 * cells and listens. Taking the keys and the display is what `resumed`/`paused` do, as HIMS' own
 * apps take them.
 */
class MainActivity : ComponentActivity(), Host {
    private val application get() = super.getApplication() as BrailleSenseApplication
    private val core get() = application.core

    private val output by lazy { HanOutputClient(applicationContext) }
    private val settings by lazy { BrailleSettings(applicationContext) }
    private val translator by lazy { Louis.translator { settings.tables } }
    private val speaker by lazy { Speaker(output, translator) }
    private var shell: Shell? = null
    private val main = Handler(Looper.getMainLooper())
    private lateinit var screen: TextView

    private val keys: KeyDelivery<Key> by lazy { KeyDelivery(front, { main.post(it) }) { shell?.press(it) } }

    private val input: HanInputClient by lazy {
        HanInputClient(applicationContext) { code, value, data ->
            keys.deliver(Chords.decode(code, value, data, settings.eightDot))
        }
    }

    /** Coming to the front and leaving it, as far as syncing and the store are concerned. */
    private val syncing by lazy { core.getOrNull()?.let { Front(this, it, PLATFORM) } }

    /** Whether the keys and display are ours: resumed and focused, and nothing else. */
    private val front: Claim = Claim(
        take = {
            settings.refresh()
            output.claim()
            input.claim()
            shell?.front()
            main.post(looking)
        },
        release = {
            main.removeCallbacks(looking)
            input.release()
            output.release()
        },
    )
    private var saving: SaveFile? = null

    /**
     * A global option changed, perhaps by its own hot key with the app in front (F2 with the Down
     * scroll key is Control Information), which no focus change tells the app about.
     */
    private val settingsChanged = object : ContentObserver(main) {
        override fun onChange(selfChange: Boolean) {
            settings.refresh()
            if (front.holding) shell?.redraw()
        }
    }

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
        front.guardCrashes()
        screen = TextView(this).apply {
            textSize = 28f
            setPadding(32, 32, 32, 32)
        }
        setContentView(screen)
        settings.refresh()
        contentResolver.registerContentObserver(BrailleSettings.URI, true, settingsChanged)
        output.connect()
        lifecycleScope.launch(Dispatchers.IO) { Louis.install(applicationContext, BuildConfig.VERSION_NAME) }
        val opened = core.getOrElse { error ->
            speaker.say(storeUnopened(error.message.orEmpty()))
            screen.text = storeUnopened(error.message.orEmpty())
            return
        }
        Clock.update(this)
        val shown = Shell(opened, speaker, translator, this, CoreReadings(settings), messageMillis = { settings.messageMillis }, opensOnLetter = { settings.opensOnLetter }).apply {
            reminderAccess = { Reminders.access(this@MainActivity) }
        }
        shell = shown
        syncing?.created()
        shown.drawn = { screen.text = it }
        lifecycleScope.launch { opened.announcements.collect { shown.announce(it) } }
        lifecycleScope.launch { opened.app.files.collect { file ->
            saving = file
            save.launch(file.name)
        } }
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
        front.resumed()
        Clock.update(this)
        syncing?.resumed()
    }

    /**
     * What is drawn over the app without pausing it (the task list, the global options, an alert)
     * takes the window's focus and nothing else: the keys have to go with it.
     */
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        front.windowFocus(hasFocus)
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
        front.paused()
        syncing?.paused() ?: SyncWorker.syncOnLeaving(this)
        super.onPause()
    }

    override fun onDestroy() {
        contentResolver.unregisterContentObserver(settingsChanged)
        input.release()
        output.disconnect()
        super.onDestroy()
    }

    /**
     * The QWERTY keyboard, which reaches the app as ordinary key events. Keys the braille
     * keyboard's service did not take arrive here too, with no device behind them and the
     * chord as their scan code: those are not typing, and are swallowed.
     */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (!front.holding) return super.dispatchKeyEvent(event)
        if (event.deviceId == -1 && event.scanCode != 0) return true
        if (event.action != KeyEvent.ACTION_DOWN) return super.dispatchKeyEvent(event)
        val key = Qwerty.decode(event.keyCode, event.metaState, event.getUnicodeChar(event.metaState))
        if (key != null) {
            shell?.press(key)
            return true
        }
        return super.dispatchKeyEvent(event)
    }

    override fun exit() = finishAffinity()

    override fun importFile() = open.launch(arrayOf("*/*"))

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
