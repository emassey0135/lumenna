package dev.emassey0135.lumenna.wear

import android.content.Intent
import android.os.Bundle
import dev.emassey0135.lumenna.ReminderOpened
import dev.emassey0135.lumenna.Reminders
import kotlinx.coroutines.flow.MutableStateFlow
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.Text
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Front
import dev.emassey0135.lumenna.Launching
import dev.emassey0135.lumenna.SyncWorker

class MainActivity : ComponentActivity() {
    private val core get() = (application as WearApplication).core

    /** Coming to the front and leaving it, as the phone does (`Front`): the watch is a peer of its own. */
    private val front by lazy { core.getOrNull()?.let { Front(this, it, PLATFORM) } }

    /** The last reminder notification tapped, counted so a second tap on the same one opens it too. */
    private val opened = MutableStateFlow<Pair<Long, ReminderOpened>?>(null)

    /** The last launch shortcut asked for (the tile, a complication), counted so asking again does it again. */
    private val launched = MutableStateFlow<Pair<Long, String>?>(null)

    private fun handle(intent: Intent?) {
        Reminders.opened(this, intent)?.let { opened.value = (opened.value?.first ?: 0L) + 1 to it }
        Launching.id(intent)?.let { launched.value = (launched.value?.first ?: 0L) + 1 to it }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handle(intent)
        front?.created()
        Clock.update(this)
        setContent {
            MaterialTheme {
                core.fold(
                    onSuccess = { WearApp(it, opened = opened, launched = launched) },
                    // Nothing works without the store, so say why plainly.
                    onFailure = { error -> Text(dev.emassey0135.lumenna.app.storeUnopened(error.message.orEmpty())) },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        Clock.update(this)
        front?.resumed()
    }

    override fun onPause() {
        front?.paused() ?: SyncWorker.syncOnLeaving(this)
        super.onPause()
    }
}
