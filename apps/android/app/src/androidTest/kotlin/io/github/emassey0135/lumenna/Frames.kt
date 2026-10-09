package io.github.emassey0135.lumenna

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.ForcedSize
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.min

/**
 * The app as a phone shows it, whatever the screen: the tests written for its tabs run on a
 * desktop-sized emulator too, where it would show the sidebar. Never wider than the screen,
 * which would shrink the density and every touch target with it. `KeyboardTest` and
 * `SidebarTest` are the wide window's.
 */
@OptIn(ExperimentalTestApi::class)
@Composable
fun PhoneWidth(content: @Composable () -> Unit) {
    BoxWithConstraints {
        DeviceConfigurationOverride(DeviceConfigurationOverride.ForcedSize(DpSize(min(maxWidth, 411.dp), maxHeight))) {
            content()
        }
    }
}
