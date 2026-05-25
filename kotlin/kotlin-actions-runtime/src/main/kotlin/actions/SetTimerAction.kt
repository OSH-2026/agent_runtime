package actions

import android.content.Intent
import android.provider.AlarmClock
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class SetTimerInput(
    val message: String = "Action Runtime timer",
    val lengthSeconds: Int,
    val skipUi: Boolean = true,
)

class SetTimerAction : Action<SetTimerInput, LaunchResult> {
    override suspend fun execute(input: SetTimerInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(AlarmClock.ACTION_SET_TIMER).apply {
            putExtra(AlarmClock.EXTRA_MESSAGE, input.message)
            putExtra(AlarmClock.EXTRA_LENGTH, input.lengthSeconds)
            putExtra(AlarmClock.EXTRA_SKIP_UI, input.skipUi)
        }
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
