package io.github.emassey0135.lumenna.wear

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.Text
import io.github.emassey0135.lumenna.Clock
import io.github.emassey0135.lumenna.SyncWorker

class MainActivity : ComponentActivity() {
    private val core get() = (application as WearApplication).core

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Clock.update(this)
        setContent {
            MaterialTheme {
                core.fold(
                    onSuccess = { WearApp(it) },
                    // Nothing works without the store, so say why plainly.
                    onFailure = { error -> Text("Lumenna could not open its store. ${error.message.orEmpty()}") },
                )
            }
        }
    }

    /** Sync runs while the app is in front, as on the phone: the watch is a peer of its own. */
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

    /** Leaving: the service stops, and one more round is asked for, to send what was just edited. */
    override fun onPause() {
        core.getOrNull()?.stopSyncing()
        SyncWorker.syncOnLeaving(this)
        super.onPause()
    }
}
