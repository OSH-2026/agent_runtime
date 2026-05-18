package runtime

import error.ErrorCode
import java.util.concurrent.CopyOnWriteArrayList

data class ActionAuditRecord(
    val requestId: String,
    val nodeId: String,
    val actionName: String,
    val success: Boolean,
    val errorCode: ErrorCode?,
    val timestampMs: Long,
)

class ActionAuditLog {
    private val events = CopyOnWriteArrayList<ActionAuditRecord>()

    fun record(
        requestId: String,
        nodeId: String,
        actionName: String,
        success: Boolean,
        errorCode: ErrorCode? = null,
    ) {
        events.add(
            ActionAuditRecord(
                requestId = requestId,
                nodeId = nodeId,
                actionName = actionName,
                success = success,
                errorCode = errorCode,
                timestampMs = System.currentTimeMillis(),
            ),
        )
    }

    fun snapshot(limit: Int = 50): List<ActionAuditRecord> {
        val size = events.size
        if (size <= limit) {
            return events.toList()
        }
        return events.takeLast(limit)
    }

    fun clear() {
        events.clear()
    }
}
