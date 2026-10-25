package dev.emassey0135.lumenna.tasker

import android.app.Activity
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import dev.emassey0135.lumenna.Choose
import dev.emassey0135.lumenna.ChooseDay
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.LumennaApplication
import dev.emassey0135.lumenna.LumennaTheme
import dev.emassey0135.lumenna.Option
import dev.emassey0135.lumenna.Target
import dev.emassey0135.lumenna.app.AssistantOperation
import dev.emassey0135.lumenna.app.AutomationChoice
import dev.emassey0135.lumenna.app.AutomationField
import dev.emassey0135.lumenna.app.AutomationGiven
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.automationCheck
import dev.emassey0135.lumenna.app.automationFields
import dev.emassey0135.lumenna.app.automationOperations
import dev.emassey0135.lumenna.app.automationWords
import dev.emassey0135.lumenna.app.sentenceCase
import dev.emassey0135.lumenna.app.storeUnopened
import dev.emassey0135.lumenna.quiet
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Tasker's "edit action" screen (Locale's `EDIT_SETTING`): where the person chooses what a
 * Tasker action does in Lumenna and fills what it takes, which is their consent to Tasker running
 * it. Saving makes the action's secret, kept here and in the bundle Tasker keeps
 * ([TaskerSecrets]); opening an action set up before keeps its secret.
 */
class TaskerEditActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val core = (application as LumennaApplication).core
        val before = TaskerAction.from(intent.getBundleExtra(Tasker.EXTRA_BUNDLE))
        val secrets = TaskerSecrets(this)
        setContent {
            LumennaTheme {
                Surface(Modifier.fillMaxSize()) {
                    core.fold(
                        onSuccess = { opened ->
                            TaskerSetup(opened, before) { operation, values, shown ->
                                val secret = before?.secret ?: secrets.make()
                                secrets.keep(secret, operation)
                                setResult(Activity.RESULT_OK, Tasker.edited(TaskerAction(operation, values, shown, secret)))
                                finish()
                            }
                        },
                        onFailure = { error ->
                            Text(storeUnopened(error.message.orEmpty()), Modifier.safeDrawingPadding().padding(24.dp))
                        },
                    )
                }
            }
        }
    }
}

/**
 * The setup itself: the core's operations an action may run, then the one chosen's fields, each
 * labelled and described in the core's words, with what it can be chosen from beside it.
 * [save] is given the operation, each value as typed (a Tasker variable among them) and each as
 * it was shown when chosen.
 */
@Composable
fun TaskerSetup(
    core: Core,
    before: TaskerAction?,
    save: (operation: String, values: Map<String, String>, shown: Map<String, String>) -> Unit,
) {
    val words = remember { automationWords() }
    val operations = remember { automationOperations() }
    // An action set up before opens on what it does; a new one on the choice of operation.
    var chosen by remember { mutableStateOf(operations.firstOrNull { it.id == before?.operation }) }
    val values = remember { mutableStateMapOf<String, String>().apply { before?.values?.let { putAll(it) } } }
    val shown = remember { mutableStateMapOf<String, String>().apply { before?.shown?.let { putAll(it) } } }
    BackHandler(enabled = chosen != null && before == null) { chosen = null }
    Column(
        Modifier
            .fillMaxSize()
            .safeDrawingPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(sentenceCase(words.title), style = MaterialTheme.typography.headlineSmall, modifier = Modifier.semantics { heading() })
        val operation = chosen
        if (operation == null) {
            Operations(operations, words.operations, words.destructive) { picked ->
                if (picked.id != before?.operation) {
                    values.clear()
                    shown.clear()
                }
                chosen = picked
            }
        } else {
            Parameters(core, operation, values, shown, words.parameters, words.variables, words.destructive, words.nothingToChoose)
            var refused by remember(operation.id) { mutableStateOf<String?>(null) }
            Button(
                onClick = {
                    val given = automationFields(operation.id).map { AutomationGiven(it.name, values[it.name].orEmpty()) }
                    refused = automationCheck(operation.id, given)
                    if (refused == null) {
                        val kept = given.filter { it.text.isNotBlank() }.associate { it.name to it.text }
                        save(operation.id, kept, shown.filterKeys { it in kept })
                    }
                },
                modifier = Target.fillMaxWidth(),
            ) { Text(sentenceCase(words.save)) }
            // Why it cannot be saved, said as it appears.
            refused?.let {
                Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
            }
        }
    }
}

/**
 * The operations to choose from, a row each, which opens its fields: its name, what it does, and
 * whether it removes something, said together as one button.
 */
@Composable
private fun Operations(operations: List<AssistantOperation>, heading: String, destructive: String, picked: (AssistantOperation) -> Unit) {
    Text(sentenceCase(heading), style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
    for (operation in operations) {
        Column(
            Modifier
                .fillMaxWidth()
                .then(Target)
                .clickable(role = Role.Button) { picked(operation) }
                .padding(vertical = 8.dp),
        ) {
            Text(operation.sentence, style = MaterialTheme.typography.bodyLarge)
            Text(operation.description, style = MaterialTheme.typography.bodyMedium, color = quiet())
            if (operation.destructive) {
                Text(destructive, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            }
        }
    }
}

/** The chosen operation, then a field for each parameter, as the core words it. */
@Composable
private fun Parameters(
    core: Core,
    operation: AssistantOperation,
    values: MutableMap<String, String>,
    shown: MutableMap<String, String>,
    heading: String,
    variables: String,
    destructive: String,
    nothingToChoose: String,
) {
    Text(operation.sentence, style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
    Text(operation.description, style = MaterialTheme.typography.bodyMedium)
    if (operation.destructive) Text(destructive, color = MaterialTheme.colorScheme.error)
    val fields = remember(operation.id) { automationFields(operation.id) }
    if (fields.isEmpty()) return
    Text(sentenceCase(heading), style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
    Text(variables, style = MaterialTheme.typography.bodyMedium, color = quiet())
    for (field in fields) {
        Field(core, operation.id, field, values, shown, nothingToChoose)
    }
}

/**
 * One parameter: a line, labelled by its title and described by its hint, which TalkBack reads
 * with it; then, where it has them, a button offering what it can be (the core's choices) and a
 * calendar for a day, each filling the line.
 */
@Composable
private fun Field(
    core: Core,
    operation: String,
    field: AutomationField,
    values: MutableMap<String, String>,
    shown: MutableMap<String, String>,
    nothingToChoose: String,
) {
    val text = values[field.name].orEmpty()
    OutlinedTextField(
        value = text,
        onValueChange = {
            values[field.name] = it
            // Typed over, it is shown as typed.
            shown.remove(field.name)
        },
        label = { Text(sentenceCase(field.title)) },
        supportingText = { Text(field.hint) },
        keyboardOptions = KeyboardOptions(keyboardType = if (field.number) KeyboardType.Number else KeyboardType.Text),
        modifier = Modifier.fillMaxWidth(),
    )
    if (field.day) {
        ChooseDay(text) {
            values[field.name] = it
            shown.remove(field.name)
        }
    }
    val choose = field.choose ?: return
    val scope = rememberCoroutineScope()
    var choices by remember { mutableStateOf<List<AutomationChoice>?>(null) }
    var none by remember { mutableStateOf(false) }
    OutlinedButton(
        onClick = {
            scope.launch {
                val offered = withContext(Dispatchers.IO) {
                    try {
                        core.device.automationChoices(operation, field.name)
                    } catch (_: LumennaError) {
                        emptyList()
                    }
                }
                none = offered.isEmpty()
                choices = offered.takeIf { it.isNotEmpty() }
            }
        },
        modifier = Target,
    ) { Text(sentenceCase(choose)) }
    if (none) Text(nothingToChoose, modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
    choices?.let { offered ->
        Choose(
            title = sentenceCase(choose),
            choices = offered.map { Option(it.value, it.title, it.detail.orEmpty()) },
            yes = sentenceCase(choose),
            dismiss = { choices = null },
        ) { option ->
            choices = null
            if (field.several && text.isNotBlank()) {
                values[field.name] = "$text, ${option.key}"
                shown[field.name] = "${shown[field.name] ?: text}, ${option.title}"
            } else {
                values[field.name] = option.key
                shown[field.name] = option.title
            }
        }
    }
}
