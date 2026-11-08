package dev.emassey0135.lumenna.orbit

import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import android.util.TypedValue
import android.view.Gravity
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import dev.emassey0135.lumenna.braille.KeyDelivery
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.Front
import dev.emassey0135.lumenna.Reminders
import dev.emassey0135.lumenna.app.SaveFile
import dev.emassey0135.lumenna.app.SettingsEvent
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.ThisDevice
import dev.emassey0135.lumenna.appWords
import dev.emassey0135.lumenna.thisDevice
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The one activity: a text view showing the current line, for a sighted helper, while Orbit's
 * screen reader voices the screens described to it and the keypad comes in through
 * [OrbitReader]. Everything the person does goes to the app ([Ui]).
 */
class OrbitActivity : ComponentActivity(), OrbitReader.Listener {
    private val main = Handler(Looper.getMainLooper())
    private lateinit var reader: OrbitReader
    private lateinit var liblouis: Liblouis
    private lateinit var view: TextView
    private var ui: Ui? = null
    private var front: Front? = null
    private var core: Core? = null

    private var saving: SaveFile? = null
    private val createDocument = registerForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
        val file = saving ?: return@registerForActivityResult
        saving = null
        if (uri == null) return@registerForActivityResult
        val resolver = contentResolver
        lifecycleScope.launch(Dispatchers.IO) { runCatching { resolver.openOutputStream(uri)?.use { it.write(file.bytes) } } }
    }
    private val openDocument = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        if (uri != null) importFrom(uri)
    }

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        takeDebugKeys()
        thisDevice = ThisDevice.DEVICE
        view = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(Color.WHITE)
            setBackgroundColor(Color.BLACK)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 26f)
            setPadding(32, 32, 32, 32)
        }
        setContentView(view)
        reader = OrbitReader(this, this)
        reader.claim.guardCrashes()
        liblouis = Liblouis(this)
        val core = (application as CoreHolder).core.getOrNull()
        if (core == null) {
            view.text = appWords.storeNotOpen
            reader.bind()
            return
        }
        this.core = core
        val app = core.app
        val ui = Ui(app, core, reader, liblouis, view, ::finish)
        ui.chooseFile = { openDocument.launch(arrayOf("*/*")) }
        this.ui = ui
        front = Front(this, core, "android").also { it.created() }
        app.send(ShellEvent.PairingAs(appWords.thisDevice, true))
        reader.bind()
        liblouis.bind()
        ui.push(HomeScreen())
        reminderTapped(intent)
        app.described = ui::turned
        lifecycleScope.launch { core.announcements.collect { ui.say(it) } }
        lifecycleScope.launch { app.files.collect { saveFile(it) } }
    }

    override fun onResume() {
        super.onResume()
        Clock.update(this)
        front?.resumed()
        reader.claim.resumed()
    }

    override fun onPause() {
        reader.claim.paused()
        front?.paused()
        super.onPause()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        reader.claim.windowFocus(hasFocus)
    }

    // Debug builds take keys over adb too: `am broadcast -a <DEBUG_KEY> --es name DOWN --ei action 0
    // --ez long false --ez phone false`, since nothing else can press the Orbit keypad remotely.
    private val debugKeys = object : android.content.BroadcastReceiver() {
        override fun onReceive(context: android.content.Context, intent: android.content.Intent) {
            key(KeyEvent(intent.getStringExtra("name").orEmpty(), intent.getIntExtra("action", 0), intent.getBooleanExtra("long", false)), intent.getBooleanExtra("phone", false))
        }
    }

    private fun takeDebugKeys() {
        if (applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0) {
            BuildConfigDebug.on = true
            registerReceiver(debugKeys, android.content.IntentFilter(DEBUG_KEY))
        }
    }

    private fun stopDebugKeys() {
        if (applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0) unregisterReceiver(debugKeys)
    }

    override fun onNewIntent(intent: android.content.Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        reminderTapped(intent)
    }

    /** Opened from a reminder's notification: its task, its block's day, or a replan. */
    private fun reminderTapped(intent: android.content.Intent?) {
        Reminders.opened(this, intent)?.reminded?.let { ui?.reminded(it) }
    }

    override fun onDestroy() {
        stopDebugKeys()
        core?.app?.described = null
        reader.unbind()
        liblouis.unbind()
        super.onDestroy()
    }

    @Deprecated("Orbit's keys are the keypad's; the back key arrives there")
    override fun onBackPressed() {
        ui?.back() ?: super.onBackPressed()
    }

    // ---- what Orbit tells this app, on binder threads ------------------------------------

    override fun readerReady() {
        main.post {
            reader.appOpened(appWords.metadata.name, "")
            ui?.readerReady()
        }
    }

    private val keys: KeyDelivery<Pair<KeyEvent, Boolean>> by lazy {
        KeyDelivery(reader.claim, { main.post(it) }) { (event, phoneKeypad) -> ui?.key(event, phoneKeypad) }
    }

    override fun key(event: KeyEvent, phoneKeypad: Boolean): Boolean = keys.deliver(event to phoneKeypad)

    override fun settingsChanged() {
        liblouis.refresh()
    }

    private companion object {
        const val DEBUG_KEY = "dev.emassey0135.lumenna.orbit.DEBUG_KEY"
    }

    // ---- files ----------------------------------------------------------------------------

    private fun saveFile(file: SaveFile) {
        saving = file
        createDocument.launch(file.name)
    }

    private fun importFrom(uri: Uri) {
        val resolver = contentResolver
        lifecycleScope.launch(Dispatchers.IO) {
            val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(0) else null
            }.orEmpty()
            val bytes = runCatching { resolver.openInputStream(uri)?.use { it.readBytes() } }.getOrNull()
                ?: return@launch
            withContext(Dispatchers.Main) {
                ui?.send(ShellEvent.Settings(SettingsEvent.Import(name, bytes)))
            }
        }
    }
}
