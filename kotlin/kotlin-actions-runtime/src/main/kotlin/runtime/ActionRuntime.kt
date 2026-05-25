package runtime

import actions.registerBuiltinActions
import actions.registerIntentActions
import android.content.Context
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
        registry.registerBuiltinActions()
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
