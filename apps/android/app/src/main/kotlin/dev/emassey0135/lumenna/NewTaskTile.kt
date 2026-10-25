package dev.emassey0135.lumenna

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.os.Build
import android.service.quicksettings.TileService

/**
 * New Task in Quick Settings: the core's launch shortcut (`Launches.NEW_TASK`), so the app opens
 * on quick add, from wherever the person is, named as the core names it. On a locked phone it
 * asks for the unlock first, since the task list is behind it.
 */
class NewTaskTile : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        val tile = qsTile ?: return
        Launches.shortcut(Launches.NEW_TASK)?.let { tile.label = it.sentence }
        tile.updateTile()
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    override fun onClick() {
        val intent = Launches.intent(this, Launches.NEW_TASK)
        val open = {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                startActivityAndCollapse(
                    PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
                )
            } else {
                @Suppress("DEPRECATION")
                startActivityAndCollapse(intent)
            }
        }
        if (isLocked) unlockAndRun(open) else open()
    }
}
