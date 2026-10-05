package io.github.emassey0135.lumenna

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable

/**
 * Material 3's baseline colours, light or dark as the phone is. Not the wallpaper's dynamic
 * colours: those are chosen for harmony, and a fixed scheme is one whose contrast is checked
 * by the accessibility tests.
 */
@Composable
fun LumennaTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme(),
        content = content,
    )
}
