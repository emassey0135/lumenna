package dev.emassey0135.lumenna

import android.os.Handler
import android.os.Looper
import dev.emassey0135.lumenna.app.App
import dev.emassey0135.lumenna.app.Capitals
import dev.emassey0135.lumenna.app.Lines
import dev.emassey0135.lumenna.app.Shell
import dev.emassey0135.lumenna.app.ShellEvent
import dev.emassey0135.lumenna.app.Turn
import dev.emassey0135.lumenna.app.ViewModel
import dev.emassey0135.lumenna.app.SaveFile
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first

/**
 * The app itself (crates/app, through crates/ffi and BoltFFI), over this device's store:
 * what the screens draw, and where they send what the person does.
 *
 * What is in hand, what is folded, which question is open and what to say are the app's; a
 * screen draws [view] and sends events, and decides none of them. Every turn the app leaves
 * comes in on a thread of the app's own and is taken on the main thread, in order: drawn,
 * then said — held until the list has put focus where the app put it, when the app moved it
 * ([ViewModel.landing]), so the move does not cut what is said off.
 */
class AppModel(
    private val core: Core,
    /** What this device says it runs, in pairing. */
    platform: String,
) {
    private val main = Handler(Looper.getMainLooper())
    private val shown = MutableStateFlow<ViewModel?>(null)
    private var seq = 0u

    /** Turns handed over and not yet drawn. */
    private val waiting = java.util.concurrent.atomic.AtomicInteger(0)

    private val app = App(core.device, platform, Clock.locale, object : Shell {
        override fun turn(turn: Turn) {
            waiting.incrementAndGet()
            main.post {
                try {
                    take(turn)
                } finally {
                    waiting.decrementAndGet()
                }
            }
        }
    })

    private val worded = MutableStateFlow<Lines?>(null)

    /** The view's lines, in this phone's clock: every row, the day, what a pick offers. */
    val lines: StateFlow<Lines?> = worded.asStateFlow()

    /** The clock the app was last told: the phone's 12- or 24-hour setting, which can change while it runs. */
    private var clock = Clock.style

    init {
        shown.value = app.view()
        worded.value = app.lines()
        app.send(ShellEvent.Clock(clock))
        app.send(ShellEvent.Capitals(Capitals.SENTENCE))
    }

    // The app's minutes, what is now and what is due, and what another process wrote: only
    // while the app is in front, where they are shown. Away, a tick each minute made a turn
    // that woke everything following the store, for a screen nobody saw.
    private val ticking = object : Runnable {
        override fun run() {
            app.tick()
            main.postDelayed(this, 15_000)
        }
    }

    private val saving = MutableSharedFlow<SaveFile>(extraBufferCapacity = 4)

    /** Files the app made for the person to keep — a backup, an export — for a screen to save where they choose. */
    val files: SharedFlow<SaveFile> = saving.asSharedFlow()

    /** What to draw: the view model as the app last left it. */
    val view: StateFlow<ViewModel?> = shown.asStateFlow()

    private val quiet = MutableStateFlow(0u)
    private var opening = false

    /**
     * The last landing that needs no move: going to a place puts its first row in hand, and on a
     * phone TalkBack says the new screen itself, so a list lands only on a count above this.
     */
    val opened: StateFlow<UInt> = quiet.asStateFlow()

    /** Sends what the person did. */
    fun send(event: ShellEvent) {
        if (event is ShellEvent.Open) opening = true
        app.send(event)
    }

    /** What could go at [cursor] (UTF-8 bytes) in a line of [syntax] outside the app's quick add: a filter. */
    fun complete(text: String, cursor: UInt, syntax: dev.emassey0135.lumenna.app.Syntax) = app.complete(text, cursor, syntax)

    /** What a widget, tile or complication shows for the next [hours], in this device's clock. */
    fun glance(hours: UInt) = app.glance(hours)

    /** Takes in a change made elsewhere, or a new minute. */
    fun tick() = app.tick()

    /**
     * The app came to the front or left it, on the main thread: syncing and the minutes run only
     * while it is in front. Coming back, the clock is told again if the person changed it away.
     */
    fun inFront(inFront: Boolean) {
        app.inFront(inFront)
        main.removeCallbacks(ticking)
        if (!inFront) return
        if (Clock.style != clock) {
            clock = Clock.style
            app.send(ShellEvent.Clock(clock))
        }
        main.post(ticking)
    }

    /** A round with every paired device now: the app says how it went. */
    fun syncNow() = send(ShellEvent.Key("sync-now"))

    private val drawn = MutableStateFlow(0L)

    /**
     * Waits until the app is still, at most [timeout] ms: looked at again after each turn is
     * drawn, and every so often, since the store's own work ends without one.
     */
    suspend fun settle(timeout: Long = 5_000) {
        kotlinx.coroutines.withTimeoutOrNull(timeout) {
            while (!still) {
                val at = drawn.value
                kotlinx.coroutines.withTimeoutOrNull(100) { drawn.first { it != at } }
            }
        }
    }

    /**
     * Whether the app is still: nothing with the store, and every turn drawn. The tests wait on
     * it before every look at the screen, as Compose waits for its own work.
     */
    val still: Boolean get() = waiting.get() == 0 && app.isStill()

    private fun take(turn: Turn) {
        if (turn.seq <= seq) return
        seq = turn.seq
        val was = shown.value
        val before = was?.landing
        if (opening && before != null && turn.view.landing > before) {
            opening = false
            quiet.value = turn.view.landing
        }
        shown.value = turn.view
        worded.value = turn.lines
        // What follows the store — reminders' alarms, the widgets — looks again if the store moved.
        core.turned()
        // Focus moves first, then this is said: the list that lands releases it — the main one,
        // or a list beside it that the change was made from.
        val sideLanded = was != null && (
            turn.view.sidebarHeld.landing > was.sidebarHeld.landing ||
                turn.view.remindersHeld.landing > was.remindersHeld.landing ||
                turn.view.devicesHeld.landing > was.devicesHeld.landing
            )
        if (sideLanded || (before != null && turn.view.landing > maxOf(before, quiet.value))) {
            core.hold()
            // No list showing to land: say it anyway, a moment on.
            main.postDelayed({ core.release() }, 1500)
        }
        turn.files.forEach { saving.tryEmit(it) }
        turn.said.forEach(core::report)
        drawn.value += 1
    }
}
