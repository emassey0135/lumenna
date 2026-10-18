package io.github.emassey0135.lumenna

import android.os.Handler
import android.os.Looper
import io.github.emassey0135.lumenna.app.App
import io.github.emassey0135.lumenna.app.Shell
import io.github.emassey0135.lumenna.app.ShellEvent
import io.github.emassey0135.lumenna.app.Turn
import io.github.emassey0135.lumenna.app.ViewModel
import io.github.emassey0135.lumenna.app.SaveFile
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * The app itself (crates/app, through crates/mobile and BoltFFI), over this device's store:
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

    private val app = App(core.device, platform, object : Shell {
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

    init {
        shown.value = app.view()
        app.send(ShellEvent.Clock(Clock.style))
        main.post(object : Runnable {
            // The app's minutes: what is now, what is due.
            override fun run() {
                app.tick()
                main.postDelayed(this, 15_000)
            }
        })
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
    fun complete(text: String, cursor: UInt, syntax: io.github.emassey0135.lumenna.app.Syntax) = app.complete(text, cursor, syntax)

    /** Takes in a change made elsewhere, or a new minute. */
    fun tick() = app.tick()

    /** The app came to the front or left it: syncing runs only while it is in front. */
    fun inFront(inFront: Boolean) = app.inFront(inFront)

    /**
     * Whether the app is still: nothing with the store, and every turn drawn. The tests wait on
     * it before every look at the screen, as Compose waits for its own work.
     */
    val still: Boolean get() = waiting.get() == 0 && app.isStill()

    private fun take(turn: Turn) {
        if (turn.seq <= seq) return
        seq = turn.seq
        val before = shown.value?.landing
        if (opening && before != null && turn.view.landing > before) {
            opening = false
            quiet.value = turn.view.landing
        }
        shown.value = turn.view
        // What follows the store — reminders' alarms — looks again, a moment after the last turn.
        core.moved()
        // Focus moves first, then this is said: the list that lands releases it.
        if (before != null && turn.view.landing > maxOf(before, quiet.value)) {
            core.hold()
            // No list showing to land: say it anyway, a moment on.
            main.postDelayed({ core.release() }, 1500)
        }
        turn.files.forEach { saving.tryEmit(it) }
        val date = turn.view.day?.date
        for (said in turn.said) {
            core.report(if (said.heading && date != null) "${Clock.spokenDay(date)}. ${said.text}" else said.text)
        }
    }
}
