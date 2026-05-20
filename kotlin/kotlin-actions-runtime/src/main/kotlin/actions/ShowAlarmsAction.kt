package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.AlarmStore

@Serializable
data class ShowAlarmsInput(val unused: Boolean = true)

@Serializable
data class AlarmListOutput(val alarms: List<AlarmEntry>)

class ShowAlarmsAction : Action<ShowAlarmsInput, AlarmListOutput> {
    override suspend fun execute(input: ShowAlarmsInput, ctx: ActionContext): AlarmListOutput {
        val alarms = AlarmStore.load(ctx.appContext)
        return AlarmListOutput(alarms)
    }
}
