package actions

import android.provider.CalendarContract
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class ListCalendarEventsInput(
    val startTimeMs: Long,
    val endTimeMs: Long,
    val limit: Int = 50,
)

@Serializable
data class CalendarEventItem(
    val id: Long,
    val title: String,
    val description: String,
    val location: String,
    val beginTimeMs: Long,
    val endTimeMs: Long,
)

@Serializable
data class ListCalendarEventsOutput(val events: List<CalendarEventItem>)

class ListCalendarEventsAction : Action<ListCalendarEventsInput, ListCalendarEventsOutput> {
    override suspend fun execute(
        input: ListCalendarEventsInput,
        ctx: ActionContext,
    ): ListCalendarEventsOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.READ_CALENDAR)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "READ_CALENDAR permission required",
                retryable = false,
            )
        }
        val projection = arrayOf(
            CalendarContract.Events._ID,
            CalendarContract.Events.TITLE,
            CalendarContract.Events.DESCRIPTION,
            CalendarContract.Events.EVENT_LOCATION,
            CalendarContract.Events.DTSTART,
            CalendarContract.Events.DTEND,
        )
        val selection = "${CalendarContract.Events.DTSTART} >= ? AND ${CalendarContract.Events.DTEND} <= ?"
        val selectionArgs = arrayOf(input.startTimeMs.toString(), input.endTimeMs.toString())
        val events = mutableListOf<CalendarEventItem>()
        context.contentResolver.query(
            CalendarContract.Events.CONTENT_URI,
            projection,
            selection,
            selectionArgs,
            "${CalendarContract.Events.DTSTART} ASC",
        )?.use { cursor ->
            val idIndex = cursor.getColumnIndex(CalendarContract.Events._ID)
            val titleIndex = cursor.getColumnIndex(CalendarContract.Events.TITLE)
            val descIndex = cursor.getColumnIndex(CalendarContract.Events.DESCRIPTION)
            val locIndex = cursor.getColumnIndex(CalendarContract.Events.EVENT_LOCATION)
            val startIndex = cursor.getColumnIndex(CalendarContract.Events.DTSTART)
            val endIndex = cursor.getColumnIndex(CalendarContract.Events.DTEND)
            while (cursor.moveToNext() && events.size < input.limit) {
                events.add(
                    CalendarEventItem(
                        id = cursor.getLong(idIndex),
                        title = cursor.getString(titleIndex) ?: "",
                        description = cursor.getString(descIndex) ?: "",
                        location = cursor.getString(locIndex) ?: "",
                        beginTimeMs = cursor.getLong(startIndex),
                        endTimeMs = cursor.getLong(endIndex),
                    ),
                )
            }
        }
        return ListCalendarEventsOutput(events)
    }
}
