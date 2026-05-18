package actions

import kotlinx.serialization.serializer
import runtime.ActionRegistry

fun ActionRegistry.registerIntentActions() {
    register("intent_set_alarm", SetAlarmAction(), serializer<SetAlarmInput>(), serializer<LaunchResult>())
    register("intent_set_timer", SetTimerAction(), serializer<SetTimerInput>(), serializer<LaunchResult>())
    register("intent_show_alarms", ShowAlarmsAction(), serializer<ShowAlarmsInput>(), serializer<LaunchResult>())
    register(
        "intent_insert_calendar",
        InsertCalendarEventAction(),
        serializer<InsertCalendarEventInput>(),
        serializer<LaunchResult>(),
    )
    register(
        "intent_capture_return",
        CaptureImageReturnAction(),
        serializer<CaptureImageReturnInput>(),
        serializer<IntentActivityResult>(),
    )
    register("intent_camera_still", CameraStillAction(), serializer<CameraStillInput>(), serializer<LaunchResult>())
    register("intent_camera_video", CameraVideoAction(), serializer<CameraVideoInput>(), serializer<LaunchResult>())
    register(
        "intent_pick_contact",
        PickContactAction(),
        serializer<PickContactInput>(),
        serializer<IntentActivityResult>(),
    )
    register(
        "intent_pick_contact_data",
        PickContactDataAction(),
        serializer<PickContactDataInput>(),
        serializer<IntentActivityResult>(),
    )
    register("intent_view_contact", ViewContactAction(), serializer<ViewContactInput>(), serializer<LaunchResult>())
    register("intent_edit_contact", EditContactAction(), serializer<EditContactInput>(), serializer<LaunchResult>())
    register(
        "intent_insert_contact",
        InsertContactAction(),
        serializer<InsertContactInput>(),
        serializer<LaunchResult>(),
    )
    register("intent_compose_email", ComposeEmailAction(), serializer<ComposeEmailInput>(), serializer<LaunchResult>())
    register("intent_get_content", GetContentAction(), serializer<GetContentInput>(), serializer<IntentActivityResult>())
    register(
        "intent_open_document",
        OpenDocumentAction(),
        serializer<OpenDocumentInput>(),
        serializer<IntentActivityResult>(),
    )
    register("intent_call_car", CallCarAction(), serializer<CallCarInput>(), serializer<LaunchResult>())
    register("intent_show_map", ShowMapAction(), serializer<ShowMapInput>(), serializer<LaunchResult>())
    register("intent_play_media", PlayMediaAction(), serializer<PlayMediaInput>(), serializer<LaunchResult>())
    register(
        "intent_play_search",
        PlayMediaSearchAction(),
        serializer<PlayMediaSearchInput>(),
        serializer<LaunchResult>(),
    )
    register("intent_create_note", CreateNoteAction(), serializer<CreateNoteInput>(), serializer<LaunchResult>())
}
