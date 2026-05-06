package runtime

import api.ActionContext

interface Middleware {
    suspend fun <T> intercept(ctx: ActionContext, next: suspend () -> T): T
}
