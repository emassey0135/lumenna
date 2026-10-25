package dev.emassey0135.lumenna.widgets

import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.widget.RemoteViews
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.datastore.preferences.core.longPreferencesKey
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.ImageProvider
import androidx.glance.appwidget.components.FilledButton
import androidx.glance.action.ActionParameters
import androidx.glance.action.actionParametersOf
import androidx.glance.action.clickable
import androidx.glance.appwidget.AndroidRemoteViews
import androidx.glance.Button
import androidx.glance.layout.Alignment
import androidx.glance.layout.Row
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetManager
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.action.actionStartActivity
import androidx.glance.appwidget.provideContent
import androidx.glance.appwidget.state.updateAppWidgetState
import androidx.glance.background
import androidx.glance.currentState
import androidx.glance.layout.Column
import androidx.glance.layout.Spacer
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.fillMaxWidth
import androidx.glance.layout.height
import androidx.glance.layout.padding
import androidx.glance.semantics.contentDescription
import androidx.glance.semantics.semantics
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import dev.emassey0135.lumenna.Clock
import dev.emassey0135.lumenna.Core
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.Glances
import dev.emassey0135.lumenna.Launches
import dev.emassey0135.lumenna.Launching
import dev.emassey0135.lumenna.R
import dev.emassey0135.lumenna.app.Argument
import dev.emassey0135.lumenna.app.ArgumentValue
import dev.emassey0135.lumenna.app.GlanceEntry
import dev.emassey0135.lumenna.app.GlancePart
import dev.emassey0135.lumenna.app.glanceWidgets
import dev.emassey0135.lumenna.app.GlanceTimer
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.sentence
import java.time.Duration
import java.time.Instant
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/**
 * The home screen's widgets, drawn from the core's glance (`Glances`): the parts the core gives
 * each (`glanceWidgets()`), in its order, and nothing decided here. Each is one TalkBack stop named
 * by the glance's `spoken` sentence, opening Today when tapped; the larger one's tasks are each the
 * core's line with the core's Mark done beside it, which marks the task done through the core's
 * assistant operation, as Gemini would, and its last part is New Task, a button of its own. A
 * widget too small for every part loses the last first.
 */
object Widgets {
    /** Moves whenever the glance may have changed, so a widget being shown composes again. */
    val REFRESH = longPreferencesKey("refresh")

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    /** Has every widget on the home screen draw the glance again. */
    fun refresh(context: Context) {
        val app = context.applicationContext
        scope.launch {
            val manager = GlanceAppWidgetManager(app)
            for (widget in listOf(NowWidget(), TodayWidget())) {
                for (id in manager.getGlanceIds(widget.javaClass)) {
                    updateAppWidgetState(app, id) { it[REFRESH] = System.currentTimeMillis() }
                    widget.update(app, id)
                }
            }
        }
    }

    /** The store, if it opened. */
    fun core(context: Context): Core? = (context.applicationContext as? CoreHolder)?.core?.getOrNull()

    /**
     * The glance true now, if the store opened. One that cannot be worked out, or has no moment,
     * is the core's empty day rather than a sentence of ours.
     */
    fun entry(context: Context): GlanceEntry? {
        val core = core(context) ?: return null
        return runCatching { Glances.now(Glances.entries(context, core)) }.getOrNull()
            ?: dev.emassey0135.lumenna.app.glanceEmpty(Clock.locale)
    }

    val text = ColorProvider(R.color.widget_text)
    val quiet = ColorProvider(R.color.widget_quiet)
    val background = ColorProvider(R.color.widget_background)

    /** Opens the app on Today, the core's launch shortcut. */
    fun openToday(context: Context) = actionStartActivity(Launches.intent(context, Launching.TODAY))

    /** The parts the core's widget called [id] shows, in order. */
    fun parts(id: String): List<GlancePart> = glanceWidgets().firstOrNull { it.id == id }?.parts.orEmpty()

    /** What New Task opens: the app on quick add, as its Quick Settings tile does. */
    fun newTask(context: Context): Intent = Launches.intent(context, Launches.NEW_TASK)

    /** New Task's words, the core's launch shortcut's, in sentence case. */
    fun newTaskWords(): String = Launches.shortcut(Launches.NEW_TASK)?.sentence.orEmpty()
}

/** The widget's frame: the background, Today when tapped, and the glance said as one sentence. */
@Composable
private fun Frame(context: Context, entry: GlanceEntry?, content: @Composable () -> Unit) {
    Column(
        GlanceModifier.fillMaxSize().background(Widgets.background).padding(12.dp)
            .clickable(Widgets.openToday(context))
            .semantics { entry?.let { contentDescription = it.spoken } },
    ) { content() }
}

@Composable
private fun Headline(entry: GlanceEntry) {
    Text(entry.headline.title, style = TextStyle(color = Widgets.text, fontSize = 16.sp, fontWeight = FontWeight.Bold), maxLines = 2)
    if (entry.headline.detail.isNotEmpty()) {
        Text(entry.headline.detail, style = TextStyle(color = Widgets.quiet, fontSize = 14.sp), maxLines = 2)
    }
}

/**
 * A running timer, counted by the platform's chronometer from the core's `countingFrom`: the
 * chronometer's base is in elapsed real time, so the time since is taken from now.
 */
@Composable
private fun Timer(context: Context, timer: GlanceTimer) {
    val since = Glances.instant(timer.countingFrom) ?: return
    val base = SystemClock.elapsedRealtime() - Duration.between(since, Instant.now()).toMillis()
    val views = RemoteViews(context.packageName, R.layout.widget_timer).apply { setChronometer(R.id.timer, base, null, true) }
    AndroidRemoteViews(views)
}

/** Says the store could not be opened, in the core's words, as the app does. */
@Composable
private fun Unopened() {
    Text(dev.emassey0135.lumenna.app.storeUnopened(""), style = TextStyle(color = Widgets.text))
}

/** The small widget: what is happening now, and the timer running. */
class NowWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        provideContent {
            val refreshed = currentState(Widgets.REFRESH)
            val entry = remember(refreshed) { Widgets.entry(context) }
            Frame(context, entry) {
                if (entry == null) return@Frame Unopened()
                Parts(context, entry, Widgets.parts("now"))
            }
        }
    }
}

/** The larger widget: what is happening now, the next block, and the tasks due, each to mark done. */
class TodayWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        provideContent {
            val refreshed = currentState(Widgets.REFRESH)
            val entry = remember(refreshed) { Widgets.entry(context) }
            Frame(context, entry) {
                if (entry == null) return@Frame Unopened()
                Parts(context, entry, Widgets.parts("today"))
            }
        }
    }
}

/** The [parts] of the glance a widget shows, each as the core words it, in the core's order. */
@Composable
private fun Parts(context: Context, entry: GlanceEntry, parts: List<GlancePart>) {
    for (part in parts) {
        when (part) {
            GlancePart.HEADLINE -> {
                Headline(entry)
                entry.moment.timer?.let { Timer(context, it) }
            }
            GlancePart.NEXT -> entry.next?.let {
                Spacer(GlanceModifier.height(8.dp))
                Text(it.text, style = TextStyle(color = Widgets.text, fontSize = 14.sp), maxLines = 1)
            }
            GlancePart.DUE -> {
                Spacer(GlanceModifier.height(8.dp))
                Text(entry.due.text, style = TextStyle(color = Widgets.quiet, fontSize = 14.sp), maxLines = 1)
            }
            GlancePart.TASKS -> Tasks(entry)
            GlancePart.NEW_TASK -> NewTask(context)
        }
    }
}

/**
 * Each task as the core words it, and its button the core's Mark done, named with the task's
 * title so TalkBack says which task it marks.
 */
@Composable
private fun Tasks(entry: GlanceEntry) {
    for ((task, line) in entry.moment.tasks.zip(entry.tasks)) {
        Row(GlanceModifier.fillMaxWidth().padding(top = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            Column(GlanceModifier.defaultWeight()) {
                Text(line.title, style = TextStyle(color = Widgets.text, fontSize = 14.sp), maxLines = 1)
                if (line.detail.isNotEmpty()) {
                    Text(line.detail, style = TextStyle(color = Widgets.quiet, fontSize = 12.sp), maxLines = 1)
                }
            }
            Button(
                text = entry.markDoneSentence,
                onClick = actionRunCallback<MarkDone>(actionParametersOf(MarkDone.TASK to task.id)),
                modifier = GlanceModifier.semantics { contentDescription = "${entry.markDoneSentence}, ${line.title}" },
            )
        }
    }
}

/** New Task, opening quick add: a button of its own in the core's words, with a plus. */
@Composable
private fun NewTask(context: Context) {
    val words = Widgets.newTaskWords()
    Spacer(GlanceModifier.height(8.dp))
    FilledButton(
        text = words,
        onClick = actionStartActivity(Widgets.newTask(context)),
        icon = ImageProvider(R.drawable.ic_new_task),
        modifier = GlanceModifier.semantics { contentDescription = words },
    )
}

/**
 * A task's Mark done button on the larger widget: the core's Complete a Task, as an assistant runs it
 * (`Device.assist`), then the app takes the change in and every glance is drawn again.
 */
class MarkDone : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val task = parameters[TASK] ?: return
        val core = Widgets.core(context) ?: return
        val main = Handler(Looper.getMainLooper())
        try {
            core.device.assist("complete_task", listOf(Argument("id", ArgumentValue.Text(task))))
            main.post { core.changed() }
        } catch (refused: LumennaError) {
            main.post { core.say(refused.sentence) }
        }
    }

    companion object {
        val TASK = ActionParameters.Key<String>("task")
    }
}

class NowWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = NowWidget()
}

class TodayWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = TodayWidget()
}
