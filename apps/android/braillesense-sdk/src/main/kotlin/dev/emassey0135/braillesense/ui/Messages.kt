package dev.emassey0135.braillesense.ui

import android.os.Handler
import android.os.Looper
import dev.emassey0135.braillesense.output.BrailleDisplay

/**
 * A message on the cells for the person's Message Display Time, then the line under it again, as
 * the device's own messages are: someone reading braille alone hears none of what is said, so what
 * an app says of a change belongs on the cells too. Any key the person presses should [clear] it
 * first, and while it is [up] the app leaves the cells to it.
 */
class Messages(
    private val display: BrailleDisplay,
    /** How long a message stays: `BrailleSettings.messageMillis`. */
    private val millis: () -> Long,
    /** What is under the message, put back when it goes: the frame in front. */
    private val under: () -> Rendered?,
    private val handler: Handler = Handler(Looper.getMainLooper()),
) {
    /** Whether a message is on the cells now. */
    var up = false
        private set

    private val restore = Runnable {
        up = false
        under()?.let(display::show)
    }

    /** Puts [text] on the cells until its time is up. It is not said: say it as well where it should be. */
    fun show(text: String) {
        display.show(Rendered(text))
        up = true
        handler.removeCallbacks(restore)
        handler.postDelayed(restore, millis())
    }

    /** Takes the message away at once, leaving the cells to whatever is shown next. */
    fun clear() {
        up = false
        handler.removeCallbacks(restore)
    }
}
