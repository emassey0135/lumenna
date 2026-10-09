package io.github.emassey0135.lumenna

import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.util.UUID
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** New Task from outside the app — the launcher shortcut, the Quick Settings tile. */
@RunWith(AndroidJUnit4::class)
class NewTaskTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aNewTaskRequestOpensQuickAddOverTheTaskList() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val core = Core(File(context.cacheDir, "test-${UUID.randomUUID()}"))
        val requests = MutableStateFlow(0L)
        rule.setContent { LumennaTheme { LumennaApp(core, requests) } }
        requests.value = 1
        rule.waitUntil(5_000) {
            rule.onAllNodes(hasSetTextAction() and hasText("Task")).fetchSemanticsNodes().isNotEmpty()
        }
        rule.onNode(hasText("New Task")).assertExists()
    }
}
