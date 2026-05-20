package actions

import android.os.Handler
import android.os.Looper
import android.webkit.WebView
import android.webkit.WebViewClient
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.serialization.Serializable

@Serializable
data class OpenWebPageInput(
    val url: String,
    val timeoutMs: Long = 10_000,
)

@Serializable
data class OpenWebPageOutput(
    val finalUrl: String,
    val title: String,
)

class OpenWebPageAction : Action<OpenWebPageInput, OpenWebPageOutput> {
    override suspend fun execute(input: OpenWebPageInput, ctx: ActionContext): OpenWebPageOutput {
        val handler = Handler(Looper.getMainLooper())
        return suspendCancellableCoroutine { cont ->
            handler.post {
                val webView = WebView(ctx.appContext)
                webView.settings.javaScriptEnabled = true
                webView.webViewClient = object : WebViewClient() {
                    override fun onPageFinished(view: WebView, url: String) {
                        if (cont.isActive) {
                            cont.resume(
                                OpenWebPageOutput(
                                    finalUrl = url,
                                    title = view.title ?: "",
                                ),
                            ) {}
                        }
                        view.destroy()
                    }
                }
                webView.loadUrl(input.url)
                val timeout = Runnable {
                    if (cont.isActive) {
                        cont.resumeWithException(
                            ActionException(
                                code = ErrorCode.TIMEOUT,
                                message = "WebView load timeout",
                                retryable = true,
                            ),
                        )
                    }
                    webView.destroy()
                }
                handler.postDelayed(timeout, input.timeoutMs)
                cont.invokeOnCancellation {
                    handler.removeCallbacks(timeout)
                    webView.destroy()
                }
            }
        }
    }
}
