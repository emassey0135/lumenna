package dev.emassey0135.lumenna.orbit

import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.TypedValue
import android.view.Gravity
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
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
import dev.emassey0135.orbit.session.OrbitSession
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The one activity: a text view showing the current line, for a sighted helper, while Orbit's
 * screen reader voices the screens described to it and the keypad comes in through the SDK's
 * [OrbitSession]. Everything the person does goes to the app ([Ui]).
 */
class OrbitActivity : ComponentActivity() {
    private lateinit var session: OrbitSession
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
        thisDevice = ThisDevice.DEVICE
        // Debug builds take keys over adb too (the SDK's `DebugKeys`): `am broadcast -a <DEBUG_KEY>
        // --es name DOWN --ei action 0 --ez long false --ez phone false`.
        session = OrbitSession(this, appWords.metadata.name, debugKeys = DEBUG_KEY)
        if (session.takesDebugKeys) BuildConfigDebug.on = true
        view = TextView(this).apply {
            gravity = Gravity.CENTER
            setTextColor(Color.WHITE)
            setBackgroundColor(Color.BLACK)
            setTextSize(TypedValue.COMPLEX_UNIT_SP, 26f)
            setPadding(32, 32, 32, 32)
        }
        setContentView(view)
        val core = (application as CoreHolder).core.getOrNull()
        if (core == null) {
            view.text = appWords.storeNotOpen
            session.created()
            return
        }
        this.core = core
        val app = core.app
        val ui = Ui(app, core, session, view, ::finish)
        ui.chooseFile = { openDocument.launch(arrayOf("*/*")) }
        this.ui = ui
        front = Front(this, core, "android").also { it.created() }
        app.send(ShellEvent.PairingAs(appWords.thisDevice, true))
        session.created()
        ui.push(HomeScreen())
        reminderTapped(intent)
        app.described = ui::turned
        lifecycleScope.launch { core.announcements.collect { ui.announce(it) } }
        lifecycleScope.launch { app.files.collect { saveFile(it) } }
    }

    private var resumedOnce = false

    override fun onResume() {
        super.onResume()
        // Back in front: what came due meanwhile is offered on Home. The first time, Home looks itself.
        if (resumedOnce) ui?.lookBack()
        resumedOnce = true
        Clock.update(this)
        front?.resumed()
        session.resumed()
    }

    override fun onPause() {
        session.paused()
        front?.paused()
        super.onPause()
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        session.windowFocus(hasFocus)
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
        core?.app?.described = null
        session.destroyed()
        super.onDestroy()
    }

    @Deprecated("Orbit's keys are the keypad's; the back key arrives there")
    override fun onBackPressed() {
        ui?.back() ?: super.onBackPressed()
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
