package dev.emassey0135.lumenna.braille

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import android.os.IInterface
import android.os.RemoteException
import android.util.Log
import java.util.concurrent.Executors

/**
 * One bound vendor service, every call to it on one ordered worker, bound again when it dies.
 *
 * Order matters: a service's own order of arrival is the order things are said in, and a call
 * made from the main thread can jump one made from the app's thread a moment before. The service
 * can die under the app whenever the vendor updates it, and Android then never binds it again
 * ([onBindingDied]), so the link does.
 *
 * Nothing here knows a device: the Orbit app and the BrailleSense app each hold their own.
 */
class ServiceLink<S : IInterface>(
    context: Context,
    private val intent: Intent,
    private val asInterface: (IBinder) -> S,
    /** Each time the service comes up, on the worker: where a client registers its callbacks. */
    private val connected: (S) -> Unit = {},
    /** The service has gone; [connected] runs again when it comes back. On a binder thread. */
    private val lost: () -> Unit = {},
) {
    private val context = context.applicationContext
    private val worker = Executors.newSingleThreadExecutor { Thread(it, "lumenna-service") }

    @Volatile
    private var service: S? = null

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            val bound = asInterface(binder)
            service = bound
            worker.execute {
                try {
                    connected(bound)
                } catch (error: RemoteException) {
                    Log.w(TAG, "setting up the service failed", error)
                } catch (error: RuntimeException) {
                    Log.w(TAG, "setting up the service failed", error)
                }
            }
        }

        override fun onServiceDisconnected(name: ComponentName) {
            service = null
            lost()
        }

        // The service's process was replaced (an update, a force-stop): Android never reconnects
        // this binding, so a new one is made.
        override fun onBindingDied(name: ComponentName) {
            service = null
            lost()
            runCatching { this@ServiceLink.context.unbindService(this) }
            bind()
        }

        override fun onNullBinding(name: ComponentName) {
            Log.e(TAG, "$name gave no binder")
        }
    }

    fun bind() {
        if (!context.bindService(intent, connection, Context.BIND_AUTO_CREATE)) {
            Log.e(TAG, "could not bind ${intent.component}")
        }
    }

    fun unbind() {
        runCatching { context.unbindService(connection) }
        service = null
    }

    val isBound: Boolean get() = service != null

    /** Runs [block] on the worker if the service is up; a failure is logged and goes no further. */
    fun call(what: String, block: (S) -> Unit) {
        worker.execute {
            val s = service ?: return@execute
            try {
                block(s)
            } catch (error: RemoteException) {
                Log.w(TAG, "$what failed", error)
            } catch (error: RuntimeException) {
                // A service throwing back across the binder (a malformed bundle, a bad state).
                Log.w(TAG, "$what failed", error)
            }
        }
    }

    /**
     * Runs [block] now, on the caller's thread: for giving up the device as the app leaves, which
     * cannot wait behind what is queued. A failure is logged and goes no further.
     */
    fun now(what: String, block: (S) -> Unit) {
        val s = service ?: return
        try {
            block(s)
        } catch (error: RemoteException) {
            Log.w(TAG, "$what failed", error)
        } catch (error: RuntimeException) {
            Log.w(TAG, "$what failed", error)
        }
    }

    /** What [block] answers, asked on the worker and waited for; [fallback] if it cannot say. */
    fun <T> ask(fallback: T, block: (S) -> T): T {
        val answer = java.util.concurrent.FutureTask {
            val s = service
            if (s == null) fallback else try {
                block(s)
            } catch (error: RemoteException) {
                fallback
            } catch (error: RuntimeException) {
                fallback
            }
        }
        worker.execute(answer)
        return answer.get()
    }

    private companion object {
        const val TAG = "LumennaService"
    }
}
