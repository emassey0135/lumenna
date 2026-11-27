package dev.emassey0135.lumenna

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext

/**
 * Asked once, as the app first finds Android holding it back in the background (its battery use
 * restricted, which MediaTek's DuraSpeed does to every app not in its list): no reminder goes off
 * until the app is opened, and reminders are often added by themselves, so the Reminders page alone
 * is not enough. The core's words; after this only the Reminders page says so.
 */
@Composable
fun HeldBackAsk() {
    val context = LocalContext.current
    var asking by remember {
        val ask = Reminders.backgroundRestricted(context) && Reminders.shouldAskHeldBack(context, BACKGROUND)
        if (ask) Reminders.askedHeldBack(context, BACKGROUND)
        mutableStateOf(ask)
    }
    if (!asking) return
    val words = appWords.notifications
    AlertDialog(
        onDismissRequest = { asking = false },
        title = { Text(words.channel) },
        text = { Text(words.heldBack) },
        confirmButton = {
            TextButton(modifier = Target, onClick = {
                asking = false
                runCatching { context.startActivity(Reminders.backgroundSettings(context)) }
            }) { Text(words.allowBackground) }
        },
        dismissButton = {
            TextButton(modifier = Target, onClick = { asking = false }) { Text(words.notNow) }
        },
    )
}

/** What [Reminders.shouldAskHeldBack] asks about on the phone: background use. */
private const val BACKGROUND = "background"
