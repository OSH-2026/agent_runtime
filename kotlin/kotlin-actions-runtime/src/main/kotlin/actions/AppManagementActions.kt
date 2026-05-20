package actions

import android.content.Context
import android.content.Intent
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable

@Serializable
data class ListInstalledAppsInput(val includeSystemApps: Boolean = false)

@Serializable
data class InstalledApp(
    val packageName: String,
    val label: String,
    val isSystem: Boolean,
)

@Serializable
data class ListInstalledAppsOutput(val apps: List<InstalledApp>)

class ListInstalledAppsAction : Action<ListInstalledAppsInput, ListInstalledAppsOutput> {
    override suspend fun execute(
        input: ListInstalledAppsInput,
        ctx: ActionContext,
    ): ListInstalledAppsOutput {
        val pm = ctx.appContext.packageManager
        val apps = pm.getInstalledApplications(0)
            .asSequence()
            .filter { input.includeSystemApps || (it.flags and android.content.pm.ApplicationInfo.FLAG_SYSTEM == 0) }
            .map {
                InstalledApp(
                    packageName = it.packageName,
                    label = pm.getApplicationLabel(it)?.toString() ?: it.packageName,
                    isSystem = (it.flags and android.content.pm.ApplicationInfo.FLAG_SYSTEM) != 0,
                )
            }
            .sortedBy { it.label }
            .toList()
        return ListInstalledAppsOutput(apps)
    }
}

@Serializable
data class LaunchAppInput(val packageName: String)

class LaunchAppAction : Action<LaunchAppInput, LaunchResult> {
    override suspend fun execute(input: LaunchAppInput, ctx: ActionContext): LaunchResult {
        val context = ctx.appContext
        val pm = context.packageManager
        val launchIntent = pm.getLaunchIntentForPackage(input.packageName)
            ?: throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "No launch intent for ${input.packageName}",
                retryable = false,
            )
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(launchIntent)
        return LaunchResult(
            launched = true,
            resolvedPackage = input.packageName,
            message = "Launched ${input.packageName}",
        )
    }
}
