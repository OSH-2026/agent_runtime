package runtime

import actions.BluetoothToggleAction
import actions.BluetoothToggleInput
import actions.BluetoothToggleOutput
import actions.ClipboardCopyAction
import actions.ClipboardCopyInput
import actions.ClipboardCopyOutput
import actions.ClipboardReadAction
import actions.ClipboardReadInput
import actions.ClipboardReadOutput
import actions.DeviceInfoAction
import actions.DeviceInfoInput
import actions.DeviceInfoOutput
import actions.FileSearchAction
import actions.FileSearchInput
import actions.FileSearchOutput
import actions.ForegroundAppAction
import actions.ForegroundAppInput
import actions.ForegroundAppOutput
import actions.LaunchResult
import actions.LaunchAppAction
import actions.LaunchAppInput
import actions.ListCalendarEventsAction
import actions.ListCalendarEventsInput
import actions.ListCalendarEventsOutput
import actions.ListInstalledAppsAction
import actions.ListInstalledAppsInput
import actions.ListInstalledAppsOutput
import actions.ListNotificationsAction
import actions.ListNotificationsInput
import actions.ListNotificationsOutput
import actions.MediaPlayPauseAction
import actions.MediaPlayPauseInput
import actions.MediaPlayPauseOutput
import actions.NetworkStatusAction
import actions.NetworkStatusInput
import actions.NetworkStatusOutput
import actions.NowPlayingAction
import actions.NowPlayingInput
import actions.NowPlayingOutput
import actions.OpenWebPageAction
import actions.OpenWebPageInput
import actions.OpenWebPageOutput
import actions.PermissionStatusAction
import actions.PermissionStatusInput
import actions.PermissionStatusOutput
import actions.PlaceCallAction
import actions.PlaceCallInput
import actions.PlaceCallOutput
import actions.PowerStatusAction
import actions.PowerStatusInput
import actions.PowerStatusOutput
import actions.ReadCallLogAction
import actions.ReadCallLogInput
import actions.ReadCallLogOutput
import actions.ReadSmsAction
import actions.ReadSmsInput
import actions.ReadSmsOutput
import actions.RecordAudioAction
import actions.RecordAudioInput
import actions.RecordAudioOutput
import actions.RecordVideoAction
import actions.RecordVideoInput
import actions.RecordVideoOutput
import actions.ScreenRecordAction
import actions.ScreenRecordInput
import actions.ScreenRecordOutput
import actions.ScreenshotAction
import actions.ScreenshotInput
import actions.ScreenshotOutput
import actions.SearchContactsAction
import actions.SearchContactsInput
import actions.SearchContactsOutput
import actions.SelectFileAction
import actions.SelectFileInput
import actions.SelectFileOutput
import actions.SendSmsAction
import actions.SendSmsInput
import actions.SendSmsOutput
import actions.SetAlarmAction
import actions.SetAlarmInput
import actions.SetAlarmOutput
import actions.SetSilentModeAction
import actions.SetSilentModeInput
import actions.SetSilentModeOutput
import actions.SetTimerAction
import actions.SetTimerInput
import actions.SetTimerOutput
import actions.SetVolumeAction
import actions.SetVolumeInput
import actions.SetVolumeOutput
import actions.ShowAlarmsAction
import actions.ShowAlarmsInput
import actions.AlarmListOutput
import actions.StorageInfoAction
import actions.StorageInfoInput
import actions.StorageInfoOutput
import actions.SystemInfoAction
import actions.SystemInfoInput
import actions.SystemInfoOutput
import actions.TakePhotoAction
import actions.TakePhotoInput
import actions.TakePhotoOutput
import actions.WifiToggleAction
import actions.WifiToggleInput
import actions.WifiToggleOutput
import actions.InsertCalendarEventAction
import actions.InsertCalendarEventInput
import actions.InsertCalendarEventOutput
import actions.LocationAction
import actions.LocationInput
import actions.LocationOutput
import actions.HttpCallAction
import actions.HttpRequest
import actions.HttpResponse
import actions.ReadFileAction
import actions.ReadFileInput
import actions.ReadFileOutput
import actions.registerIntentActions
import android.content.Context
import kotlinx.serialization.serializer
import transport.grpc.ActionServiceImpl
import transport.grpc.GrpcServer
import transport.serialization.Codec
import transport.serialization.JsonCodec

class ActionRuntime(
    private val appContext: Context,
    private val port: Int = 8080,
    private val codec: Codec = JsonCodec(),
    private val registry: ActionRegistry = ActionRegistry(),
) {
    private var server: GrpcServer? = null

    fun registerDefaults(): ActionRuntime {
        registry.register(
            "device_info",
            DeviceInfoAction(),
            serializer<DeviceInfoInput>(),
            serializer<DeviceInfoOutput>(),
        )
        registry.register(
            "system_info",
            SystemInfoAction(),
            serializer<SystemInfoInput>(),
            serializer<SystemInfoOutput>(),
        )
        registry.register(
            "network_status",
            NetworkStatusAction(),
            serializer<NetworkStatusInput>(),
            serializer<NetworkStatusOutput>(),
        )
        registry.register(
            "power_status",
            PowerStatusAction(),
            serializer<PowerStatusInput>(),
            serializer<PowerStatusOutput>(),
        )
        registry.register(
            "set_volume",
            SetVolumeAction(),
            serializer<SetVolumeInput>(),
            serializer<SetVolumeOutput>(),
        )
        registry.register(
            "set_silent_mode",
            SetSilentModeAction(),
            serializer<SetSilentModeInput>(),
            serializer<SetSilentModeOutput>(),
        )
        registry.register(
            "storage_info",
            StorageInfoAction(),
            serializer<StorageInfoInput>(),
            serializer<StorageInfoOutput>(),
        )
        registry.register(
            "get_location",
            LocationAction(),
            serializer<LocationInput>(),
            serializer<LocationOutput>(),
        )
        registry.register(
            "foreground_app",
            ForegroundAppAction(),
            serializer<ForegroundAppInput>(),
            serializer<ForegroundAppOutput>(),
        )
        registry.register(
            "list_installed_apps",
            ListInstalledAppsAction(),
            serializer<ListInstalledAppsInput>(),
            serializer<ListInstalledAppsOutput>(),
        )
        registry.register(
            "launch_app",
            LaunchAppAction(),
            serializer<LaunchAppInput>(),
            serializer<LaunchResult>(),
        )
        registry.register(
            "set_alarm",
            SetAlarmAction(),
            serializer<SetAlarmInput>(),
            serializer<SetAlarmOutput>(),
        )
        registry.register(
            "set_timer",
            SetTimerAction(),
            serializer<SetTimerInput>(),
            serializer<SetTimerOutput>(),
        )
        registry.register(
            "list_alarms",
            ShowAlarmsAction(),
            serializer<ShowAlarmsInput>(),
            serializer<AlarmListOutput>(),
        )
        registry.register(
            "read_sms",
            ReadSmsAction(),
            serializer<ReadSmsInput>(),
            serializer<ReadSmsOutput>(),
        )
        registry.register(
            "send_sms",
            SendSmsAction(),
            serializer<SendSmsInput>(),
            serializer<SendSmsOutput>(),
        )
        registry.register(
            "read_call_log",
            ReadCallLogAction(),
            serializer<ReadCallLogInput>(),
            serializer<ReadCallLogOutput>(),
        )
        registry.register(
            "place_call",
            PlaceCallAction(),
            serializer<PlaceCallInput>(),
            serializer<PlaceCallOutput>(),
        )
        registry.register(
            "search_contacts",
            SearchContactsAction(),
            serializer<SearchContactsInput>(),
            serializer<SearchContactsOutput>(),
        )
        registry.register(
            "list_notifications",
            ListNotificationsAction(),
            serializer<ListNotificationsInput>(),
            serializer<ListNotificationsOutput>(),
        )
        registry.register(
            "clipboard_copy",
            ClipboardCopyAction(),
            serializer<ClipboardCopyInput>(),
            serializer<ClipboardCopyOutput>(),
        )
        registry.register(
            "clipboard_read",
            ClipboardReadAction(),
            serializer<ClipboardReadInput>(),
            serializer<ClipboardReadOutput>(),
        )
        registry.register(
            "wifi_toggle",
            WifiToggleAction(),
            serializer<WifiToggleInput>(),
            serializer<WifiToggleOutput>(),
        )
        registry.register(
            "bluetooth_toggle",
            BluetoothToggleAction(),
            serializer<BluetoothToggleInput>(),
            serializer<BluetoothToggleOutput>(),
        )
        registry.register(
            "media_play_pause",
            MediaPlayPauseAction(),
            serializer<MediaPlayPauseInput>(),
            serializer<MediaPlayPauseOutput>(),
        )
        registry.register(
            "media_now_playing",
            NowPlayingAction(),
            serializer<NowPlayingInput>(),
            serializer<NowPlayingOutput>(),
        )
        registry.register(
            "screenshot",
            ScreenshotAction(),
            serializer<ScreenshotInput>(),
            serializer<ScreenshotOutput>(),
        )
        registry.register(
            "screen_record",
            ScreenRecordAction(),
            serializer<ScreenRecordInput>(),
            serializer<ScreenRecordOutput>(),
        )
        registry.register(
            "take_photo",
            TakePhotoAction(),
            serializer<TakePhotoInput>(),
            serializer<TakePhotoOutput>(),
        )
        registry.register(
            "record_video",
            RecordVideoAction(),
            serializer<RecordVideoInput>(),
            serializer<RecordVideoOutput>(),
        )
        registry.register(
            "record_audio",
            RecordAudioAction(),
            serializer<RecordAudioInput>(),
            serializer<RecordAudioOutput>(),
        )
        registry.register(
            "open_webpage",
            OpenWebPageAction(),
            serializer<OpenWebPageInput>(),
            serializer<OpenWebPageOutput>(),
        )
        registry.register(
            "select_file",
            SelectFileAction(),
            serializer<SelectFileInput>(),
            serializer<SelectFileOutput>(),
        )
        registry.register(
            "search_files",
            FileSearchAction(),
            serializer<FileSearchInput>(),
            serializer<FileSearchOutput>(),
        )
        registry.register(
            "list_calendar_events",
            ListCalendarEventsAction(),
            serializer<ListCalendarEventsInput>(),
            serializer<ListCalendarEventsOutput>(),
        )
        registry.register(
            "create_calendar_event",
            InsertCalendarEventAction(),
            serializer<InsertCalendarEventInput>(),
            serializer<InsertCalendarEventOutput>(),
        )
        registry.register(
            "check_permissions",
            PermissionStatusAction(),
            serializer<PermissionStatusInput>(),
            serializer<PermissionStatusOutput>(),
        )

        registry.register(
            "read_file",
            ReadFileAction(),
            serializer<ReadFileInput>(),
            serializer<ReadFileOutput>(),
        )
        registry.register(
            "http_call",
            HttpCallAction(),
            serializer<HttpRequest>(),
            serializer<HttpResponse>(),
        )
        registry.registerIntentActions()
        return this
    }

    fun auditSnapshot(limit: Int = 50) = ActionAuditLogHolder.log.snapshot(limit)

    fun start() {
        val executor = ActionExecutor(appContext, registry, codec)
        val service = ActionServiceImpl(executor)
        val grpcServer = GrpcServer(port, service)
        grpcServer.start()
        server = grpcServer
    }

    fun stop() {
        server?.shutdown()
    }
}
