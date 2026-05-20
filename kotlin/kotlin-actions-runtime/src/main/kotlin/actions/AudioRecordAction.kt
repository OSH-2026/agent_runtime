package actions

import android.media.MediaRecorder
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class RecordAudioInput(
    val durationSeconds: Int = 10,
)

@Serializable
data class RecordAudioOutput(
    val path: String,
    val durationSeconds: Int,
)

class RecordAudioAction : Action<RecordAudioInput, RecordAudioOutput> {
    override suspend fun execute(input: RecordAudioInput, ctx: ActionContext): RecordAudioOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.RECORD_AUDIO)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "RECORD_AUDIO permission required",
                retryable = false,
            )
        }
        return withContext(Dispatchers.IO) {
            val output = File(context.cacheDir, "audio_${System.currentTimeMillis()}.m4a")
            val recorder = MediaRecorder()
            recorder.setAudioSource(MediaRecorder.AudioSource.MIC)
            recorder.setOutputFormat(MediaRecorder.OutputFormat.MPEG_4)
            recorder.setOutputFile(output.absolutePath)
            recorder.setAudioEncoder(MediaRecorder.AudioEncoder.AAC)
            recorder.setAudioEncodingBitRate(128_000)
            recorder.setAudioSamplingRate(44_100)
            recorder.prepare()
            recorder.start()
            delay(input.durationSeconds * 1000L)
            recorder.stop()
            recorder.reset()
            recorder.release()
            RecordAudioOutput(path = output.absolutePath, durationSeconds = input.durationSeconds)
        }
    }
}
