package io.github.emassey0135.lumenna

import androidx.compose.ui.graphics.Color
import io.github.emassey0135.lumenna.app.Swatch

/**
 * The colour to draw a project's, label's, saved filter's or block's [Swatch] in: the core's
 * value for a dark background or a light one, each of 3:1 contrast with it. Drawn beside the
 * name and never in place of it, and kept out of what TalkBack says: the name, and the colour's
 * word in the row's line, already say it.
 */
fun Swatch.colour(dark: Boolean): Color = Color(android.graphics.Color.parseColor(if (dark) this.dark else light))
