package example

import actions.BluetoothToggleInput
import actions.ClipboardCopyInput
import actions.ClipboardReadInput
import actions.DeviceInfoInput
import actions.FileSearchInput
import actions.ForegroundAppInput
import actions.HttpRequest
import actions.InsertCalendarEventInput
import actions.ListCalendarEventsInput
import actions.ListInstalledAppsInput
import actions.ListNotificationsInput
import actions.MediaPlayPauseInput
import actions.NowPlayingInput
import actions.OpenWebPageInput
import actions.PermissionStatusInput
import actions.PlaceCallInput
import actions.PowerStatusInput
import actions.ReadCallLogInput
import actions.ReadFileInput
import actions.ReadSmsInput
import actions.RecordAudioInput
import actions.RecordVideoInput
import actions.ScreenshotInput
import actions.SearchContactsInput
import actions.SendSmsInput
import actions.SetAlarmInput
import actions.SetSilentModeInput
import actions.SetTimerInput
import actions.SetVolumeInput
import actions.StorageInfoInput
import actions.SystemInfoInput
import actions.TakePhotoInput
import actions.WifiToggleInput
import actions.LocationInput
import actions.NetworkStatusInput
import kotlinx.serialization.serializer
import transport.serialization.JsonCodec

data class BackgroundSmokeCase(
    val actionName: String,
    val label: String,
    val payload: ByteArray,
)

fun buildBackgroundSmokeCases(
    codec: JsonCodec,
    smokeTestFilePath: String,
    nowMs: Long = System.currentTimeMillis(),
): List<BackgroundSmokeCase> {
    return listOf(
        bg(codec, "device_info", DeviceInfoInput(includeHardware = true)),
        bg(codec, "system_info", SystemInfoInput(includeStorage = true)),
        bg(codec, "network_status", NetworkStatusInput()),
        bg(codec, "power_status", PowerStatusInput()),
        bg(codec, "set_volume", SetVolumeInput(stream = "music", level = 5)),
        bg(codec, "set_silent_mode", SetSilentModeInput(mode = "normal")),
        bg(codec, "storage_info", StorageInfoInput()),
        bg(codec, "get_location", LocationInput()),
        bg(codec, "foreground_app", ForegroundAppInput()),
        bg(codec, "list_installed_apps", ListInstalledAppsInput(includeSystemApps = false)),
        bg(codec, "set_alarm", SetAlarmInput(hour = 8, minutes = 0, skipUi = true)),
        bg(codec, "set_timer", SetTimerInput(lengthSeconds = 60, skipUi = true)),
        bg(codec, "read_sms", ReadSmsInput(limit = 5)),
        bg(codec, "send_sms", SendSmsInput(address = "5550000000", body = "smoke-test-do-not-send")),
        bg(codec, "read_call_log", ReadCallLogInput(limit = 5)),
        bg(codec, "place_call", PlaceCallInput(phoneNumber = "5550000000")),
        bg(codec, "search_contacts", SearchContactsInput(query = "a", limit = 5)),
        bg(codec, "list_notifications", ListNotificationsInput()),
        bg(codec, "clipboard_copy", ClipboardCopyInput(text = "smoke clipboard")),
        bg(codec, "clipboard_read", ClipboardReadInput()),
        bg(codec, "wifi_toggle", WifiToggleInput(enabled = false)),
        bg(codec, "bluetooth_toggle", BluetoothToggleInput(enabled = false)),
        bg(codec, "media_play_pause", MediaPlayPauseInput(action = "toggle")),
        bg(codec, "media_now_playing", NowPlayingInput()),
        bg(codec, "screenshot", ScreenshotInput(timeoutMs = 5_000)),
        bg(codec, "take_photo", TakePhotoInput(lens = "back")),
        bg(codec, "record_video", RecordVideoInput(durationSeconds = 3, withAudio = false)),
        bg(codec, "record_audio", RecordAudioInput(durationSeconds = 3)),
        bg(codec, "open_webpage", OpenWebPageInput(url = "https://example.com", timeoutMs = 8_000)),
        bg(codec, "search_files", FileSearchInput(query = "smoke", limit = 10)),
        bg(
            codec,
            "list_calendar_events",
            ListCalendarEventsInput(
                startTimeMs = nowMs - 86_400_000,
                endTimeMs = nowMs + 86_400_000,
                limit = 10,
            ),
        ),
        bg(
            codec,
            "create_calendar_event",
            InsertCalendarEventInput(
                title = "Smoke background event",
                beginTimeMs = nowMs + 3_600_000,
                endTimeMs = nowMs + 7_200_000,
            ),
        ),
        bg(
            codec,
            "check_permissions",
            PermissionStatusInput(
                permissions = SmokePermissionHelper.runtimePermissionNames(),
            ),
        ),
        bg(codec, "read_file", ReadFileInput(path = smokeTestFilePath)),
        bg(codec, "http_call", HttpRequest(url = "https://example.com")),
    )
}

private inline fun <reified T> bg(codec: JsonCodec, actionName: String, input: T): BackgroundSmokeCase {
    return BackgroundSmokeCase(
        actionName = actionName,
        label = "[bg] $actionName",
        payload = codec.encode(input, serializer()),
    )
}
