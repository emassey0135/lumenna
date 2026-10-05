package io.github.emassey0135.lumenna

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.ListenableWorker
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.testing.TestListenableWorkerBuilder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Syncing while the app is not in front: the worker, and when it is asked for. */
@RunWith(AndroidJUnit4::class)
class SyncWorkerTest {
    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    @Test
    fun aBackgroundRoundWithNoPairedDevicesSucceeds() {
        val worker = TestListenableWorkerBuilder<SyncWorker>(context.applicationContext).build()
        assertEquals(ListenableWorker.Result.success(), worker.doWork())
    }

    @Test
    fun aRoundEveryFifteenMinutesIsAskedForWhenTheProcessStarts() {
        val periodic = WorkManager.getInstance(context).getWorkInfosForUniqueWork(SyncWorker.PERIODIC).get()
        assertEquals(1, periodic.size)
        assertEquals(15 * 60 * 1000L, periodic.single().periodicityInfo?.repeatIntervalMillis)
        assertTrue(periodic.single().state != WorkInfo.State.CANCELLED)
    }

    @Test
    fun leavingTheAppAsksForOneRoundNowAndReplacesAnyWaiting() {
        SyncWorker.syncOnLeaving(context)
        SyncWorker.syncOnLeaving(context)
        val waiting = WorkManager.getInstance(context).getWorkInfosForUniqueWork(SyncWorker.LEAVING).get()
            .filter { !it.state.isFinished }
        assertTrue("at most one round waits: $waiting", waiting.size <= 1)
    }
}
