package runtime

import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder

class ActionRuntimeService : Service() {
    private var runtime: ActionRuntime? = null

    override fun onCreate() {
        super.onCreate()
        NotificationHelper.createChannel(this)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val port = intent?.getIntExtra(EXTRA_PORT, DEFAULT_PORT) ?: DEFAULT_PORT
        if (runtime == null) {
            runtime = ActionRuntime(applicationContext, port)
                .registerDefaults()
            startForeground(
                NotificationHelper.NOTIFICATION_ID,
                NotificationHelper.buildNotification(this, port),
            )
            runtime?.start()
        }
        return START_STICKY
    }

    override fun onDestroy() {
        runtime?.stop()
        runtime = null
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private const val EXTRA_PORT = "runtime_port"
        private const val DEFAULT_PORT = 8080

        fun start(context: Context, port: Int = DEFAULT_PORT) {
            val intent = Intent(context, ActionRuntimeService::class.java)
                .putExtra(EXTRA_PORT, port)
            context.startForegroundService(intent)
        }

        fun stop(context: Context) {
            val intent = Intent(context, ActionRuntimeService::class.java)
            context.stopService(intent)
        }
    }
}
