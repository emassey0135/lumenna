package dev.emassey0135.lumenna

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.os.Build
import android.service.quicksettings.TileService

/**
 * Energy today in Quick Settings: the core's control and launch shortcut
 * (`Launches.ENERGY_TODAY`), so the app opens on Today with the day's energy check-in asked,
 * named as the core names it. On a locked phone it asks for the unlock first, since the day is
 * behind it.
 */
class EnergyTodayTile : TileService() {
    override fun onStartListening() {
        super.onStartListening()
        val tile = qsTile ?: return
        Launches.shortcut(Launches.ENERGY_TODAY)?.let { tile.label = it.sentence }
        tile.updateTile()
    }

    @SuppressLint("StartActivityAndCollapseDeprecated")
    override fun onClick() {
        val intent = Launches.intent(this, Launches.ENERGY_TODAY)
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
