package dev.emassey0135.lumenna

import android.app.Activity
import android.content.Context
import android.content.Intent

/**
 * Opening an app on one of the core's launch shortcuts (`launch_shortcuts()`): the phone's icon
 * shortcuts, its Quick Settings tile and widgets, the watch's tile and complications. The app is
 * sent the shortcut's `id` as `ShellEvent.Launch` and does it.
 */
object Launching {
    /** Opens the app on a launch shortcut, its `id` in [ID]. */
    const val ACTION = "dev.emassey0135.lumenna.LAUNCH"

    /** The launch shortcut's `id`. */
    const val ID = "dev.emassey0135.lumenna.launch"

    /** Today's `id`, which a glance opens when tapped. */
    const val TODAY = "today"

    /** The intent that opens [activity] on the launch shortcut [id]. */
    fun intent(context: Context, activity: Class<out Activity>, id: String): Intent = Intent(ACTION)
        .setClass(context, activity)
        .putExtra(ID, id)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)

    /**
     * The launch shortcut an intent asks for, if it asks for one: by its extra, whatever the
     * action, since a tile's launch names no action of its own.
     */
    fun id(intent: Intent?): String? = intent?.getStringExtra(ID)
}
