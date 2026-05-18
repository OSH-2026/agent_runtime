package runtime

import actions.IntentActivityResult
import android.content.Context
import android.content.Intent
import error.ActionException
import error.ErrorCode
import util.resolveActivityPackage
import kotlin.coroutines.cancellation.CancellationException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.CancellableContinuation
import kotlinx.coroutines.suspendCancellableCoroutine

object IntentHostCoordinator {
    private val lock = Any()

    @Volatile
    private var pending: CancellableContinuation<IntentActivityResult>? = null

    suspend fun startForResult(context: Context, intent: Intent): IntentActivityResult {
        val pm = context.packageManager
        val resolvedPackage = resolveActivityPackage(pm, intent)
            ?: throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "No app can handle intent: ${intent.action}",
                retryable = false,
            )
        return suspendCancellableCoroutine { cont ->
            synchronized(lock) {
                if (pending != null) {
                    cont.resumeWithException(
                        ActionException(
                            code = ErrorCode.INTERNAL,
                            message = "Another intent result request is already in progress",
                            retryable = true,
                        ),
                    )
                    return@suspendCancellableCoroutine
                }
                pending = cont
            }
            cont.invokeOnCancellation {
                synchronized(lock) {
                    if (pending === cont) {
                        pending = null
                    }
                }
            }
            val hostIntent = Intent(context, IntentHostActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                putExtra(IntentHostActivity.EXTRA_FORWARD_INTENT, intent)
                putExtra(IntentHostActivity.EXTRA_RESOLVED_PACKAGE, resolvedPackage)
            }
            context.startActivity(hostIntent)
        }
    }

    internal fun complete(result: IntentActivityResult) {
        val cont = synchronized(lock) {
            val current = pending
            pending = null
            current
        } ?: return
        cont.resume(result)
    }

    internal fun fail(message: String) {
        val cont = synchronized(lock) {
            val current = pending
            pending = null
            current
        } ?: return
        cont.resumeWithException(
            ActionException(code = ErrorCode.INTERNAL, message = message, retryable = false),
        )
    }
}
