package dev.emassey0135.lumenna.tasker

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import dev.emassey0135.lumenna.CoreHolder
import dev.emassey0135.lumenna.app.storeUnopened
import kotlin.concurrent.thread

/**
 * Tasker firing an action set up in Lumenna (Locale's `FIRE_SETTING`). The core runs it off the
 * main thread while the broadcast is held open (`goAsync`), so no service or notification is
 * needed: Tasker waits on the ordered broadcast and reads its result code and variables from the
 * result extras. Fired otherwise, the answer goes back through Tasker's completion intent.
 *
 * Exported, as Tasker must reach it, so any app can send it: [Tasker.fire] runs nothing whose
 * secret was not set up here.
 */
class TaskerFireReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val ordered = isOrderedBroadcast
        val pending = goAsync()
        val bundle = intent.getBundleExtra(Tasker.EXTRA_BUNDLE)
        val application = context.applicationContext
        thread(name = "Tasker action") {
            val fired = (application as CoreHolder).core.fold(
                onSuccess = { Tasker.fire(it, TaskerSecrets(application), bundle) },
                onFailure = { Fired(false, android.os.Bundle().apply { putString("%errmsg", storeUnopened(it.message.orEmpty())) }) },
            )
            try {
                if (ordered) {
                    pending.resultCode = if (fired.ok) Tasker.RESULT_OK else Tasker.RESULT_FAILED
                    Tasker.answer(pending.getResultExtras(true), fired)
                } else {
                    Tasker.finish(application, intent, fired)
                }
            } finally {
                pending.finish()
            }
        }
    }
}
