package dev.emassey0135.lumenna

import java.util.concurrent.atomic.AtomicBoolean

/**
 * Whether the store moved, by its heads ([read]: `Device.heads`, which every write moves, this
 * device's own included): [moved] runs only when they differ from the last look. Each look runs
 * through [run], off the main thread, since reading the heads reads the store; a look asked for
 * while one is under way runs once more after it, so the last change is never missed, and a run
 * of them is one look.
 */
class Heads(private val read: () -> String?, private val run: (Runnable) -> Unit, private val moved: () -> Unit) {
    private val looking = AtomicBoolean(false)

    @Volatile
    private var last: String? = null

    fun look() {
        if (!looking.compareAndSet(false, true)) return
        run(
            Runnable {
                looking.set(false)
                val now = read() ?: return@Runnable
                val before = last
                last = now
                // The first look is where the store stood as the app opened, which everything
                // following it has read already.
                if (before != null && before != now) moved()
            },
        )
    }
}
