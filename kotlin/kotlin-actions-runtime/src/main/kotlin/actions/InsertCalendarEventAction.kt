package actions

import android.content.ContentValues
import android.provider.CalendarContract
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class InsertCalendarEventInput(
    val title: String,
    val description: String = "",
    val location: String = "",
    val beginTimeMs: Long,
    val endTimeMs: Long,
    val calendarId: Long? = null,
    val timeZone: String = "UTC",
)

@Serializable
data class InsertCalendarEventOutput(
    val eventId: Long,
    val created: Boolean,
)

class InsertCalendarEventAction : Action<InsertCalendarEventInput, InsertCalendarEventOutput> {
    override suspend fun execute(input: InsertCalendarEventInput, ctx: ActionContext): InsertCalendarEventOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.WRITE_CALENDAR)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "WRITE_CALENDAR permission required",
                retryable = false,
            )
        }
        val calendarId = input.calendarId ?: findPrimaryCalendarId(context)
            ?: throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "No writable calendar found",
                retryable = false,
            )
        val values = ContentValues().apply {
            put(CalendarContract.Events.CALENDAR_ID, calendarId)
            put(CalendarContract.Events.TITLE, input.title)
            put(CalendarContract.Events.DESCRIPTION, input.description)
            put(CalendarContract.Events.EVENT_LOCATION, input.location)
            put(CalendarContract.Events.DTSTART, input.beginTimeMs)
            put(CalendarContract.Events.DTEND, input.endTimeMs)
            put(CalendarContract.Events.EVENT_TIMEZONE, input.timeZone)
        }
        val uri = context.contentResolver.insert(CalendarContract.Events.CONTENT_URI, values)
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "Failed to insert calendar event",
                retryable = true,
            )
        val id = uri.lastPathSegment?.toLongOrNull() ?: 0L
        return InsertCalendarEventOutput(eventId = id, created = id > 0)
    }
}

private fun findPrimaryCalendarId(context: android.content.Context): Long? {
    val projection = arrayOf(CalendarContract.Calendars._ID, CalendarContract.Calendars.IS_PRIMARY)
    context.contentResolver.query(
        CalendarContract.Calendars.CONTENT_URI,
        projection,
        "${CalendarContract.Calendars.VISIBLE} = 1",
        null,
        null,
    )?.use { cursor ->
        val idIndex = cursor.getColumnIndex(CalendarContract.Calendars._ID)
        val primaryIndex = cursor.getColumnIndex(CalendarContract.Calendars.IS_PRIMARY)
        var fallbackId: Long? = null
        while (cursor.moveToNext()) {
            val id = cursor.getLong(idIndex)
            val isPrimary = if (primaryIndex >= 0) cursor.getInt(primaryIndex) == 1 else false
            if (isPrimary) return id
            if (fallbackId == null) fallbackId = id
        }
        return fallbackId
    }
    return null
}
