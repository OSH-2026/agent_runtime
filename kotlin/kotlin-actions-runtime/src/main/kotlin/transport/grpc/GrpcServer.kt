package transport.grpc

import io.grpc.Grpc
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.InsecureServerCredentials

class GrpcServer(
    port: Int,
    service: ActionServiceImpl,
) {
    private val server: Server = Grpc.newServerBuilderForPort(
        port,
        InsecureServerCredentials.create(),
    )
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
