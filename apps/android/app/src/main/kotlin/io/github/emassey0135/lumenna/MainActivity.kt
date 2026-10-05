package io.github.emassey0135.lumenna

import android.os.Bundle
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        Clock.update(this)
        setContent {
            LumennaTheme {
                core.fold(
                    onSuccess = { LumennaApp(it) },
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

    /** Sync runs while the app is in front, as on the iPhone (§8). */
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

    override fun onPause() {
        core.getOrNull()?.stopSyncing()
        super.onPause()
    }
}
