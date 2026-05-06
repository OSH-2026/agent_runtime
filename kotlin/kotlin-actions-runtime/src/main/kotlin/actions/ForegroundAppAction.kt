package actions

import android.app.AppOpsManager
import android.app.usage.UsageStatsManager
import android.content.Context
import android.os.Process
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable

@Serializable
data class ForegroundAppInput(val lookbackMs: Long = 5 * 60 * 1000)

@Serializable
data class ForegroundAppOutput(
    val packageName: String,
    val lastTimeUsed: Long,
    val available: Boolean,
)

class ForegroundAppAction : Action<ForegroundAppInput, ForegroundAppOutput> {
    override suspend fun execute(input: ForegroundAppInput, ctx: ActionContext): ForegroundAppOutput {
        val context = ctx.appContext
        val appOps = context.getSystemService(Context.APP_OPS_SERVICE) as AppOpsManager
        val mode = appOps.unsafeCheckOpNoThrow(
            AppOpsManager.OPSTR_GET_USAGE_STATS,
            Process.myUid(),
            context.packageName,
        )
        if (mode != AppOpsManager.MODE_ALLOWED) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "PACKAGE_USAGE_STATS not granted",
                retryable = false,
            )
        }
        val usageStats = context.getSystemService(UsageStatsManager::class.java)
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "UsageStatsManager unavailable",
                retryable = true,
            )
        val end = System.currentTimeMillis()
        val begin = end - input.lookbackMs
        val stats = usageStats.queryUsageStats(
            UsageStatsManager.INTERVAL_DAILY,
            begin,
            end,
        )
        val recent = stats.maxByOrNull { it.lastTimeUsed }
        return if (recent == null) {
            ForegroundAppOutput("", 0, false)
        } else {
            ForegroundAppOutput(recent.packageName ?: "", recent.lastTimeUsed, true)
        }
    }
}
