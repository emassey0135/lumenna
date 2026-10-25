package dev.emassey0135.lumenna

import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import dev.emassey0135.lumenna.app.LaunchShortcut
import dev.emassey0135.lumenna.app.launchShortcuts

/**
 * The ways into Lumenna from outside it (`launch_shortcuts()`, `records::launch`): the app
 * icon's shortcuts, which TalkBack lists among the icon's actions, in the core's order and
 * words, set at run time so they never differ from it; and what the Quick Settings tile and a
 * widget open. Each opens [MainActivity] with its `id`, which the app is sent as
 * `ShellEvent.Launch` and does.
 */
object Launches {
    /** Opens the app on a launch shortcut, its `id` in [ID]. */
    const val LAUNCH = "dev.emassey0135.lumenna.LAUNCH"

    /** The launch shortcut's `id`. */
    const val ID = "dev.emassey0135.lumenna.launch"

    /** The intent that opens the app on the launch shortcut [id]. */
    fun intent(context: Context, id: String): Intent = Intent(LAUNCH)
        .setClass(context, MainActivity::class.java)
        .putExtra(ID, id)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)

    /**
     * The launch shortcut an intent asks for, if it asks for one: this one's, or New Task as the
     * launcher's shortcut asked for it before the core listed them, which a shortcut pinned then
     * still does.
     */
    fun id(intent: Intent?): String? = when (intent?.action) {
        LAUNCH -> intent.getStringExtra(ID)
        MainActivity.NEW_TASK -> NEW_TASK
        else -> null
    }

    /** New Task's `id`, which the Quick Settings tile opens. */
    const val NEW_TASK = "new-task"

    /** The core's shortcut called [id]. */
    fun shortcut(id: String): LaunchShortcut? = launchShortcuts().firstOrNull { it.id == id }

    /** Gives the launcher the core's shortcuts, replacing what it had. */
    fun publish(context: Context) {
        val shortcuts = launchShortcuts().mapIndexed { rank, shortcut ->
            ShortcutInfoCompat.Builder(context, shortcut.id)
                .setShortLabel(shortcut.sentence)
                .setLongLabel(shortcut.sentence)
                .setIcon(IconCompat.createWithResource(context, icon(shortcut.icon)))
                .setIntent(intent(context, shortcut.id))
                .setRank(rank)
                .build()
        }
        // A shortcut pinned from the file this replaced keeps its id, and the launcher may refuse
        // to let it be changed: the rest are still given.
        runCatching { ShortcutManagerCompat.setDynamicShortcuts(context, shortcuts) }
    }

    /** The core's generic icon name as this app draws it. */
    private fun icon(name: String): Int = when (name) {
        "today" -> R.drawable.ic_today
        "next" -> R.drawable.ic_next
        else -> R.drawable.ic_new_task
    }
}
