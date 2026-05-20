package actions

import android.content.Context
import androidx.core.app.NotificationManagerCompat
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import runtime.ActionNotificationListenerService

@Serializable
data class NotificationEntry(
    val key: String,
    val packageName: String,
    val title: String,
    val text: String,
    val postTimeMs: Long,
    val isOngoing: Boolean,
)

@Serializable
data class ListNotificationsInput(val includeOngoing: Boolean = true)

@Serializable
data class ListNotificationsOutput(val notifications: List<NotificationEntry>)

class ListNotificationsAction : Action<ListNotificationsInput, ListNotificationsOutput> {
    override suspend fun execute(
        input: ListNotificationsInput,
        ctx: ActionContext,
    ): ListNotificationsOutput {
        val context = ctx.appContext
        if (!hasListenerAccess(context)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "Notification listener access required",
                retryable = false,
            )
        }
        val list = ActionNotificationListenerService.getSnapshot()
            .filter { input.includeOngoing || !it.isOngoing }
        return ListNotificationsOutput(list)
    }
}

private fun hasListenerAccess(context: Context): Boolean {
    val enabled = NotificationManagerCompat.getEnabledListenerPackages(context)
    return enabled.contains(context.packageName)
}
