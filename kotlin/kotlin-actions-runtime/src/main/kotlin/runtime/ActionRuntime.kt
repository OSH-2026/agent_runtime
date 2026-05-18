package runtime

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
