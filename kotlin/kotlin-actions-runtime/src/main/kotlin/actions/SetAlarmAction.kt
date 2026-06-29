package actions

import android.content.Intent
import android.provider.AlarmClock
import api.Action
import api.ActionContext
import kotlinx.coroutines.delay
import kotlinx.serialization.Serializable
import util.IntentLauncher

private const val SET_ALARM_SETTLE_MS = 1_200L

@Serializable
data class SetAlarmInput(
    val message: String = "Action Runtime alarm",
    val hour: Int,
    val minutes: Int,
    val skipUi: Boolean = true,
)

class SetAlarmAction : Action<SetAlarmInput, LaunchResult> {
    override suspend fun execute(input: SetAlarmInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(AlarmClock.ACTION_SET_ALARM).apply {
            putExtra(AlarmClock.EXTRA_MESSAGE, input.message)
            putExtra(AlarmClock.EXTRA_HOUR, input.hour)
            putExtra(AlarmClock.EXTRA_MINUTES, input.minutes)
            putExtra(AlarmClock.EXTRA_SKIP_UI, input.skipUi)
        }
        val result = IntentLauncher.launch(ctx.appContext, intent)
        if (input.skipUi) {
            // Clock apps consume ACTION_SET_ALARM asynchronously; avoid dropping rapid batches.
            delay(SET_ALARM_SETTLE_MS)
        }
        return result
    }
}
