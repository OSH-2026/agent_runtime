package runtime

import api.ActionContext
import error.ActionException

class AuditMiddleware(
    private val auditLog: ActionAuditLog,
    private val actionNameProvider: (ActionContext) -> String,
) : Middleware {
    override suspend fun <T> intercept(ctx: ActionContext, next: suspend () -> T): T {
        val actionName = actionNameProvider(ctx)
        return try {
            val result = next()
            auditLog.record(
                requestId = ctx.requestId,
                nodeId = ctx.nodeId,
                actionName = actionName,
                success = true,
            )
            result
        } catch (e: ActionException) {
            auditLog.record(
                requestId = ctx.requestId,
                nodeId = ctx.nodeId,
                actionName = actionName,
                success = false,
                errorCode = e.code,
            )
            throw e
        } catch (e: Exception) {
            auditLog.record(
                requestId = ctx.requestId,
                nodeId = ctx.nodeId,
                actionName = actionName,
                success = false,
                errorCode = null,
            )
            throw e
        }
    }
}
