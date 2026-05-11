package example

import actions.DeviceInfoAction
import actions.DeviceInfoInput
import actions.DeviceInfoOutput
import actions.ForegroundAppAction
import actions.ForegroundAppInput
import actions.ForegroundAppOutput
import actions.HttpCallAction
import actions.HttpRequest
import actions.HttpResponse
import actions.LocationAction
import actions.LocationInput
import actions.LocationOutput
import actions.NetworkStatusAction
import actions.NetworkStatusInput
import actions.NetworkStatusOutput
import actions.PermissionStatusAction
import actions.PermissionStatusInput
import actions.PermissionStatusOutput
import actions.PowerStatusAction
import actions.PowerStatusInput
import actions.PowerStatusOutput
import actions.ReadFileAction
import actions.ReadFileInput
import actions.ReadFileOutput
import actions.StorageInfoAction
import actions.StorageInfoInput
import actions.StorageInfoOutput
import kotlinx.serialization.serializer
import runtime.ActionRegistry

/**
 * Mirrors [runtime.ActionRuntime.registerDefaults] registration list.
 * Keep in sync when built-in actions change.
 */
fun buildSmokeActionRegistry(): ActionRegistry {
    return ActionRegistry().apply {
        register(
            "device_info",
            DeviceInfoAction(),
            serializer<DeviceInfoInput>(),
            serializer<DeviceInfoOutput>(),
        )
        register(
            "network_status",
            NetworkStatusAction(),
            serializer<NetworkStatusInput>(),
            serializer<NetworkStatusOutput>(),
        )
        register(
            "power_status",
            PowerStatusAction(),
            serializer<PowerStatusInput>(),
            serializer<PowerStatusOutput>(),
        )
        register(
            "storage_info",
            StorageInfoAction(),
            serializer<StorageInfoInput>(),
            serializer<StorageInfoOutput>(),
        )
        register(
            "get_location",
            LocationAction(),
            serializer<LocationInput>(),
            serializer<LocationOutput>(),
        )
        register(
            "foreground_app",
            ForegroundAppAction(),
            serializer<ForegroundAppInput>(),
            serializer<ForegroundAppOutput>(),
        )
        register(
            "check_permissions",
            PermissionStatusAction(),
            serializer<PermissionStatusInput>(),
            serializer<PermissionStatusOutput>(),
        )
        register(
            "read_file",
            ReadFileAction(),
            serializer<ReadFileInput>(),
            serializer<ReadFileOutput>(),
        )
        register(
            "http_call",
            HttpCallAction(),
            serializer<HttpRequest>(),
            serializer<HttpResponse>(),
        )
    }
}
