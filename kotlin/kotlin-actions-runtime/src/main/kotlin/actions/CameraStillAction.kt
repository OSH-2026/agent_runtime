package actions

import android.content.Intent
import android.provider.MediaStore
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class CameraStillInput(val unused: Boolean = true)

class CameraStillAction : Action<CameraStillInput, LaunchResult> {
    override suspend fun execute(input: CameraStillInput, ctx: ActionContext): LaunchResult {
        return IntentLauncher.launch(ctx.appContext, Intent(MediaStore.ACTION_IMAGE_CAPTURE))
    }
}
