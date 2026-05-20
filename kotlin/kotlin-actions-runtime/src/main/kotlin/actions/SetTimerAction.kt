package actions

import android.app.AlarmManager
import android.content.Context
import android.os.SystemClock
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import java.util.UUID
import kotlinx.serialization.Serializable
import util.AlarmStore

@Serializable
data class SetTimerInput(
    val message: String = "Action Runtime timer",
    val lengthSeconds: Int,
)

@Serializable
data class SetTimerOutput(
    val id: String,
    val triggerAtMs: Long,
    val scheduled: Boolean,
)

class SetTimerAction : Action<SetTimerInput, SetTimerOutput> {
    override suspend fun execute(input: SetTimerInput, ctx: ActionContext): SetTimerOutput {
        val context = ctx.appContext
        val alarmManager = context.getSystemService(Context.ALARM_SERVICE) as? AlarmManager
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "AlarmManager unavailable",
                retryable = true,
            )
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S) {
            if (!alarmManager.canScheduleExactAlarms()) {
                throw ActionException(
                    code = ErrorCode.PERMISSION,
                    message = "Exact alarm scheduling not permitted",
                    retryable = false,
                )
            }
        }
        val triggerAtElapsed = SystemClock.elapsedRealtime() + input.lengthSeconds * 1000L
        val triggerAtMs = System.currentTimeMillis() + input.lengthSeconds * 1000L
        val id = UUID.randomUUID().toString()
        val pendingIntent = buildAlarmIntent(context, id, input.message)
        alarmManager.setExactAndAllowWhileIdle(AlarmManager.ELAPSED_REALTIME_WAKEUP, triggerAtElapsed, pendingIntent)
        AlarmStore.add(
            context,
            AlarmEntry(id = id, triggerAtMs = triggerAtMs, message = input.message, type = "TIMER"),
        )
        return SetTimerOutput(id = id, triggerAtMs = triggerAtMs, scheduled = true)
    }
}
