package actions

import android.content.Intent
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class CallCarInput(val unused: Boolean = true)

class CallCarAction : Action<CallCarInput, LaunchResult> {
    override suspend fun execute(input: CallCarInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(ACTION_RESERVE_TAXI_RESERVATION)
        return IntentLauncher.launch(ctx.appContext, intent)
    }

    companion object {
        private const val ACTION_RESERVE_TAXI_RESERVATION =
            "com.google.android.gms.actions.RESERVE_TAXI_RESERVATION"
    }
}
