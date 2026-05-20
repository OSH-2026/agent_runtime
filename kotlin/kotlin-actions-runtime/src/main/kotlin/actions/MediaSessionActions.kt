package actions

import android.content.ComponentName
import android.content.Context
import android.media.session.MediaSessionManager
import android.view.KeyEvent
import androidx.core.app.NotificationManagerCompat
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import runtime.ActionNotificationListenerService

@Serializable
data class MediaPlayPauseInput(val action: String = "toggle")

@Serializable
data class MediaPlayPauseOutput(val handled: Boolean)

class MediaPlayPauseAction : Action<MediaPlayPauseInput, MediaPlayPauseOutput> {
    override suspend fun execute(input: MediaPlayPauseInput, ctx: ActionContext): MediaPlayPauseOutput {
        val controller = getActiveController(ctx.appContext)
            ?: throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "No active media session",
                retryable = true,
            )
        when (input.action.lowercase()) {
            "play" -> controller.transportControls.play()
            "pause" -> controller.transportControls.pause()
            else -> controller.dispatchMediaButtonEvent(
                KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE),
            )
        }
        return MediaPlayPauseOutput(handled = true)
    }
}

@Serializable
data class NowPlayingInput(val unused: Boolean = true)

@Serializable
data class NowPlayingOutput(
    val title: String,
    val artist: String,
    val album: String,
    val packageName: String,
    val isPlaying: Boolean,
)

class NowPlayingAction : Action<NowPlayingInput, NowPlayingOutput> {
    override suspend fun execute(input: NowPlayingInput, ctx: ActionContext): NowPlayingOutput {
        val controller = getActiveController(ctx.appContext)
            ?: return NowPlayingOutput("", "", "", "", false)
        val metadata = controller.metadata
        val state = controller.playbackState
        return NowPlayingOutput(
            title = metadata?.getString(android.media.MediaMetadata.METADATA_KEY_TITLE) ?: "",
            artist = metadata?.getString(android.media.MediaMetadata.METADATA_KEY_ARTIST) ?: "",
            album = metadata?.getString(android.media.MediaMetadata.METADATA_KEY_ALBUM) ?: "",
            packageName = controller.packageName ?: "",
            isPlaying = state?.state == android.media.session.PlaybackState.STATE_PLAYING,
        )
    }
}

private fun getActiveController(context: Context): android.media.session.MediaController? {
    val enabled = NotificationManagerCompat.getEnabledListenerPackages(context)
    if (!enabled.contains(context.packageName)) {
        return null
    }
    val manager = context.getSystemService(MediaSessionManager::class.java) ?: return null
    val component = ComponentName(context, ActionNotificationListenerService::class.java)
    val controllers = manager.getActiveSessions(component)
    return controllers.firstOrNull()
}
