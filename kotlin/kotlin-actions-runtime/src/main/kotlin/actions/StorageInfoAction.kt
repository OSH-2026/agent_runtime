package actions

import android.os.Environment
import android.os.StatFs
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class StorageInfoInput(val includeExternal: Boolean = true)

@Serializable
data class StorageInfoOutput(
    val internalTotalBytes: Long,
    val internalAvailableBytes: Long,
    val externalTotalBytes: Long?,
    val externalAvailableBytes: Long?,
)

class StorageInfoAction : Action<StorageInfoInput, StorageInfoOutput> {
    override suspend fun execute(input: StorageInfoInput, ctx: ActionContext): StorageInfoOutput {
        val internalStat = StatFs(Environment.getDataDirectory().absolutePath)
        val internalTotal = internalStat.totalBytes
        val internalAvailable = internalStat.availableBytes

        val externalDir = ctx.appContext.getExternalFilesDir(null)
        val externalStat = externalDir?.let { StatFs(it.absolutePath) }

        return StorageInfoOutput(
            internalTotalBytes = internalTotal,
            internalAvailableBytes = internalAvailable,
            externalTotalBytes = externalStat?.totalBytes,
            externalAvailableBytes = externalStat?.availableBytes,
        )
    }
}
