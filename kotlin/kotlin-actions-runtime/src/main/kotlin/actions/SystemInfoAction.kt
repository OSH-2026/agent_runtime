package actions

import android.os.Build
import android.os.Environment
import android.os.StatFs
import android.os.SystemClock
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class SystemInfoInput(val includeStorage: Boolean = true)

@Serializable
data class SystemInfoOutput(
    val brand: String,
    val model: String,
    val sdkInt: Int,
    val release: String,
    val uptimeMs: Long,
    val screenWidthPx: Int,
    val screenHeightPx: Int,
    val densityDpi: Int,
    val internalTotalBytes: Long,
    val internalAvailableBytes: Long,
)

class SystemInfoAction : Action<SystemInfoInput, SystemInfoOutput> {
    override suspend fun execute(input: SystemInfoInput, ctx: ActionContext): SystemInfoOutput {
        val metrics = ctx.appContext.resources.displayMetrics
        val stat = StatFs(Environment.getDataDirectory().absolutePath)
        return SystemInfoOutput(
            brand = Build.BRAND ?: "",
            model = Build.MODEL ?: "",
            sdkInt = Build.VERSION.SDK_INT,
            release = Build.VERSION.RELEASE ?: "",
            uptimeMs = SystemClock.uptimeMillis(),
            screenWidthPx = metrics.widthPixels,
            screenHeightPx = metrics.heightPixels,
            densityDpi = metrics.densityDpi,
            internalTotalBytes = stat.totalBytes,
            internalAvailableBytes = stat.availableBytes,
        )
    }
}
