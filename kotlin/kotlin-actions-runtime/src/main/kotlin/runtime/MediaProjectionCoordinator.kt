package runtime

import android.content.Context
import android.content.Intent
import error.ActionException
import error.ErrorCode
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.CancellableContinuation
import kotlinx.coroutines.suspendCancellableCoroutine

object MediaProjectionCoordinator {
    private val lock = Any()

    @Volatile
    private var pending: CancellableContinuation<MediaProjectionGrant>? = null

    suspend fun request(context: Context): MediaProjectionGrant {
        return suspendCancellableCoroutine { cont ->
            synchronized(lock) {
                if (pending != null) {
                    cont.resumeWithException(
                        ActionException(
                            code = ErrorCode.INTERNAL,
                            message = "Another media projection request is already in progress",
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
            val intent = Intent(context, MediaProjectionPermissionActivity::class.java).apply {
                addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK or
                        Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS or
                        Intent.FLAG_ACTIVITY_NO_ANIMATION,
                )
            }
            context.startActivity(intent)
        }
    }

    internal fun complete(resultCode: Int, data: Intent?) {
        val cont = synchronized(lock) {
            val current = pending
            pending = null
            current
        } ?: return
        cont.resume(MediaProjectionGrant(resultCode, data))
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

data class MediaProjectionGrant(
    val resultCode: Int,
    val data: Intent?,
)
