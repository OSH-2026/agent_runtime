package actions

import android.content.Intent
import android.provider.MediaStore
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class CameraVideoInput(val unused: Boolean = true)

class CameraVideoAction : Action<CameraVideoInput, LaunchResult> {
    override suspend fun execute(input: CameraVideoInput, ctx: ActionContext): LaunchResult {
        return IntentLauncher.launch(ctx.appContext, Intent(MediaStore.ACTION_VIDEO_CAPTURE))
    }
}
