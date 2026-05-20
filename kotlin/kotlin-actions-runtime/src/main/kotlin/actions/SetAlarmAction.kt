package actions

import android.app.AlarmManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import java.util.Calendar
import java.util.UUID
import kotlinx.serialization.Serializable
import runtime.AlarmReceiver
import util.AlarmStore

@Serializable
data class AlarmEntry(
    val id: String,
    val triggerAtMs: Long,
    val message: String,
    val type: String,
)

@Serializable
data class SetAlarmInput(
    val message: String = "Action Runtime alarm",
    val hour: Int,
    val minutes: Int,
    val allowWhileIdle: Boolean = true,
)

@Serializable
data class SetAlarmOutput(
    val id: String,
    val triggerAtMs: Long,
    val scheduled: Boolean,
)

class SetAlarmAction : Action<SetAlarmInput, SetAlarmOutput> {
    override suspend fun execute(input: SetAlarmInput, ctx: ActionContext): SetAlarmOutput {
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
        val calendar = Calendar.getInstance().apply {
            set(Calendar.SECOND, 0)
            set(Calendar.MILLISECOND, 0)
            set(Calendar.HOUR_OF_DAY, input.hour)
            set(Calendar.MINUTE, input.minutes)
        }
        if (calendar.timeInMillis <= System.currentTimeMillis()) {
            calendar.add(Calendar.DAY_OF_YEAR, 1)
        }
        val triggerAtMs = calendar.timeInMillis
        val id = UUID.randomUUID().toString()
        val pendingIntent = buildAlarmIntent(context, id, input.message)
        if (input.allowWhileIdle) {
            alarmManager.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, triggerAtMs, pendingIntent)
        } else {
            alarmManager.setExact(AlarmManager.RTC_WAKEUP, triggerAtMs, pendingIntent)
        }
        AlarmStore.add(
            context,
            AlarmEntry(id = id, triggerAtMs = triggerAtMs, message = input.message, type = "ALARM"),
        )
        return SetAlarmOutput(id = id, triggerAtMs = triggerAtMs, scheduled = true)
    }
}

internal fun buildAlarmIntent(context: Context, id: String, message: String): PendingIntent {
    val intent = Intent(context, AlarmReceiver::class.java).apply {
        putExtra(AlarmReceiver.EXTRA_ALARM_ID, id)
        putExtra(AlarmReceiver.EXTRA_ALARM_MESSAGE, message)
    }
    return PendingIntent.getBroadcast(
        context,
        id.hashCode(),
        intent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
}
