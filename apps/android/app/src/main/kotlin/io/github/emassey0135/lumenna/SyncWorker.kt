package io.github.emassey0135.lumenna

import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import io.github.emassey0135.lumenna.core.LumennaException
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit

/**
 * One sync round while the app is not in front (§8), run by WorkManager within what Android
 * allows: once just after the app is left, so what was just edited is sent now, and every
 * fifteen minutes or so — longer when the phone is dozing or the app is seldom used.
 *
 * A round reaches only the devices that are running: a Mac, a daemon, a phone that is open.
 * Two phones both in the background meet through those, or when either is opened.
 */
class SyncWorker(context: Context, parameters: WorkerParameters) : Worker(context, parameters) {
    override fun doWork(): Result {
        val core = (applicationContext as LumennaApplication).core.getOrNull() ?: return Result.failure()
        return try {
            core.syncRound()
            // Anything that arrived is shown if the app is open.
            Handler(Looper.getMainLooper()).post { core.changed() }
            Result.success()
        } catch (failure: ExecutionException) {
            when (failure.cause) {
                // Another process is syncing this device already; nothing is lost.
                is LumennaException.SyncElsewhere -> Result.success()
                // No network, a peer that would not answer: the next run tries again.
                else -> Result.retry()
            }
        }
    }

    companion object {
        internal const val LEAVING = "lumenna-sync-on-leaving"
        internal const val PERIODIC = "lumenna-sync-periodic"
        private val network = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()

        /** Asks for a round every fifteen minutes, the least Android allows; once per install. */
        fun schedulePeriodic(context: Context) {
            val request = PeriodicWorkRequestBuilder<SyncWorker>(15, TimeUnit.MINUTES)
                .setConstraints(network)
                .build()
            WorkManager.getInstance(context).enqueueUniquePeriodicWork(PERIODIC, ExistingPeriodicWorkPolicy.KEEP, request)
        }

        /**
         * Asks for a round now, as the app is left. Expedited where that needs nothing more —
         * before Android 12 an expedited job runs as a foreground service with a notification,
         * so there it is an ordinary job, which still runs within moments as a rule.
         */
        fun syncOnLeaving(context: Context) {
            val request = OneTimeWorkRequestBuilder<SyncWorker>()
                .setConstraints(network)
                .apply {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
                    }
                }
                .build()
            WorkManager.getInstance(context).enqueueUniqueWork(LEAVING, ExistingWorkPolicy.REPLACE, request)
        }
    }
}
