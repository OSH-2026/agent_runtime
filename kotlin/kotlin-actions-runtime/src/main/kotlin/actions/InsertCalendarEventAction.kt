package actions

import android.content.Intent
import android.provider.CalendarContract
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class InsertCalendarEventInput(
    val title: String,
    val description: String = "",
    val location: String = "",
    val beginTimeMs: Long,
    val endTimeMs: Long,
)

class InsertCalendarEventAction : Action<InsertCalendarEventInput, LaunchResult> {
    override suspend fun execute(input: InsertCalendarEventInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_INSERT).apply {
            data = CalendarContract.Events.CONTENT_URI
            putExtra(CalendarContract.Events.TITLE, input.title)
            putExtra(CalendarContract.Events.DESCRIPTION, input.description)
            putExtra(CalendarContract.Events.EVENT_LOCATION, input.location)
            putExtra(CalendarContract.EXTRA_EVENT_BEGIN_TIME, input.beginTimeMs)
            putExtra(CalendarContract.EXTRA_EVENT_END_TIME, input.endTimeMs)
        }
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
