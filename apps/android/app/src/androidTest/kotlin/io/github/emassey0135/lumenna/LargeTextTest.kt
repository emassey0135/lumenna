package io.github.emassey0135.lumenna

import android.graphics.Bitmap
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.DeviceConfigurationOverride
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.FontScale
import androidx.compose.ui.test.ForcedSize
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.accessibility.enableAccessibilityChecks
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.then
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import io.github.emassey0135.lumenna.core.MoveTarget
import java.io.File
import java.util.UUID
import org.junit.After
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Every screen at Android's largest font size, 200%, in a small phone's width, 360dp: where
 * layouts break first, as the iPhone SE showed. Google's accessibility checks run on each, and
 * each is kept as a screenshot (in `connected_android_test_additional_output`), since what a
 * clipped label looks like is what the checks cannot say.
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(AndroidJUnit4::class)
class LargeTextTest {
    @get:Rule
    val rule = createComposeRule()

    private lateinit var directory: File
    private lateinit var core: Core

    @Before
    fun open() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        directory = File(context.cacheDir, "test-${UUID.randomUUID()}")
        core = Core(directory)
        Clock.update(context)
        core.lumenna.addTask("call the bank about the mortgage renewal tomorrow at 3pm p1")
        core.lumenna.addTask("essay")
        core.lumenna.addTask("outline the argument")
        val rows = core.lumenna.listTasks("").rows
        core.lumenna.moveTask(
            rows.first { it.title == "outline the argument" }.id,
            MoveTarget.Parent(rows.first { it.title == "essay" }.id),
        )
        rule.enableAccessibilityChecks()
        rule.setContent {
            BoxWithConstraints {
                DeviceConfigurationOverride(
                    DeviceConfigurationOverride.ForcedSize(DpSize(360.dp, maxHeight)) then
                        DeviceConfigurationOverride.FontScale(2f),
                ) {
                    LumennaTheme { LumennaApp(core) }
                }
            }
        }
        rule.waitForIdle()
    }

    @After
    fun close() {
        directory.deleteRecursively()
    }

    /** Kept where Gradle collects a test's extra output, else beside the store. */
    private fun keep(name: String) {
        rule.waitForIdle()
        val folder = InstrumentationRegistry.getArguments().getString("additionalTestOutputDir")
            ?.let(::File) ?: File(directory.parentFile, "large-text")
        folder.mkdirs()
        File(folder, "large-text-$name.png").outputStream().use {
            rule.onRoot().captureToImage().asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, it)
        }
    }

    private fun tab(name: String) {
        rule.onNode(hasText(name) and SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Tab)).performClick()
        rule.waitForIdle()
    }

    private fun row(title: String) {
        rule.onNode(SemanticsMatcher("row titled $title") { it.config.getOrNull(RowTitle) == title }).performClick()
        rule.waitForIdle()
    }

    private fun press(name: String) {
        rule.onNodeWithContentDescription(name).performClick()
        rule.waitForIdle()
    }

    private fun back() = press("Back")

    @Test
    fun everyScreenAtTheLargestFontInASmallPhone() {
        tab("Today")
        keep("1-today")
        press("Add block")
        keep("2-block-form")
        back()
        tab("Tasks")
        keep("3-tasks")
        press("Add task")
        rule.onNode(hasSetTextAction() and hasText("Task")).performTextInput("call the bank tomorrow at 3pm p1 #Inbox")
        keep("4-quick-add")
        back()
        row("essay")
        keep("5-task-detail")
        back()
        tab("Browse")
        keep("6-browse")
        row("Projects")
        keep("7-projects")
        back()
        tab("Settings")
        keep("8-settings")
        for ((page, name) in listOf("Planning" to "9-planning", "Devices and Sync" to "10-devices", "Backups" to "11-backups", "Export and Import" to "12-export")) {
            row(page)
            keep(name)
            back()
        }
    }
}
