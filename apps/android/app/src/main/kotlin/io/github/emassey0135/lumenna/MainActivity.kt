package io.github.emassey0135.lumenna

import android.content.Intent
import android.os.Bundle
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

    /** Counts New Task requests — the launcher shortcut, the Quick Settings tile — so each opens quick add. */
    private val newTask = MutableStateFlow(0L)

    private fun handle(intent: Intent?) {
        if (intent?.action == NEW_TASK) newTask.value += 1
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        Clock.update(this)
        handle(intent)
        setContent {
            LumennaTheme {
                core.fold(
                    onSuccess = { LumennaApp(it, newTask) },
                    // Nothing works without the store, so say why plainly.
                    onFailure = { error ->
                        Text(
                            "Lumenna could not open its store. ${error.message.orEmpty()}",
                            Modifier.fillMaxSize().padding(24.dp),
                        )
                    },
                )
            }
        }
    }

    /** Sync runs while the app is in front, as on the iPhone. */
    override fun onResume() {
        super.onResume()
        Clock.update(this)
        core.getOrNull()?.run {
            timeZoneMayHaveChanged()
            changed()
            startSyncing(this@MainActivity)
            backUpIfDue()
        }
    }

    companion object {
        /** Opens quick add: the launcher shortcut's and the tile's intent. */
        const val NEW_TASK = "io.github.emassey0135.lumenna.NEW_TASK"
    }

    /** Leaving: the service stops, and one more round is asked for, to send what was just edited. */
    override fun onPause() {
        core.getOrNull()?.stopSyncing()
        SyncWorker.syncOnLeaving(this)
        super.onPause()
    }
}
