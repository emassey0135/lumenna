package dev.emassey0135.lumenna

import android.content.ComponentName
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import dev.emassey0135.lumenna.app.LumennaError
import dev.emassey0135.lumenna.app.TimerControl
import dev.emassey0135.lumenna.app.controls
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/**
 * The timer in Quick Settings: the core's control (`controls()`, `Device.timerControl`), on while
 * a timer runs, and a press the core's (`Device.toggleTimer`): it stops the timer running, else
 * starts on What Should I Work On?'s first choice. Nothing here decides what a press starts.
 *
 * A press works on a locked phone: starting or stopping a timer writes only the timer, and is
 * undone as any change is. What a task is called is behind the lock, so while a secure lock
 * screen shows, the tile says only what a press does, never the task, and a press is told by the
 * tile's state alone.
 */
class TimerTile : TileService() {
    private val main = Handler(Looper.getMainLooper())

    /** What the last press said, shown until Quick Settings closes. */
    private var said: String? = null

    /** Follows the store while the tile is shown, so a change from elsewhere redraws it. */
    private var following: Job? = null

    override fun onStartListening() {
        super.onStartListening()
        draw()
        val core = core() ?: return
        following = CoroutineScope(Dispatchers.Main).launch { core.changes.drop(1).collect { draw() } }
    }

    override fun onStopListening() {
        following?.cancel()
        following = null
        said = null
        super.onStopListening()
    }

    override fun onClick() {
        val core = core() ?: return
        // The core reads and writes the store: never on the main thread.
        work.execute {
            val change = try {
                core.device.toggleTimer().announcement
            } catch (error: LumennaError) {
                error.sentence
            }
            core.changed()
            main.post {
                said = change
                draw()
            }
        }
    }

    private fun core(): Core? = (applicationContext as? CoreHolder)?.core?.getOrNull()

    /** Reads the control off the main thread, then draws it. */
    private fun draw() {
        val core = core() ?: return
        val locked = isSecure
        work.execute {
            val control = core.attempt { core.device.timerControl() } ?: return@execute
            main.post {
                val tile = qsTile ?: return@post
                shown(control, locked, said).apply(tile)
                tile.updateTile()
            }
        }
    }

    /** What the tile shows: its state, label, subtitle and what TalkBack says of it. */
    data class Shown(val state: Int, val label: String, val subtitle: String, val description: String) {
        /** Sets it on [tile], the subtitle where the platform has one (Android 10 and later). */
        fun apply(tile: Tile) {
            tile.state = state
            tile.label = label
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = subtitle
            tile.contentDescription = description
        }
    }

    companion object {
        private val work = Executors.newSingleThreadExecutor { Thread(it, "lumenna-timer-tile") }

        /** The core's Timer control, in Android's sentence case. */
        fun name(): String = controls().first { it.id == "timer" }.sentence

        /**
         * What [control] shows: on while a timer runs, off when nothing does, unavailable when a
         * press would do nothing. Beside it the task (or why nothing would start), else, behind a
         * secure lock, only what a press does; after a press, what the core said of it.
         */
        fun shown(control: TimerControl, locked: Boolean, said: String? = null): Shown {
            val state = when {
                !control.enabled -> Tile.STATE_UNAVAILABLE
                control.running -> Tile.STATE_ACTIVE
                else -> Tile.STATE_INACTIVE
            }
            val detail = if (locked) control.actionSentence else said ?: control.detail
            val description = if (locked) listOf(name(), control.actionSentence) else listOf(name(), control.actionSentence, detail)
            return Shown(state, name(), detail, description.filter { it.isNotBlank() }.joinToString(", "))
        }

        /** Asks the system to have the tile draw itself again, though Quick Settings is closed. */
        fun refresh(context: Context) {
            runCatching { requestListeningState(context, ComponentName(context, TimerTile::class.java)) }
        }
    }
}
