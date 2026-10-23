package dev.emassey0135.lumenna.wear

import dev.emassey0135.lumenna.RowFocus
import dev.emassey0135.lumenna.rowKey
import androidx.compose.ui.focus.focusRequester
import android.app.RemoteInput
import android.content.Intent
import android.view.inputmethod.EditorInfo
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import dev.emassey0135.lumenna.app.Swatch
import dev.emassey0135.lumenna.colour
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ListHeader
import androidx.wear.compose.material3.Text
import androidx.wear.input.RemoteInputIntentHelper
import androidx.wear.input.wearableExtender
import dev.emassey0135.lumenna.RowAction

/**
 * Asking for a line of text. On a watch there is no text field: the system's own input screen
 * takes it — voice, the keyboard, handwriting — and hands it back. A test provides its own.
 */
fun interface TextEntry {
    fun ask(label: String, answered: (String) -> Unit)
}

val LocalTextEntry = compositionLocalOf<TextEntry?> { null }

private const val KEY = "lumenna-text"

/** The system's input screen, as the app asks for text: `null` in a test, which provides one. */
@Composable
fun rememberSystemTextEntry(): TextEntry {
    val waiting = remember { mutableStateOf<((String) -> Unit)?>(null) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        val text = result.data?.let { RemoteInput.getResultsFromIntent(it) }?.getCharSequence(KEY)?.toString()
        val answered = waiting.value
        waiting.value = null
        if (text != null) answered?.invoke(text)
    }
    return remember {
        TextEntry { label, answered ->
            waiting.value = answered
            val input = RemoteInput.Builder(KEY).setLabel(label).wearableExtender {
                setEmojisAllowed(false)
                setInputActionType(EditorInfo.IME_ACTION_DONE)
            }.build()
            val intent: Intent = RemoteInputIntentHelper.createActionRemoteInputIntent()
            RemoteInputIntentHelper.putRemoteInputsExtra(intent, listOf(input))
            launcher.launch(intent)
        }
    }
}

/**
 * A row: its title, then what is said of it, one stop for TalkBack, with its actions as the
 * row's custom actions and, for touch, on a long press. As on the phone, the title comes
 * first and is never abbreviated.
 */
@Composable
fun RowButton(
    title: String,
    detail: String? = null,
    state: String? = null,
    actions: List<RowAction> = emptyList(),
    onLongClick: (() -> Unit)? = null,
    /** The row's key, and its list's landing (`rememberLanding`), for focus put back on it. */
    key: String? = null,
    focus: RowFocus? = null,
    /** A project's, label's or block's colour, drawn as the button's icon beside its name. */
    swatch: Swatch? = null,
    onClick: () -> Unit,
) {
    Button(
        onClick = onClick,
        onLongClick = onLongClick,
        icon = swatch?.let { { SwatchDot(it) } },
        modifier = Modifier.fillMaxWidth()
            .then(if (key != null && focus != null) Modifier.focusRequester(focus.requester(key)) else Modifier)
            .semantics {
                if (key != null) rowKey = key
                if (state != null) stateDescription = state
                if (actions.isNotEmpty()) customActions = actions.map { action -> CustomAccessibilityAction(action.name) { action.run(); true } }
            },
        label = { Text(title) },
        secondaryLabel = detail?.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
    )
}

/**
 * A colour's mark beside a name ([Swatch]): a dot in the core's value for a dark background,
 * which a watch always has. Nothing for TalkBack, which the name and its line already tell.
 */
@Composable
fun SwatchDot(swatch: Swatch) {
    Box(Modifier.size(14.dp).clip(CircleShape).background(swatch.colour(dark = true)))
}

/**
 * A section's title, which TalkBack can move between by heading; [actions] are what is done to
 * what it heads, as its custom actions — the day's, on the day's heading.
 */
@Composable
fun Heading(text: String, actions: List<RowAction> = emptyList()) {
    ListHeader(modifier = Modifier.semantics {
        heading()
        if (actions.isNotEmpty()) customActions = actions.map { action -> CustomAccessibilityAction(action.name) { action.run(); true } }
    }) { Text(text) }
}
