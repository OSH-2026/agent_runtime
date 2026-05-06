package transport.grpc

import io.grpc.Server
import io.grpc.ServerBuilder

class GrpcServer(
    port: Int,
    service: ActionServiceImpl,
) {
    private val server: Server = ServerBuilder.forPort(port)
        .addService(service)
        .build()

    fun start() {
        server.start()
    }

    fun blockUntilShutdown() {
        server.awaitTermination()
    }

    fun shutdown() {
        server.shutdown()
    }
}
