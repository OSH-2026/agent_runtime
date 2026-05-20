package actions

import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.media.ImageReader
import android.media.MediaRecorder
import android.os.Handler
import android.os.HandlerThread
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import java.io.File
import java.io.FileOutputStream
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class TakePhotoInput(
    val lens: String = "back",
)

@Serializable
data class TakePhotoOutput(
    val path: String,
    val width: Int,
    val height: Int,
)

class TakePhotoAction : Action<TakePhotoInput, TakePhotoOutput> {
    override suspend fun execute(input: TakePhotoInput, ctx: ActionContext): TakePhotoOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.CAMERA)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "CAMERA permission required",
                retryable = false,
            )
        }
        return withContext(Dispatchers.IO) {
            val cameraManager = context.getSystemService(CameraManager::class.java)
                ?: throw ActionException(ErrorCode.INTERNAL, "CameraManager unavailable", true)
            val cameraId = selectCamera(cameraManager, input.lens)
                ?: throw ActionException(ErrorCode.UNAVAILABLE, "No camera available", false)
            val characteristics = cameraManager.getCameraCharacteristics(cameraId)
            val streamConfig = characteristics.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)
                ?: throw ActionException(ErrorCode.UNAVAILABLE, "No camera output configuration", false)
            val size = streamConfig.getOutputSizes(ImageFormat.JPEG).maxByOrNull { it.width * it.height }
                ?: throw ActionException(ErrorCode.UNAVAILABLE, "No JPEG output size", false)
            val handlerThread = HandlerThread("camera_photo").apply { start() }
            val handler = Handler(handlerThread.looper)
            val reader = ImageReader.newInstance(size.width, size.height, ImageFormat.JPEG, 2)
            val camera = openCamera(cameraManager, cameraId, handler)
            val session = createSession(camera, listOf(reader.surface), handler)
            val request = camera.createCaptureRequest(CameraDevice.TEMPLATE_STILL_CAPTURE).apply {
                addTarget(reader.surface)
                set(CaptureRequest.CONTROL_AF_MODE, CaptureRequest.CONTROL_AF_MODE_CONTINUOUS_PICTURE)
            }
            session.capture(request.build(), null, handler)
            val image = reader.awaitImage(handler)
            val file = File(context.getExternalFilesDir(android.os.Environment.DIRECTORY_PICTURES) ?: context.cacheDir, "photo_${System.currentTimeMillis()}.jpg")
            FileOutputStream(file).use { out ->
                val buffer = image.planes[0].buffer
                val bytes = ByteArray(buffer.remaining())
                buffer.get(bytes)
                out.write(bytes)
            }
            image.close()
            reader.close()
            session.close()
            camera.close()
            handlerThread.quitSafely()
            TakePhotoOutput(path = file.absolutePath, width = size.width, height = size.height)
        }
    }
}

@Serializable
data class RecordVideoInput(
    val durationSeconds: Int = 10,
    val lens: String = "back",
    val withAudio: Boolean = false,
)

@Serializable
data class RecordVideoOutput(
    val path: String,
    val durationSeconds: Int,
)

class RecordVideoAction : Action<RecordVideoInput, RecordVideoOutput> {
    override suspend fun execute(input: RecordVideoInput, ctx: ActionContext): RecordVideoOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.CAMERA)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "CAMERA permission required",
                retryable = false,
            )
        }
        if (input.withAudio && !PermissionUtils.hasPermission(context, android.Manifest.permission.RECORD_AUDIO)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "RECORD_AUDIO permission required",
                retryable = false,
            )
        }
        return withContext(Dispatchers.IO) {
            val cameraManager = context.getSystemService(CameraManager::class.java)
                ?: throw ActionException(ErrorCode.INTERNAL, "CameraManager unavailable", true)
            val cameraId = selectCamera(cameraManager, input.lens)
                ?: throw ActionException(ErrorCode.UNAVAILABLE, "No camera available", false)
            val characteristics = cameraManager.getCameraCharacteristics(cameraId)
            val streamConfig = characteristics.get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)
            val videoSize = streamConfig?.getOutputSizes(MediaRecorder::class.java)?.maxByOrNull { it.width * it.height }
            val handlerThread = HandlerThread("camera_video").apply { start() }
            val handler = Handler(handlerThread.looper)
            val camera = openCamera(cameraManager, cameraId, handler)
            val recorder = MediaRecorder()
            if (input.withAudio) {
                recorder.setAudioSource(MediaRecorder.AudioSource.MIC)
            }
            recorder.setVideoSource(MediaRecorder.VideoSource.SURFACE)
            recorder.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            val file = File(context.getExternalFilesDir(android.os.Environment.DIRECTORY_MOVIES) ?: context.cacheDir, "video_${System.currentTimeMillis()}.mp4")
            recorder.setOutputFile(file.absolutePath)
            recorder.setVideoEncodingBitRate(5_000_000)
            recorder.setVideoFrameRate(30)
            val width = videoSize?.width ?: 1280
            val height = videoSize?.height ?: 720
            recorder.setVideoSize(width, height)
            recorder.setVideoEncoder(MediaRecorder.VideoEncoder.H264)
            if (input.withAudio) {
                recorder.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
                recorder.setAudioEncodingBitRate(128_000)
                recorder.setAudioSamplingRate(44_100)
            }
            recorder.prepare()
            val session = createSession(camera, listOf(recorder.surface), handler)
            val request = camera.createCaptureRequest(CameraDevice.TEMPLATE_RECORD).apply {
                addTarget(recorder.surface)
            }
            session.setRepeatingRequest(request.build(), null, handler)
            recorder.start()
            delay(input.durationSeconds * 1000L)
            recorder.stop()
            recorder.reset()
            recorder.release()
            session.close()
            camera.close()
            handlerThread.quitSafely()
            RecordVideoOutput(path = file.absolutePath, durationSeconds = input.durationSeconds)
        }
    }
}

private suspend fun openCamera(
    manager: CameraManager,
    cameraId: String,
    handler: Handler,
): CameraDevice {
    return suspendCancellableCoroutine { cont ->
        manager.openCamera(cameraId, object : CameraDevice.StateCallback() {
            override fun onOpened(camera: CameraDevice) {
                cont.resume(camera) {}
            }

            override fun onDisconnected(camera: CameraDevice) {
                camera.close()
                if (cont.isActive) {
                    cont.resumeWithException(ActionException(ErrorCode.UNAVAILABLE, "Camera disconnected", true))
                }
            }

            override fun onError(camera: CameraDevice, error: Int) {
                camera.close()
                if (cont.isActive) {
                    cont.resumeWithException(ActionException(ErrorCode.INTERNAL, "Camera error $error", true))
                }
            }
        }, handler)
        cont.invokeOnCancellation { }
    }
}

private suspend fun createSession(
    camera: CameraDevice,
    surfaces: List<android.view.Surface>,
    handler: Handler,
): CameraCaptureSession {
    return suspendCancellableCoroutine { cont ->
        camera.createCaptureSession(surfaces, object : CameraCaptureSession.StateCallback() {
            override fun onConfigured(session: CameraCaptureSession) {
                cont.resume(session) {}
            }

            override fun onConfigureFailed(session: CameraCaptureSession) {
                cont.resumeWithException(ActionException(ErrorCode.INTERNAL, "Capture session failed", true))
            }
        }, handler)
    }
}

private suspend fun ImageReader.awaitImage(handler: Handler): android.media.Image {
    return suspendCancellableCoroutine { cont ->
        setOnImageAvailableListener({ reader ->
            val image = reader.acquireLatestImage()
            if (image != null && cont.isActive) {
                cont.resume(image) {}
            }
        }, handler)
        cont.invokeOnCancellation { setOnImageAvailableListener(null, null) }
    }
}

private fun selectCamera(manager: CameraManager, lens: String): String? {
    for (id in manager.cameraIdList) {
        val characteristics = manager.getCameraCharacteristics(id)
        val facing = characteristics.get(CameraCharacteristics.LENS_FACING)
        val wantFront = lens.lowercase() == "front"
        if (wantFront && facing == CameraCharacteristics.LENS_FACING_FRONT) return id
        if (!wantFront && facing == CameraCharacteristics.LENS_FACING_BACK) return id
    }
    return manager.cameraIdList.firstOrNull()
}
