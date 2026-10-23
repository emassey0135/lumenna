package dev.emassey0135.lumenna

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext

/**
 * Asks for notifications, from Android 13, the first time it is called: when the person first
 * adds a reminder or turns reminders on, on the phone or the watch. Afterwards the Reminders
 * settings page says they are off and opens the system's page.
 */
@Composable
fun rememberNotificationAsk(): () -> Unit {
    val context = LocalContext.current
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    return remember(launcher) {
        {
            if (Reminders.shouldAsk(context)) {
                Reminders.asked(context)
                launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        }
    }
}
