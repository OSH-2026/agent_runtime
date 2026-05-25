package example

import actions.BluetoothToggleOutput
import actions.ClipboardCopyOutput
import actions.ClipboardReadOutput
import actions.DeviceInfoOutput
import actions.FileSearchOutput
import actions.ForegroundAppOutput
import actions.HttpResponse
import actions.InsertCalendarEventOutput
import actions.IntentActivityResult
import actions.LaunchResult
import actions.ListCalendarEventsOutput
import actions.ListInstalledAppsOutput
import actions.ListNotificationsOutput
import actions.LocationOutput
import actions.MediaPlayPauseOutput
import actions.NetworkStatusOutput
import actions.NowPlayingOutput
import actions.OpenWebPageOutput
import actions.PermissionStatusOutput
import actions.PlaceCallOutput
import actions.PowerStatusOutput
import actions.ReadCallLogOutput
import actions.ReadFileOutput
import actions.ReadSmsOutput
import actions.RecordAudioOutput
import actions.RecordVideoOutput
import actions.ScreenRecordOutput
import actions.ScreenshotOutput
import actions.SearchContactsOutput
import actions.SelectFileOutput
import actions.SendSmsOutput
import actions.SetSilentModeOutput
import actions.SetVolumeOutput
import actions.StorageInfoOutput
import actions.SystemInfoOutput
import actions.TakePhotoOutput
import actions.WifiToggleOutput
import kotlinx.serialization.serializer
import transport.serialization.JsonCodec

object SmokeResultFormatter {

    fun format(codec: JsonCodec, actionName: String, bytes: ByteArray): String {
        return try {
            when (actionName) {
                "device_info" -> fmt(codec.decode(bytes, serializer<DeviceInfoOutput>())) { "brand=${it.brand} model=${it.model}" }
                "system_info" -> fmt(codec.decode(bytes, serializer<SystemInfoOutput>())) { "sdk=${it.sdkInt} uptime=${it.uptimeMs}" }
                "network_status" -> fmt(codec.decode(bytes, serializer<NetworkStatusOutput>())) { "connected=${it.connected}" }
                "power_status" -> fmt(codec.decode(bytes, serializer<PowerStatusOutput>())) { "pct=${it.batteryPercent} charging=${it.charging}" }
                "set_volume" -> fmt(codec.decode(bytes, serializer<SetVolumeOutput>())) { "${it.stream}=${it.level}" }
                "set_silent_mode" -> fmt(codec.decode(bytes, serializer<SetSilentModeOutput>())) { "mode=${it.mode}" }
                "storage_info" -> fmt(codec.decode(bytes, serializer<StorageInfoOutput>())) { "avail=${it.internalAvailableBytes}" }
                "get_location" -> fmt(codec.decode(bytes, serializer<LocationOutput>())) { "lat=${it.latitude} lon=${it.longitude}" }
                "foreground_app" -> fmt(codec.decode(bytes, serializer<ForegroundAppOutput>())) { "pkg=${it.packageName}" }
                "list_installed_apps" -> fmt(codec.decode(bytes, serializer<ListInstalledAppsOutput>())) { "count=${it.apps.size}" }
                "launch_app", "intent_view_contact", "intent_edit_contact", "intent_insert_contact",
                "intent_compose_email", "intent_call_car", "intent_show_map", "intent_play_media",
                "intent_play_search", "intent_create_note",
                -> fmt(codec.decode(bytes, serializer<LaunchResult>())) { "launched=${it.launched} pkg=${it.resolvedPackage} ${it.message}" }
                    "set_alarm", "intent_set_alarm" -> fmt(codec.decode(bytes, serializer<LaunchResult>())) { "launched=${it.launched} pkg=${it.resolvedPackage ?: ""}" }
                    "set_timer", "intent_set_timer" -> fmt(codec.decode(bytes, serializer<LaunchResult>())) { "launched=${it.launched} pkg=${it.resolvedPackage ?: ""}" }
                    "list_alarms", "intent_show_alarms" -> fmt(codec.decode(bytes, serializer<LaunchResult>())) { "launched=${it.launched} pkg=${it.resolvedPackage ?: ""}" }
                "read_sms" -> fmt(codec.decode(bytes, serializer<ReadSmsOutput>())) { "messages=${it.messages.size}" }
                "send_sms" -> fmt(codec.decode(bytes, serializer<SendSmsOutput>())) { "sent=${it.sent}" }
                "read_call_log" -> fmt(codec.decode(bytes, serializer<ReadCallLogOutput>())) { "calls=${it.calls.size}" }
                "place_call" -> fmt(codec.decode(bytes, serializer<PlaceCallOutput>())) { "placed=${it.placed}" }
                "search_contacts" -> fmt(codec.decode(bytes, serializer<SearchContactsOutput>())) { "results=${it.results.size}" }
                "list_notifications" -> fmt(codec.decode(bytes, serializer<ListNotificationsOutput>())) { "count=${it.notifications.size}" }
                "clipboard_copy" -> fmt(codec.decode(bytes, serializer<ClipboardCopyOutput>())) { "copied=${it.copied}" }
                "clipboard_read" -> fmt(codec.decode(bytes, serializer<ClipboardReadOutput>())) { "text=${it.text.take(80)}" }
                "wifi_toggle" -> fmt(codec.decode(bytes, serializer<WifiToggleOutput>())) { "enabled=${it.enabled}" }
                "bluetooth_toggle" -> fmt(codec.decode(bytes, serializer<BluetoothToggleOutput>())) { "enabled=${it.enabled}" }
                "media_play_pause" -> fmt(codec.decode(bytes, serializer<MediaPlayPauseOutput>())) { "handled=${it.handled}" }
                "media_now_playing" -> fmt(codec.decode(bytes, serializer<NowPlayingOutput>())) { "title=${it.title} artist=${it.artist}" }
                "screenshot" -> fmt(codec.decode(bytes, serializer<ScreenshotOutput>())) { "path=${it.path} ${it.width}x${it.height}" }
                "screen_record" -> fmt(codec.decode(bytes, serializer<ScreenRecordOutput>())) { "path=${it.path}" }
                "take_photo", "intent_camera_still" -> fmt(codec.decode(bytes, serializer<TakePhotoOutput>())) { "path=${it.path}" }
                "record_video", "intent_camera_video" -> fmt(codec.decode(bytes, serializer<RecordVideoOutput>())) { "path=${it.path}" }
                "record_audio" -> fmt(codec.decode(bytes, serializer<RecordAudioOutput>())) { "path=${it.path}" }
                "open_webpage" -> fmt(codec.decode(bytes, serializer<OpenWebPageOutput>())) { "url=${it.finalUrl} title=${it.title}" }
                "select_file", "intent_open_document" -> fmt(codec.decode(bytes, serializer<SelectFileOutput>())) { "uri=${it.uri} code=${it.resultCode}" }
                "search_files" -> fmt(codec.decode(bytes, serializer<FileSearchOutput>())) { "files=${it.files.size}" }
                "list_calendar_events" -> fmt(codec.decode(bytes, serializer<ListCalendarEventsOutput>())) { "events=${it.events.size}" }
                "create_calendar_event", "intent_insert_calendar" -> fmt(codec.decode(bytes, serializer<InsertCalendarEventOutput>())) { "eventId=${it.eventId} created=${it.created}" }
                "check_permissions" -> fmt(codec.decode(bytes, serializer<PermissionStatusOutput>())) { "${it.granted}" }
                "read_file" -> fmt(codec.decode(bytes, serializer<ReadFileOutput>())) { it.content.take(120) }
                "http_call" -> fmt(codec.decode(bytes, serializer<HttpResponse>())) { "status=${it.status} body=${it.body.take(80)}" }
                "intent_capture_return", "intent_pick_contact", "intent_pick_contact_data", "intent_get_content" ->
                    fmt(codec.decode(bytes, serializer<IntentActivityResult>())) { "code=${it.resultCode} uri=${it.dataUri} ${it.message}" }
                else -> bytes.decodeToString().take(200)
            }
        } catch (_: Exception) {
            bytes.decodeToString().take(200)
        }
    }

    private fun <T> fmt(value: T, block: (T) -> String): String = block(value)
}
