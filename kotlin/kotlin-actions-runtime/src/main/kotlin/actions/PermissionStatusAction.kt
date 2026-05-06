package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class PermissionStatusInput(val permissions: List<String>)

@Serializable
data class PermissionStatusOutput(val granted: Map<String, Boolean>)

class PermissionStatusAction : Action<PermissionStatusInput, PermissionStatusOutput> {
    override suspend fun execute(input: PermissionStatusInput, ctx: ActionContext): PermissionStatusOutput {
        val result = input.permissions.associateWith { permission ->
            PermissionUtils.hasPermission(ctx.appContext, permission)
        }
        return PermissionStatusOutput(result)
    }
}
