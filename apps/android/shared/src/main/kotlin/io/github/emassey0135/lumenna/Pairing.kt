package io.github.emassey0135.lumenna

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.github.emassey0135.lumenna.core.LumennaException
import io.github.emassey0135.lumenna.core.PairedWith
import io.github.emassey0135.lumenna.core.PairingPrompt
import io.github.emassey0135.lumenna.core.Reach
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.concurrent.thread

/**
 * One pairing, as the phone and the watch both run it: waiting to be found, or joining with
 * a code; the words to compare; and giving up a wait to join with a code instead. The screen
 * shows [status], [code] and [asked], and calls [waitToBeFound], [join], [answer] and [cancel].
 */
class PairingSession(private val core: Core, context: Context, private val paired: () -> Unit) {
    private val context = context.applicationContext
    private val main = Handler(Looper.getMainLooper())
    private val cancelled = AtomicBoolean(false)

    /** What is happening, as a polite live region says it. */
    var status by mutableStateOf(
        "On the same network, start pairing on both devices and they find each other. " +
            "On different networks, one shows a code and the other enters it.",
    )
        private set

    /** The code this device shows while it waits. */
    var code by mutableStateOf<String?>(null)
        private set

    /** The words to compare, while they are asked. */
    var asked by mutableStateOf<List<String>?>(null)
        private set

    var running by mutableStateOf(false)
        private set

    /** Whether this device is waiting to be found, which a code can still interrupt. */
    var waiting by mutableStateOf(false)
        private set

    /** A code entered while this device waits: joined with once the wait has ended. */
    var nextCode by mutableStateOf<String?>(null)
        private set

    private var reply: ((Boolean) -> Unit)? = null

    /** Waits to be found, showing a code for another network. */
    fun waitToBeFound() = start(null)

    /**
     * Joins with `entered`, or with the code on the clipboard when it is empty — never this
     * device's own, copied while it waits. Entering a code while waiting gives up the wait,
     * then joins.
     */
    fun join(entered: String): String {
        var given = entered.trim()
        if (given.isEmpty()) {
            given = (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                .primaryClip?.getItemAt(0)?.text?.toString()?.trim().orEmpty()
            if (given == code) given = ""
        }
        when {
            given.isEmpty() -> say("Type or paste the code the other device shows.")
            running -> {
                nextCode = given
                say("Stopping the wait, then connecting with this code.")
                cancelled.set(true)
            }
            else -> start(given)
        }
        return given
    }

    /** Answers whether the words match. */
    fun answer(match: Boolean) {
        asked = null
        reply?.invoke(match)
        reply = null
    }

    /** Gives up, as leaving the screen does. */
    fun cancel() = cancelled.set(true)

    private fun say(text: String) {
        status = text
        core.say(text)
    }

    private fun start(given: String?) {
        if (running) return
        running = true
        waiting = given == null
        cancelled.set(false)
        say(if (given == null) "Opening a pairing session." else "Connecting to the other device.")
        // The pairing thread calls these and waits; the screen answers on the main thread.
        val prompt = object : PairingPrompt {
            override fun showCode(code: String) {
                main.post {
                    this@PairingSession.code = code
                    (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                        .setPrimaryClip(ClipData.newPlainText("Pairing code", code))
                    say(
                        "Waiting for the other device. On this network it finds this one by itself. On another " +
                            "network, enter this code there. Waiting up to ten minutes. The code is copied, so it can be pasted.",
                    )
                }
            }

            override fun confirm(words: List<String>): Boolean {
                val answered = CountDownLatch(1)
                var matched = false
                main.post {
                    reply = { yes ->
                        matched = yes
                        answered.countDown()
                    }
                    asked = words
                }
                answered.await()
                return matched
            }

            override fun isCancelled(): Boolean = cancelled.get()
        }
        val wifi = context.getSystemService(Context.WIFI_SERVICE) as WifiManager
        thread(name = "lumenna-pairing") {
            // Local discovery hears multicast only while the app holds this lock.
            val lock = wifi.createMulticastLock("lumenna-pairing").apply { setReferenceCounted(false); acquire() }
            val result = runCatching { core.lumenna.pair(given, Reach.INTERNET, deviceName(context), "android", prompt) }
            lock.release()
            main.post {
                running = false
                asked = null
                nextCode?.let { next ->
                    nextCode = null
                    code = null
                    start(next)
                    return@post
                }
                result.fold(
                    { done: PairedWith ->
                        core.changed()
                        core.say(sentence(done.announcement, done.notices))
                        paired()
                    },
                    { error ->
                        code = null
                        say((error as? LumennaException)?.sentence ?: error.message.orEmpty())
                    },
                )
            }
        }
    }

    companion object {
        /** What the device is called, as its settings name it. */
        fun deviceName(context: Context): String =
            android.provider.Settings.Global.getString(context.contentResolver, android.provider.Settings.Global.DEVICE_NAME)
                ?: Build.MODEL

        /** What comparing the words asks. */
        fun matchQuestion(words: List<String>) =
            "${words.joinToString(", ")}. Say yes only if the other device shows the same three words."
    }
}
