package example

import actions.CallCarInput
import actions.CameraStillInput
import actions.CameraVideoInput
import actions.CaptureImageReturnInput
import actions.ComposeEmailInput
import actions.CreateNoteInput
import actions.GetContentInput
import actions.InsertCalendarEventInput
import actions.InsertContactInput
import actions.LaunchAppInput
import actions.OpenDocumentInput
import actions.PickContactDataInput
import actions.PickContactInput
import actions.PlayMediaInput
import actions.PlayMediaSearchInput
import actions.ScreenRecordInput
import actions.SelectFileInput
import actions.EditContactInput
import actions.ViewContactInput
import actions.SetAlarmInput
import actions.SetTimerInput
import actions.ShowAlarmsInput
import actions.ShowMapInput
import kotlinx.serialization.serializer
import transport.serialization.JsonCodec

data class IntentSmokeCase(
    val actionName: String,
    val label: String,
    val payload: ByteArray,
)

fun buildIntentSmokeCases(codec: JsonCodec, nowMs: Long = System.currentTimeMillis()): List<IntentSmokeCase> {
    return listOf(
        case(codec, "set_alarm", SetAlarmInput(hour = 7, minutes = 30, skipUi = false)),
        case(codec, "set_timer", SetTimerInput(lengthSeconds = 300, skipUi = false)),
        case(codec, "list_alarms", ShowAlarmsInput()),
        case(codec, "launch_app", LaunchAppInput(packageName = "com.android.settings")),
        case(codec, "select_file", SelectFileInput()),
        case(codec, "screen_record", ScreenRecordInput(durationSeconds = 3, withAudio = false)),
        case(
            codec,
            "intent_insert_calendar",
            InsertCalendarEventInput(
                title = "Action Runtime event",
                beginTimeMs = nowMs + 3_600_000,
                endTimeMs = nowMs + 7_200_000,
            ),
        ),
        case(codec, "intent_capture_return", CaptureImageReturnInput()),
        case(codec, "intent_camera_still", CameraStillInput()),
        case(codec, "intent_camera_video", CameraVideoInput()),
        case(codec, "intent_pick_contact", PickContactInput()),
        case(codec, "intent_pick_contact_data", PickContactDataInput()),
        case(
            codec,
            "intent_view_contact",
            ViewContactInput(contactUri = "content://com.android.contacts/contacts/1"),
        ),
        case(
            codec,
            "intent_edit_contact",
            EditContactInput(contactUri = "content://com.android.contacts/contacts/1"),
        ),
        case(codec, "intent_insert_contact", InsertContactInput()),
        case(codec, "intent_compose_email", ComposeEmailInput(to = "test@example.com", subject = "Smoke")),
        case(codec, "intent_get_content", GetContentInput()),
        case(codec, "intent_open_document", OpenDocumentInput()),
        case(codec, "intent_call_car", CallCarInput()),
        case(codec, "intent_show_map", ShowMapInput(geoUri = "geo:0,0?q=University")),
        case(
            codec,
            "intent_play_media",
            PlayMediaInput(contentUri = "content://media/external/audio/media/1"),
        ),
        case(codec, "intent_play_search", PlayMediaSearchInput(query = "test song")),
        case(codec, "intent_create_note", CreateNoteInput(title = "Smoke", text = "From Action Runtime")),
    )
}

private inline fun <reified T> case(codec: JsonCodec, actionName: String, input: T): IntentSmokeCase {
    return IntentSmokeCase(
        actionName = actionName,
        label = "[ui] $actionName",
        payload = codec.encode(input, serializer()),
    )
}
