package runtime

import api.ActionContext
import api.ActionRequest
import api.ActionResponse
import error.ActionError
import kotlinx.coroutines.withTimeout
import transport.serialization.Codec

class ActionExecutor(
    private val appContext: android.content.Context,
    private val registry: ActionRegistry,
    private val codec: Codec,
    private val middleware: List<Middleware> = emptyList(),
) {
    suspend fun execute(req: ActionRequest): ActionResponse {
        return try {
            val spec = registry.get(req.actionName)
            @Suppress("UNCHECKED_CAST")
            val typedSpec = spec as ActionSpec<Any, Any>
            val input = codec.decode(req.payload, typedSpec.inputSerializer)
            val ctx = ActionContext(
                appContext = appContext,
                requestId = req.metadata["requestId"] ?: "",
                nodeId = req.metadata["nodeId"] ?: "",
                deadline = System.currentTimeMillis() + 30_000,
                metadata = req.metadata,
            )
            val timeoutMs = (ctx.deadline - System.currentTimeMillis()).coerceAtLeast(0)
            val result = runWithMiddleware(ctx) {
                withTimeout(timeoutMs) {
                    typedSpec.action.execute(input, ctx)
                }
            }
            ActionResponse(
                success = true,
                result = codec.encode(result, typedSpec.outputSerializer),
            )
        } catch (e: Exception) {
            ActionResponse(
                success = false,
                error = ActionError.from(e),
            )
        }
    }

    private suspend fun <T> runWithMiddleware(ctx: ActionContext, block: suspend () -> T): T {
        var next = block
        middleware.reversed().forEach { layer ->
            val current = next
            next = { layer.intercept(ctx, current) }
        }
        return next()
    }
}
