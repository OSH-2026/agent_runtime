package actions

import android.content.Context
import android.media.AudioManager
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class SetVolumeInput(
    val stream: String = "music",
    val level: Int,
    val showUi: Boolean = false,
)

@Serializable
data class SetVolumeOutput(
    val stream: String,
    val level: Int,
)

class SetVolumeAction : Action<SetVolumeInput, SetVolumeOutput> {
    override suspend fun execute(input: SetVolumeInput, ctx: ActionContext): SetVolumeOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.MODIFY_AUDIO_SETTINGS)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "MODIFY_AUDIO_SETTINGS permission required",
                retryable = false,
            )
        }
        val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "AudioManager unavailable",
                retryable = true,
            )
        val streamType = streamFromString(input.stream)
        val max = audioManager.getStreamMaxVolume(streamType)
        val level = input.level.coerceIn(0, max)
        val flags = if (input.showUi) AudioManager.FLAG_SHOW_UI else 0
        audioManager.setStreamVolume(streamType, level, flags)
        return SetVolumeOutput(stream = input.stream, level = level)
    }
}

@Serializable
data class SetSilentModeInput(val mode: String = "silent")

@Serializable
data class SetSilentModeOutput(val mode: String)

class SetSilentModeAction : Action<SetSilentModeInput, SetSilentModeOutput> {
    override suspend fun execute(input: SetSilentModeInput, ctx: ActionContext): SetSilentModeOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.MODIFY_AUDIO_SETTINGS)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "MODIFY_AUDIO_SETTINGS permission required",
                retryable = false,
            )
        }
        val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "AudioManager unavailable",
                retryable = true,
            )
        val mode = when (input.mode.lowercase()) {
            "vibrate" -> AudioManager.RINGER_MODE_VIBRATE
            "normal" -> AudioManager.RINGER_MODE_NORMAL
            else -> AudioManager.RINGER_MODE_SILENT
        }
        audioManager.ringerMode = mode
        val modeName = when (audioManager.ringerMode) {
            AudioManager.RINGER_MODE_VIBRATE -> "vibrate"
            AudioManager.RINGER_MODE_NORMAL -> "normal"
            else -> "silent"
        }
        return SetSilentModeOutput(mode = modeName)
    }
}

private fun streamFromString(value: String): Int {
    return when (value.lowercase()) {
        "alarm" -> AudioManager.STREAM_ALARM
        "ring" -> AudioManager.STREAM_RING
        "notification" -> AudioManager.STREAM_NOTIFICATION
        "call" -> AudioManager.STREAM_VOICE_CALL
        else -> AudioManager.STREAM_MUSIC
    }
}
