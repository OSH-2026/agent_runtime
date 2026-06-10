package runtime

import android.accessibilityservice.AccessibilityService
import android.graphics.Bitmap
import android.os.Build
import android.view.Display
import android.view.accessibility.AccessibilityEvent
import error.ActionException
import error.ErrorCode
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.suspendCancellableCoroutine

class ActionScreenshotAccessibilityService : AccessibilityService() {
    override fun onServiceConnected() {
        instance = this
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) = Unit

    override fun onInterrupt() = Unit

    override fun onDestroy() {
        if (instance === this) {
            instance = null
        }
        super.onDestroy()
    }

    private suspend fun captureDisplay(): Bitmap {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) {
            throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "Accessibility screenshots require Android 11 or newer",
                retryable = false,
            )
        }
        return suspendCancellableCoroutine { cont ->
            takeScreenshot(
                Display.DEFAULT_DISPLAY,
                mainExecutor,
                object : TakeScreenshotCallback {
                    override fun onSuccess(result: ScreenshotResult) {
                        val buffer = result.hardwareBuffer
                        try {
                            val hardwareBitmap = Bitmap.wrapHardwareBuffer(buffer, result.colorSpace)
                            try {
                                val bitmap = hardwareBitmap?.copy(Bitmap.Config.ARGB_8888, false)
                                if (bitmap == null) {
                                    if (cont.isActive) {
                                        cont.resumeWithException(
                                            screenshotError("Unable to decode accessibility screenshot"),
                                        )
                                    }
                                } else if (cont.isActive) {
                                    cont.resume(bitmap)
                                } else {
                                    bitmap.recycle()
                                }
                            } finally {
                                hardwareBitmap?.recycle()
                            }
                        } finally {
                            buffer.close()
                        }
                    }

                    override fun onFailure(errorCode: Int) {
                        if (cont.isActive) {
                            cont.resumeWithException(
                                screenshotError(
                                    "Accessibility screenshot failed: ${failureName(errorCode)}",
                                ),
                            )
                        }
                    }
                },
            )
        }
    }

    companion object {
        @Volatile
        private var instance: ActionScreenshotAccessibilityService? = null

        suspend fun capture(): Bitmap {
            val service = instance
                ?: throw ActionException(
                    code = ErrorCode.PERMISSION,
                    message = "Enable Action Runtime screenshot service in Accessibility settings",
                    retryable = false,
                )
            return service.captureDisplay()
        }

        private fun screenshotError(message: String): ActionException {
            return ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = message,
                retryable = false,
            )
        }

        private fun failureName(errorCode: Int): String {
            return when (errorCode) {
                ERROR_TAKE_SCREENSHOT_INTERNAL_ERROR -> "internal error"
                ERROR_TAKE_SCREENSHOT_INTERVAL_TIME_SHORT -> "requests are too frequent"
                ERROR_TAKE_SCREENSHOT_INVALID_DISPLAY -> "invalid display"
                ERROR_TAKE_SCREENSHOT_NO_ACCESSIBILITY_ACCESS -> "accessibility access unavailable"
                ERROR_TAKE_SCREENSHOT_SECURE_WINDOW -> "current window blocks screenshots"
                else -> "error $errorCode"
            }
        }
    }
}
