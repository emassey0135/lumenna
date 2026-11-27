package dev.emassey0135.lumenna.orbit

import android.graphics.Color
import android.os.Bundle
import android.util.TypedValue
import android.view.Gravity
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import dev.emassey0135.orbit.files.FilePicker
import dev.emassey0135.orbit.files.FileRequest
import dev.emassey0135.orbit.files.PickKind
import dev.emassey0135.orbit.files.PickedFile
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

    // What a file chosen in Orbit's File Manager goes to, and the folder the last one was in.
    private var picking: ((PickedFile) -> Unit)? = null
    private var folder: String? = null
    private val picker = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val then = picking
        picking = null
        val file = FilePicker.picked(result.resultCode, result.data) ?: return@registerForActivityResult
        folder = file.parent
        then?.invoke(file)
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
        ui.chooseFile = { pick(FileRequest(PickKind.OPEN_FILE, startIn = folder), ::importFrom) }
        ui.chooseFolder = { key, current -> chooseFolder(key, current) }
        ui.reminderAccess = { Reminders.access(this) }
        ui.allowBackground = { Reminders.openBackgroundSettings(this) }
        ui.returnToPrevious = { dev.emassey0135.orbit.Foreground.returnToPrevious(this) }
        this.ui = ui
        front = Front(this, core, "android").also { it.created() }
        app.send(ShellEvent.PairingAs(appWords.thisDevice, true))
        session.created()
        ui.push(HomeScreen())
        // Held back in the background, no reminder goes off: asked once, over Home.
        if (Reminders.backgroundRestricted(this) && Reminders.shouldAskHeldBack(this, HELD_BACKGROUND)) {
            Reminders.askedHeldBack(this, HELD_BACKGROUND)
            ui.push(HeldBackScreen())
        }
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

    /**
     * Opened from a reminder's notification: its task, its block's day, or a replan; or brought to
     * the front by a reminder going off, which is said as its dialog. The ring is taken out of the
     * intent once heard, so the activity made again keeps no ringing of the past.
     */
    private fun reminderTapped(intent: android.content.Intent?) {
        Reminders.opened(this, intent)?.reminded?.let { ui?.reminded(it) }
        dev.emassey0135.lumenna.Ringing.of(intent)?.let { ringing ->
            dev.emassey0135.lumenna.Ringing.heard(intent)
            // Come to the front over another app for it: back to that app once it is dealt with.
            val fromBackground = dev.emassey0135.orbit.Foreground.openedFromBackground(intent)
            dev.emassey0135.orbit.Foreground.heard(intent)
            ui?.ring(ringing, fromBackground)
        }
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

        /** What [Reminders.shouldAskHeldBack] asks about here: background use. */
        const val HELD_BACKGROUND = "background"
    }

    // ---- files ----------------------------------------------------------------------------
    //
    // Chosen in Orbit's own File Manager (the SDK's `FilePicker`), which the keypad works and which
    // reads itself: Android's own picker is one Orbit's screen reader clicks through by itself. The
    // File Manager grants the file it answers with, so no storage permission is needed.

    /** Asks the File Manager for [request], then hands what was chosen to [then]; refused where it cannot be asked. */
    private fun pick(request: FileRequest, then: (PickedFile) -> Unit) {
        picking = then
        val started = runCatching { FilePicker.launch(request) { picker.launch(it) } }.getOrDefault(false)
        if (!started) {
            picking = null
            ui?.refuse()
        }
    }

    /** Each file the app makes is saved where the person chooses, written off the main thread. */
    private fun saveFile(file: SaveFile) {
        pick(FileRequest(PickKind.SAVE_FILE, suggestedName = file.name, startIn = folder)) { chosen ->
            val context = applicationContext
            lifecycleScope.launch(Dispatchers.IO) { runCatching { chosen.openOutput(context).use { it.write(file.bytes) } } }
        }
    }

    private fun importFrom(file: PickedFile) {
        val context = applicationContext
        lifecycleScope.launch(Dispatchers.IO) {
            val bytes = runCatching { file.openInput(context).use { it.readBytes() } }.getOrNull() ?: return@launch
            withContext(Dispatchers.Main) {
                ui?.send(ShellEvent.Settings(SettingsEvent.Import(file.name, bytes)))
            }
        }
    }

    /** A folder setting set to the folder chosen, starting where it is now. */
    private fun chooseFolder(key: String, current: String) {
        pick(FileRequest(PickKind.OPEN_FOLDER, startIn = current.ifEmpty { folder })) { chosen ->
            ui?.send(ShellEvent.Settings(SettingsEvent.Set(key, chosen.path)))
        }
    }
}
