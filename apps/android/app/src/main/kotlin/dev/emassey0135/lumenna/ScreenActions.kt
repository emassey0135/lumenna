package dev.emassey0135.lumenna

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.RowScope
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.AddCircle
import androidx.compose.material.icons.filled.DateRange
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.emassey0135.lumenna.app.ScreenAction
import dev.emassey0135.lumenna.app.ShellEvent

/**
 * A place's own actions, as the core lists them (`ViewModel.screen_actions`), and what this screen
 * runs of those that are the shell's own ([own], by `id`: the filter field). Every other one is
 * the app's, sent as `ShellEvent.Key(id)`.
 */
class ScreenButtons(val actions: List<ScreenAction>, val own: Map<String, () -> Unit> = emptyMap()) {
    /** What is drawn: the shell's own only where this screen runs it. */
    val shown: List<ScreenAction> get() = actions.filter { !it.shell || it.id in own }

    fun run(app: AppModel, action: ScreenAction) {
        if (action.shell) own[action.id]?.invoke() else app.send(ShellEvent.Key(action.id))
    }
}

/**
 * Each screen action's icon in the top bar, by its `id`: the look is Android's, the name the
 * core's (the icon's content description is the action's title). One with none here is listed
 * in the bar's overflow menu, by its name.
 */
private fun iconOf(id: String): ImageVector? = when (id) {
    "new-task", "new-block" -> Icons.Filled.Add
    "new-project-inside" -> Icons.Filled.AddCircle
    "filter" -> Icons.Filled.Search
    "what-now" -> Icons.Filled.PlayArrow
    "previous-day" -> Icons.AutoMirrored.Filled.KeyboardArrowLeft
    "next-day" -> Icons.AutoMirrored.Filled.KeyboardArrowRight
    "go-to-day" -> Icons.Filled.DateRange
    "go-to-now" -> Glyphs.today
    "undo" -> Glyphs.undo
    "redo" -> Glyphs.redo
    else -> null
}

/** What the title keeps of a top bar at least, besides its icons: Material's own inset and a short title. */
private val TitleRoom = 160.dp

/** An icon button's width in the top bar: Android's minimum touch target. */
private val IconRoom = 48.dp

/**
 * How many icons a top bar [width] wide holds beside its title, the overflow's included: at
 * least two, so one action shows beside the overflow.
 */
internal fun iconRoom(width: Dp): Int = ((width - TitleRoom) / IconRoom).toInt().coerceIn(2, 8)

/**
 * The screen's own actions in the top bar, in the core's order, as Material lays out a top app
 * bar: icons while there is [room], the rest in its overflow menu. Each icon is named by the
 * core's title, disabled while the core says it does nothing now; an overflow item that asks
 * something first ends in "…", as the Export page's buttons do.
 */
@Composable
fun RowScope.ScreenActionButtons(app: AppModel, buttons: ScreenButtons, room: Int) {
    val shown = buttons.shown
    val iconed = shown.filter { iconOf(it.id) != null }
    val unIconed = shown.size - iconed.size
    val fits = iconed.size + (if (unIconed > 0) 1 else 0) <= room
    val asIcons = if (fits) iconed else iconed.take(room - 1)
    val inMenu = shown.filter { it !in asIcons }
    asIcons.forEach { action ->
        IconButton(onClick = { buttons.run(app, action) }, enabled = action.enabled) {
            Icon(iconOf(action.id)!!, contentDescription = action.title)
        }
    }
    if (inMenu.isEmpty()) return
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Filled.MoreVert, contentDescription = appWords.more)
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            inMenu.forEach { action ->
                DropdownMenuItem(
                    text = { Text(if (action.asks) "${action.title}…" else action.title) },
                    enabled = action.enabled,
                    onClick = {
                        open = false
                        buttons.run(app, action)
                    },
                )
            }
        }
    }
}
