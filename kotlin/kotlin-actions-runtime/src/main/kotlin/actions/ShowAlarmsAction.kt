package actions

import android.content.Intent
import android.provider.AlarmClock
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class ShowAlarmsInput(val unused: Boolean = true)

class ShowAlarmsAction : Action<ShowAlarmsInput, LaunchResult> {
    override suspend fun execute(input: ShowAlarmsInput, ctx: ActionContext): LaunchResult {
        return IntentLauncher.launch(ctx.appContext, Intent(AlarmClock.ACTION_SHOW_ALARMS))
    }
}
