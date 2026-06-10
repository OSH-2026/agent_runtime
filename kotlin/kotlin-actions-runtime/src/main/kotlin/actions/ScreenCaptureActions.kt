package actions

import android.app.Activity
import android.content.Context
import android.graphics.Bitmap
import android.graphics.PixelFormat
import android.hardware.display.VirtualDisplay
import android.media.ImageReader
import android.media.MediaRecorder
import android.media.projection.MediaProjection
import android.media.projection.MediaProjectionManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.Serializable
import runtime.ActionRuntimeService
import runtime.ActionScreenshotAccessibilityService
import runtime.MediaProjectionCoordinator
import util.PermissionUtils

@Serializable
data class ScreenshotInput(
    val timeoutMs: Long = 3_000,
)

@Serializable
data class ScreenshotOutput(
    val path: String,
    val width: Int,
    val height: Int,
)

class ScreenshotAction : Action<ScreenshotInput, ScreenshotOutput> {
    override suspend fun execute(input: ScreenshotInput, ctx: ActionContext): ScreenshotOutput {
        val context = ctx.appContext
        return withContext(Dispatchers.IO) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                return@withContext withTimeout(input.timeoutMs) {
                    val bitmap = ActionScreenshotAccessibilityService.capture()
                    val file = File(context.cacheDir, "screenshot_${System.currentTimeMillis()}.png")
                    try {
                        FileOutputStream(file).use { out ->
                            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
                        }
                        ScreenshotOutput(
                            path = file.absolutePath,
                            width = bitmap.width,
                            height = bitmap.height,
                        )
                    } finally {
                        bitmap.recycle()
                    }
                }
            }
            val projection = requestProjection(context)
            val metrics = context.resources.displayMetrics
            val reader = ImageReader.newInstance(metrics.widthPixels, metrics.heightPixels, PixelFormat.RGBA_8888, 2)
            val resources = ScreenshotCaptureResources(reader)
            var callbackRegistered = false
            try {
                projection.registerCallback(resources, Handler(Looper.getMainLooper()))
                callbackRegistered = true
                resources.attachDisplay(
                    projection.createVirtualDisplay(
                        "action_runtime_screenshot",
                        metrics.widthPixels,
                        metrics.heightPixels,
                        metrics.densityDpi,
                        0,
                        reader.surface,
                        null,
                        null,
                    ),
                )
                val file = File(context.cacheDir, "screenshot_${System.currentTimeMillis()}.png")
                withTimeout(input.timeoutMs) { reader.awaitImage() }.use { image ->
                    val bitmap = imageToBitmap(image, metrics)
                    try {
                        FileOutputStream(file).use { out ->
                            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
                        }
                    } finally {
                        bitmap.recycle()
                    }
                }
                ScreenshotOutput(path = file.absolutePath, width = metrics.widthPixels, height = metrics.heightPixels)
            } finally {
                resources.release()
                if (callbackRegistered) {
                    projection.unregisterCallback(resources)
                }
                projection.stop()
            }
        }
    }
}

@Serializable
data class ScreenRecordInput(
    val durationSeconds: Int = 10,
    val withAudio: Boolean = false,
)

@Serializable
data class ScreenRecordOutput(
    val path: String,
    val durationSeconds: Int,
)

class ScreenRecordAction : Action<ScreenRecordInput, ScreenRecordOutput> {
    override suspend fun execute(input: ScreenRecordInput, ctx: ActionContext): ScreenRecordOutput {
        val context = ctx.appContext
        return withContext(Dispatchers.IO) {
            if (input.withAudio && !PermissionUtils.hasPermission(context, android.Manifest.permission.RECORD_AUDIO)) {
                throw ActionException(
                    code = ErrorCode.PERMISSION,
                    message = "RECORD_AUDIO permission required",
                    retryable = false,
                )
            }
            val projection = requestProjection(context)
            val metrics = context.resources.displayMetrics
            val output = File(context.cacheDir, "screen_record_${System.currentTimeMillis()}.mp4")
            val recorder = MediaRecorder()
            if (input.withAudio) {
                recorder.setAudioSource(MediaRecorder.AudioSource.MIC)
            }
            recorder.setVideoSource(MediaRecorder.VideoSource.SURFACE)
            recorder.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            recorder.setOutputFile(output.absolutePath)
            recorder.setVideoEncodingBitRate(5_000_000)
            recorder.setVideoFrameRate(30)
            recorder.setVideoSize(metrics.widthPixels, metrics.heightPixels)
            recorder.setVideoEncoder(MediaRecorder.VideoEncoder.H264)
            if (input.withAudio) {
                recorder.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
                recorder.setAudioEncodingBitRate(128_000)
                recorder.setAudioSamplingRate(44_100)
            }
            recorder.prepare()
            val resources = ScreenRecordResources(recorder)
            var callbackRegistered = false
            try {
                projection.registerCallback(resources, Handler(Looper.getMainLooper()))
                callbackRegistered = true
                resources.attachDisplay(
                    projection.createVirtualDisplay(
                        "action_runtime_record",
                        metrics.widthPixels,
                        metrics.heightPixels,
                        metrics.densityDpi,
                        0,
                        recorder.surface,
                        null,
                        null,
                    ),
                )
                resources.startRecorder()
                delay(input.durationSeconds * 1000L)
                ScreenRecordOutput(path = output.absolutePath, durationSeconds = input.durationSeconds)
            } finally {
                resources.release()
                if (callbackRegistered) {
                    projection.unregisterCallback(resources)
                }
                projection.stop()
            }
        }
    }
}

private class ScreenshotCaptureResources(
    private val reader: ImageReader,
) : MediaProjection.Callback() {
    private val released = AtomicBoolean(false)

    @Volatile
    private var display: VirtualDisplay? = null

    fun attachDisplay(value: VirtualDisplay) {
        if (released.get()) {
            value.release()
            return
        }
        display = value
        if (released.get()) {
            display?.release()
            display = null
        }
    }

    override fun onStop() {
        release()
    }

    fun release() {
        if (!released.compareAndSet(false, true)) {
            return
        }
        reader.setOnImageAvailableListener(null, null)
        display?.release()
        display = null
        reader.close()
    }
}

private class ScreenRecordResources(
    private val recorder: MediaRecorder,
) : MediaProjection.Callback() {
    private var display: VirtualDisplay? = null
    private var recorderStarted = false
    private var released = false

    @Synchronized
    fun attachDisplay(value: VirtualDisplay) {
        if (released) {
            value.release()
        } else {
            display = value
        }
    }

    @Synchronized
    fun startRecorder() {
        check(!released) { "Media projection stopped before recording started" }
        recorder.start()
        recorderStarted = true
    }

    override fun onStop() {
        release()
    }

    @Synchronized
    fun release() {
        if (released) {
            return
        }
        released = true
        if (recorderStarted) {
            runCatching { recorder.stop() }
            recorderStarted = false
        }
        runCatching { recorder.reset() }
        recorder.release()
        display?.release()
        display = null
    }
}

private suspend fun requestProjection(context: Context): android.media.projection.MediaProjection {
    val manager = context.getSystemService(MediaProjectionManager::class.java)
        ?: throw ActionException(
            code = ErrorCode.INTERNAL,
            message = "MediaProjectionManager unavailable",
            retryable = true,
        )
    val grant = MediaProjectionCoordinator.request(context)
    if (grant.resultCode != Activity.RESULT_OK || grant.data == null) {
        throw ActionException(
            code = ErrorCode.PERMISSION,
            message = "Media projection permission denied",
            retryable = false,
        )
    }
    delay(200)
    ActionRuntimeService.enableMediaProjection(context)
    return manager.getMediaProjection(grant.resultCode, grant.data)
        ?: throw ActionException(
            code = ErrorCode.INTERNAL,
            message = "Failed to obtain MediaProjection",
            retryable = true,
        )
}

private suspend fun ImageReader.awaitImage(): android.media.Image {
    return kotlinx.coroutines.suspendCancellableCoroutine { cont ->
        val handler = Handler(Looper.getMainLooper())
        val listener = ImageReader.OnImageAvailableListener {
            val image = it.acquireLatestImage()
            if (image != null && cont.isActive) {
                setOnImageAvailableListener(null, null)
                cont.resume(image) {}
            }
        }
        setOnImageAvailableListener(listener, handler)
        cont.invokeOnCancellation { setOnImageAvailableListener(null, null) }
    }
}

private fun imageToBitmap(image: android.media.Image, metrics: DisplayMetrics): Bitmap {
    val plane = image.planes[0]
    val buffer = plane.buffer
    val pixelStride = plane.pixelStride
    val rowStride = plane.rowStride
    val rowPadding = rowStride - pixelStride * metrics.widthPixels
    val bitmap = Bitmap.createBitmap(
        metrics.widthPixels + rowPadding / pixelStride,
        metrics.heightPixels,
        Bitmap.Config.ARGB_8888,
    )
    bitmap.copyPixelsFromBuffer(buffer)
    return Bitmap.createBitmap(bitmap, 0, 0, metrics.widthPixels, metrics.heightPixels)
}
