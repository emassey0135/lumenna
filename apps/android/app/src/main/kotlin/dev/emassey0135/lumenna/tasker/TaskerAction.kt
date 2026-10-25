package dev.emassey0135.lumenna.tasker

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.app.AutomationGiven
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.automationOperations
import dev.emassey0135.lumenna.app.automationOutputs
import dev.emassey0135.lumenna.app.automationSummary
import dev.emassey0135.lumenna.app.automationWords
import dev.emassey0135.lumenna.app.sentenceCase
import dev.emassey0135.lumenna.sentence
import java.security.MessageDigest
import java.security.SecureRandom

/**
 * An action set up for Tasker: the core's operation (`automation_operations()`, the assistant
 * table less what asks first or takes a proposal), what each parameter was given as text (a
 * Tasker variable among it, filled in only as it fires), how each was shown when chosen (a
 * task's title for its identifier), and the secret made when it was set up.
 *
 * It travels as the Locale bundle Tasker keeps and hands back at every firing: Tasker replaces
 * variables in the values alone, never in the secret.
 */
data class TaskerAction(
    val operation: String,
    val values: Map<String, String>,
    val shown: Map<String, String>,
    val secret: String,
) {
    /** The bundle Tasker keeps. */
    fun bundle(): Bundle = Bundle().apply {
        putString(OPERATION, operation)
        putString(SECRET, secret)
        values.forEach { (name, text) -> putString(VALUE + name, text) }
        shown.forEach { (name, text) -> putString(SHOWN + name, text) }
        // Tasker fills in its variables in these alone, as the action fires.
        putString(REPLACE_KEYS, values.keys.joinToString(" ") { VALUE + it })
    }

    /** What Tasker lists the action by: the core's summary of it, as it was chosen. */
    fun blurb(): String = automationSummary(
        operation,
        values.map { (name, text) -> AutomationGiven(name, shown[name] ?: text) },
        true,
    )

    /** What it was given, for the core, once Tasker has filled in its variables. */
    fun given(): List<AutomationGiven> = values.map { (name, text) -> AutomationGiven(name, text) }

    companion object {
        private const val PREFIX = "dev.emassey0135.lumenna.tasker."
        const val OPERATION = PREFIX + "OPERATION"
        const val SECRET = PREFIX + "SECRET"
        const val VALUE = PREFIX + "VALUE."
        const val SHOWN = PREFIX + "SHOWN."

        /** The bundle's own keys whose String values Tasker fills its variables into. */
        const val REPLACE_KEYS = "net.dinglisch.android.tasker.extras.VARIABLE_REPLACE_KEYS"

        /** The action in [bundle], if it is one of ours. */
        fun from(bundle: Bundle?): TaskerAction? {
            val operation = bundle?.getString(OPERATION) ?: return null
            val secret = bundle.getString(SECRET) ?: return null
            fun under(prefix: String) = bundle.keySet().filter { it.startsWith(prefix) }
                .associate { it.removePrefix(prefix) to bundle.getString(it).orEmpty() }
            return TaskerAction(operation, under(VALUE), under(SHOWN), secret)
        }
    }
}

/**
 * The secrets of the actions set up on this device, each kept as its hash beside the operation
 * it was set up for, in the app's own preferences (no other app can read them, and the app is
 * never backed up). A firing is run only when its secret is one of these and names the same
 * operation: another app can send the plugin's broadcast, but cannot know a secret, and an action
 * imported from someone else's Tasker carries one this device never made.
 */
class TaskerSecrets(context: Context) {
    private val kept = context.getSharedPreferences("tasker-secrets", Context.MODE_PRIVATE)

    /** A new secret, for an action being set up for the first time. */
    fun make(): String = ByteArray(32).also { SecureRandom().nextBytes(it) }.joinToString("") { "%02x".format(it) }

    /**
     * Keeps [secret] as the one set up for [operation]; setting it up again replaces what it ran.
     * Written before Tasker is handed the bundle, so a firing straight after finds it.
     */
    @SuppressLint("ApplySharedPref")
    fun keep(secret: String, operation: String) {
        kept.edit().putString(hash(secret), operation).commit()
    }

    /** Whether [secret] was set up on this device for [operation]. */
    fun allows(secret: String, operation: String): Boolean = kept.getString(hash(secret), null) == operation

    private fun hash(secret: String): String =
        MessageDigest.getInstance("SHA-256").digest(secret.toByteArray()).joinToString("") { "%02x".format(it) }
}

/** What a firing answers Tasker: success with its variables, or a failure with the core's sentence. */
data class Fired(val ok: Boolean, val variables: Bundle)

/** Tasker's side of a plugin action: what its edit and fire intents carry, and what it takes back. */
object Tasker {
    const val EXTRA_BUNDLE = "com.twofortyfouram.locale.intent.extra.BUNDLE"
    const val EXTRA_BLURB = "com.twofortyfouram.locale.intent.extra.BLURB"
    private const val RELEVANT_VARIABLES = "net.dinglisch.android.tasker.RELEVANT_VARIABLES"
    private const val REQUESTED_TIMEOUT = "net.dinglisch.android.tasker.extras.REQUESTED_TIMEOUT"
    private const val VARIABLES = "net.dinglisch.android.tasker.extras.VARIABLES"
    private const val COMPLETION_INTENT = "net.dinglisch.android.tasker.extras.COMPLETION_INTENT"
    private const val RESULT_CODE = "net.dinglisch.android.tasker.extras.RESULT_CODE"
    private const val CALL_SERVICE_PACKAGE = "net.dinglisch.android.tasker.EXTRA_CALL_SERVICE_PACKAGE"
    private const val CALL_SERVICE = "net.dinglisch.android.tasker.EXTRA_CALL_SERVICE"
    private const val CALL_SERVICE_FOREGROUND = "net.dinglisch.android.tasker.EXTRA_CALL_SERVICE_FOREGROUND"

    /** Tasker's result codes: `Activity.RESULT_OK`, and its own first failure. */
    const val RESULT_OK = -1
    const val RESULT_FAILED = 2

    /** How long Tasker waits for an answer; the core answers in well under a second. */
    private const val TIMEOUT_MS = 30_000

    /** The edit activity's result: the bundle, its blurb, the variables it gives back, and that Tasker should wait. */
    fun edited(action: TaskerAction): Intent = Intent().apply {
        putExtra(EXTRA_BUNDLE, action.bundle())
        putExtra(EXTRA_BLURB, action.blurb())
        putExtra(RELEVANT_VARIABLES, relevant(action.operation).toTypedArray())
        putExtra(REQUESTED_TIMEOUT, TIMEOUT_MS)
    }

    /**
     * The variables [operation] gives back, as Tasker's picker lists them: `%name`, or `%name()`
     * for several, then the core's title and description.
     */
    fun relevant(operation: String): List<String> = automationOutputs(operation).map {
        "%${it.name}${if (it.several) "()" else ""}\n${sentenceCase(it.title)}\n${it.description}"
    }

    /**
     * Runs the action Tasker fired with [bundle] (its variables already filled in) against
     * [core], refusing it unless [secrets] has its secret for its operation. The core's answer
     * comes back as Tasker's variables (several values as `%name1`, `%name2`…); a refusal as
     * `%errmsg`, in the core's words.
     */
    fun fire(core: Core, secrets: TaskerSecrets, bundle: Bundle?): Fired {
        val action = TaskerAction.from(bundle)
        if (action == null || !secrets.allows(action.secret, action.operation)) {
            return failed(automationWords().notSetUp)
        }
        val answer = try {
            core.device.automate(action.operation, action.given())
        } catch (refused: LumennaError) {
            return failed(refused.sentence)
        }
        // A write is taken in by the app, as another process's would be.
        if (automationOperations().first { it.id == action.operation }.writes) Handler(Looper.getMainLooper()).post { core.changed() }
        val variables = Bundle()
        for (value in answer.values) {
            if (value.several) {
                value.values.forEachIndexed { at, text -> variables.putString("%${value.name}${at + 1}", text) }
            } else {
                value.values.firstOrNull()?.let { variables.putString("%${value.name}", it) }
            }
        }
        return Fired(true, variables)
    }

    private fun failed(sentence: String) = Fired(false, Bundle().apply { putString("%errmsg", sentence) })

    /**
     * Tells Tasker a firing is done, when it did not wait on the broadcast itself: through the
     * completion intent it sent, as `TaskerPlugin.Setting.signalFinish` does.
     */
    fun finish(context: Context, fire: Intent, fired: Fired): Boolean {
        val uri = fire.getStringExtra(COMPLETION_INTENT) ?: return false
        val completion = runCatching { Intent.parseUri(uri, Intent.URI_INTENT_SCHEME) }.getOrNull() ?: return false
        completion.putExtra(RESULT_CODE, if (fired.ok) RESULT_OK else RESULT_FAILED)
        completion.putExtra(VARIABLES, fired.variables)
        val servicePackage = completion.getStringExtra(CALL_SERVICE_PACKAGE)
        val service = completion.getStringExtra(CALL_SERVICE)
        return runCatching {
            if (servicePackage != null && service != null && completion.hasExtra(CALL_SERVICE_FOREGROUND)) {
                completion.setClassName(servicePackage, service)
                if (completion.getBooleanExtra(CALL_SERVICE_FOREGROUND, false)) context.startForegroundService(completion)
                else context.startService(completion)
            } else {
                context.sendBroadcast(completion)
            }
            true
        }.getOrDefault(false)
    }

    /** The variables bundle Tasker reads from a fire receiver's result extras. */
    fun answer(extras: Bundle, fired: Fired) {
        extras.putBundle(VARIABLES, fired.variables)
    }
}
