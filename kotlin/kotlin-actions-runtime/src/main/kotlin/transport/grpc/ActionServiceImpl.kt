package transport.grpc

import api.ActionRequest
import api.ActionResponse
import io.grpc.stub.StreamObserver
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import runtime.ActionExecutor
import agent.runtime.proto.ActionServiceGrpc
import agent.runtime.proto.ActionRequest as ActionRequestProto
import agent.runtime.proto.ActionResponse as ActionResponseProto
import agent.runtime.proto.ActionError as ActionErrorProto

class ActionServiceImpl(
    private val executor: ActionExecutor,
) : ActionServiceGrpc.ActionServiceImplBase() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun execute(
        request: ActionRequestProto,
        responseObserver: StreamObserver<ActionResponseProto>,
    ) {
        scope.launch {
            try {
                val response = executor.execute(request.toDomain())
                responseObserver.onNext(response.toProto())
                responseObserver.onCompleted()
            } catch (e: Exception) {
                responseObserver.onError(e)
            }
        }
    }
}

private fun ActionRequestProto.toDomain(): ActionRequest {
    return ActionRequest(
        actionName = actionName,
        payload = payload.toByteArray(),
        metadata = metadataMap,
    )
}

private fun ActionResponse.toProto(): ActionResponseProto {
    val builder = ActionResponseProto.newBuilder()
        .setSuccess(success)
    result?.let { builder.setResult(com.google.protobuf.ByteString.copyFrom(it)) }
    error?.let {
        builder.setError(
            ActionErrorProto.newBuilder()
                .setCode(it.code.name)
                .setMessage(it.message)
                .setRetryable(it.retryable)
                .build(),
        )
    }
    return builder.build()
}
