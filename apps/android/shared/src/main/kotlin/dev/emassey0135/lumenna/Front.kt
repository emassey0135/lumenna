package dev.emassey0135.lumenna

import androidx.activity.ComponentActivity
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/**
 * The phone's and the watch's activity coming to the front and leaving it, the same on both:
 * syncing runs while it is in front, the watch's link follows the store while it is shown, and
 * leaving asks for one more round. [platform] is what this device runs: `android` or `wearos`.
 */
class Front(private val activity: ComponentActivity, private val core: Core, private val platform: String) {
    /**
     * From the activity's `onCreate`: a change here, or one from another device, goes to the
     * watch or phone near by, a moment after the last of a run of them. Only while the activity
     * is shown: in the background, each change opened a channel for nobody, and `LinkService`
     * answers the other side's link there.
     */
    @OptIn(FlowPreview::class)
    fun created() {
        activity.lifecycleScope.launch {
            activity.repeatOnLifecycle(Lifecycle.State.STARTED) {
                core.changes.drop(1).debounce(2_000).collect { WatchLink.syncNearby(activity, core, platform) }
            }
        }
    }

    /** From `onResume`: sync runs while the app is in front, as on the iPhone. */
    fun resumed() {
        Clock.update(activity)
        core.changed()
        core.startSyncing(activity)
        // The watch and the phone over their own link, while near: before Iroh's round.
        WatchLink.syncNearby(activity, core, platform)
        core.backUpIfDue()
    }

    /** From `onPause`: the service stops, and one more round is asked for, to send what was just edited. */
    fun paused() {
        core.stopSyncing()
        SyncWorker.syncOnLeaving(activity)
    }
}
