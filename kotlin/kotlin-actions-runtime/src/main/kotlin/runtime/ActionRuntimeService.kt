package runtime

import android.app.Service
import android.content.pm.ServiceInfo
import android.content.Context
import android.content.Intent
import android.os.IBinder
import androidx.core.app.ServiceCompat
import error.ActionException
import error.ErrorCode
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.CancellableContinuation
import kotlinx.coroutines.suspendCancellableCoroutine

class ActionRuntimeService : Service() {
    private var runtime: ActionRuntime? = null
    private var runtimePort = DEFAULT_PORT
    private var mediaProjectionTypeEnabled = false

    override fun onCreate() {
        super.onCreate()
        NotificationHelper.createChannel(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (runtime == null) {
            runtimePort = intent?.getIntExtra(EXTRA_PORT, DEFAULT_PORT) ?: DEFAULT_PORT
            runtime = ActionRuntime(applicationContext, runtimePort)
                .registerDefaults()
            ServiceCompat.startForeground(
                this,
                NotificationHelper.NOTIFICATION_ID,
                NotificationHelper.buildNotification(this, runtimePort),
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )
            runtime?.start()
        }
        if (intent?.action == ACTION_ENABLE_MEDIA_PROJECTION) {
            enableMediaProjectionType()
        }
        return START_STICKY
    }

    private fun enableMediaProjectionType() {
        try {
            if (!mediaProjectionTypeEnabled) {
                ServiceCompat.startForeground(
                    this,
                    NotificationHelper.NOTIFICATION_ID,
                    NotificationHelper.buildNotification(this, runtimePort),
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC or
                        ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION,
                )
                mediaProjectionTypeEnabled = true
            }
            completeMediaProjectionPromotion()
        } catch (error: Exception) {
            failMediaProjectionPromotion(error)
        }
    }

    override fun onDestroy() {
        failMediaProjectionPromotion(
            IllegalStateException("Action runtime service stopped before media projection was enabled"),
        )
        runtime?.stop()
        runtime = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val EXTRA_PORT = "runtime_port"
        private const val DEFAULT_PORT = 8080
        private const val ACTION_ENABLE_MEDIA_PROJECTION =
            "agent.runtime.action.ENABLE_MEDIA_PROJECTION"
        private val promotionLock = Any()

        @Volatile
        private var pendingPromotion: CancellableContinuation<Unit>? = null

        fun start(context: Context, port: Int = DEFAULT_PORT) {
            val intent = Intent(context, ActionRuntimeService::class.java)
                .putExtra(EXTRA_PORT, port)
            context.startForegroundService(intent)
        }

        fun stop(context: Context) {
            val intent = Intent(context, ActionRuntimeService::class.java)
            context.stopService(intent)
        }

        suspend fun enableMediaProjection(context: Context) {
            suspendCancellableCoroutine { cont ->
                synchronized(promotionLock) {
                    if (pendingPromotion != null) {
                        cont.resumeWithException(
                            ActionException(
                                code = ErrorCode.INTERNAL,
                                message = "Media projection foreground service promotion is already in progress",
                                retryable = true,
                            ),
                        )
                        return@suspendCancellableCoroutine
                    }
                    pendingPromotion = cont
                }
                cont.invokeOnCancellation {
                    synchronized(promotionLock) {
                        if (pendingPromotion === cont) {
                            pendingPromotion = null
                        }
                    }
                }
                try {
                    val intent = Intent(context, ActionRuntimeService::class.java).apply {
                        action = ACTION_ENABLE_MEDIA_PROJECTION
                    }
                    if (context.startService(intent) == null) {
                        failMediaProjectionPromotion(
                            IllegalStateException("Unable to start action runtime service"),
                        )
                    }
                } catch (error: Exception) {
                    failMediaProjectionPromotion(error)
                }
            }
        }

        private fun completeMediaProjectionPromotion() {
            val cont = synchronized(promotionLock) {
                val current = pendingPromotion
                pendingPromotion = null
                current
            } ?: return
            cont.resume(Unit)
        }

        private fun failMediaProjectionPromotion(error: Exception) {
            val cont = synchronized(promotionLock) {
                val current = pendingPromotion
                pendingPromotion = null
                current
            } ?: return
            cont.resumeWithException(
                ActionException(
                    code = ErrorCode.INTERNAL,
                    message = "Failed to enable media projection foreground service: ${error.message}",
                    retryable = false,
                    cause = error,
                ),
            )
        }
    }
}
