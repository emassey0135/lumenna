package dev.emassey0135.lumenna

import android.app.Activity
import android.content.Context
import android.content.Intent
import dev.emassey0135.lumenna.app.Place
import dev.emassey0135.lumenna.app.ShellEvent

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

/** Where the screens follow the app once it has done a launch shortcut, on the phone and the watch alike. */
sealed interface Launched {
    /** It opened its line: quick add. */
    data object QuickAdd : Launched

    /** It went to [place], with the details of [task] where it shows a task's. */
    data class At(val place: Place?, val task: String?) : Launched
}

/** Has the app do the launch shortcut [id], and once it is still, says where it went. */
suspend fun AppModel.launch(id: String): Launched? {
    send(ShellEvent.Launch(id))
    settle()
    val shown = view.value ?: return null
    return if (shown.line != null) Launched.QuickAdd else Launched.At(shown.place, shown.details?.task?.id)
}
