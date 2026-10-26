package dev.emassey0135.lumenna

import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import android.view.KeyboardShortcutGroup
import android.view.Menu
import kotlinx.coroutines.flow.MutableStateFlow
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

class MainActivity : ComponentActivity() {
    private val core get() = (application as LumennaApplication).core

    /** Coming to the front and leaving it, as the watch does (`Front`). */
    private val front by lazy { core.getOrNull()?.let { Front(this, it, "android") } }

    /**
     * The last launch shortcut asked for (`Launches`: the icon's shortcuts, the Quick Settings
     * tile, a widget), counted so asking again for the same one does it again.
     */
    private val launched = MutableStateFlow<Pair<Long, String>?>(null)

    /** What each keyboard command does now, offered by the screens shown. */
    private val shortcuts = Shortcuts()

    /**
     * A key nothing focused took: an app-wide command, if it is one. Compose's root takes
     * them while anything in it has focus; this is for when nothing does.
     */
    override fun dispatchKeyEvent(event: KeyEvent): Boolean =
        super.dispatchKeyEvent(event) || shortcuts.handle(event)

    /** The system's keyboard shortcuts helper (Meta+/) lists what works now. */
    override fun onProvideKeyboardShortcuts(data: MutableList<KeyboardShortcutGroup>, menu: Menu?, deviceId: Int) {
        super.onProvideKeyboardShortcuts(data, menu, deviceId)
        data.addAll(shortcuts.groups())
    }

    /** The last reminder notification tapped, counted so a second tap on the same one opens it too. */
    private val opened = MutableStateFlow<Pair<Long, ReminderOpened>?>(null)

    private fun handle(intent: Intent?) {
        Launches.id(intent)?.let { id ->
            launched.value = (launched.value?.first ?: 0L) + 1 to id
            androidx.core.content.pm.ShortcutManagerCompat.reportShortcutUsed(this, id)
        }
        Reminders.opened(this, intent)?.let { opened.value = (opened.value?.first ?: 0L) + 1 to it }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        front?.created()
        enableEdgeToEdge()
        Clock.update(this)
        handle(intent)
        setContent {
            LumennaTheme {
                core.fold(
                    onSuccess = { LumennaApp(it, launched, shortcuts, opened) },
                    // Nothing works without the store, so say why plainly.
                    onFailure = { error ->
                        Text(
                            dev.emassey0135.lumenna.app.storeUnopened(error.message.orEmpty()),
                            Modifier.fillMaxSize().padding(24.dp),
                        )
                    },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        Clock.update(this)
        front?.resumed()
    }

    companion object {
        /** Opens quick add: what the launcher's shortcut asked for before the core listed them (`Launches`). */
        const val NEW_TASK = "dev.emassey0135.lumenna.NEW_TASK"
    }

    override fun onPause() {
        front?.paused() ?: SyncWorker.syncOnLeaving(this)
        super.onPause()
    }
}
