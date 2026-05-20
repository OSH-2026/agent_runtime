package runtime

import actions.NotificationEntry
import android.app.Notification
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification

class ActionNotificationListenerService : NotificationListenerService() {
    override fun onListenerConnected() {
        updateSnapshot(activeNotifications?.toList().orEmpty())
    }

    override fun onNotificationPosted(sbn: StatusBarNotification) {
        updateSnapshot(activeNotifications?.toList().orEmpty())
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        updateSnapshot(activeNotifications?.toList().orEmpty())
    }

    private fun updateSnapshot(list: List<StatusBarNotification>) {
        snapshot = list.map { it.toEntry() }
    }

    private fun StatusBarNotification.toEntry(): NotificationEntry {
        val extras = notification.extras
        val title = extras.getString(Notification.EXTRA_TITLE) ?: ""
        val text = extras.getCharSequence(Notification.EXTRA_TEXT)?.toString() ?: ""
        return NotificationEntry(
            key = key ?: "",
            packageName = packageName ?: "",
            title = title,
            text = text,
            postTimeMs = postTime,
            isOngoing = notification.flags and Notification.FLAG_ONGOING_EVENT != 0,
        )
    }

    companion object {
        @Volatile
        private var snapshot: List<NotificationEntry> = emptyList()

        fun getSnapshot(): List<NotificationEntry> = snapshot
    }
}
