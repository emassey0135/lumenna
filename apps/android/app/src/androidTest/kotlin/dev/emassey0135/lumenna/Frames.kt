package dev.emassey0135.lumenna

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.ForcedSize
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.min
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick

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

/**
 * Presses what a top bar offers by its name, as TalkBack finds it: its icon, named by the core's
 * title, or, where the bar had no room for it, its item in the bar's overflow menu, which ends in
 * "…" where it asks something first.
 */
fun ComposeTestRule.pressInBar(name: String) {
    if (onAllNodesWithContentDescription(name).fetchSemanticsNodes().isNotEmpty()) {
        onNodeWithContentDescription(name).performClick()
    } else {
        onNodeWithContentDescription(appWords.more).performClick()
        waitForIdle()
        onNode(hasClickAction() and (hasText(name) or hasText("$name…"))).performClick()
    }
    waitForIdle()
}
