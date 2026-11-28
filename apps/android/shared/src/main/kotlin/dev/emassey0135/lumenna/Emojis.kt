package dev.emassey0135.lumenna

import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp

/**
 * A task's, block's or project's emoji, drawn before its name as other planners draw an icon:
 * the core's (`RowView.emoji`, `SidebarEntry.emoji`, `PlanBlock.emoji`, `PlanAssignment.emoji`),
 * only ever one whole emoji. Seen and never said: it is cleared from TalkBack here, so a row's
 * description stays the core's line, which never holds it. [color] is the text's around it, for
 * an emoji the font draws as a plain glyph.
 */
@Composable
fun EmojiMark(emoji: String, modifier: Modifier = Modifier, size: TextUnit = 20.sp, color: Color = Color.Unspecified) {
    BasicText(emoji, modifier.clearAndSetSemantics {}, style = TextStyle(color = color, fontSize = size), maxLines = 1)
}

/**
 * [title] with [emoji] before it, for what draws text alone (a widget's line, the tile, a
 * complication), where TalkBack is given the glance's `spoken` instead and never this.
 */
fun withEmoji(emoji: String?, title: String): String = if (emoji.isNullOrEmpty()) title else "$emoji $title"
