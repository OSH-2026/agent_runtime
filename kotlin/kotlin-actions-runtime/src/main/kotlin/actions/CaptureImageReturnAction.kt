package actions

import android.content.Intent
import android.provider.MediaStore
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import runtime.IntentHostCoordinator

@Serializable
data class CaptureImageReturnInput(val unused: Boolean = true)

class CaptureImageReturnAction : Action<CaptureImageReturnInput, IntentActivityResult> {
    override suspend fun execute(input: CaptureImageReturnInput, ctx: ActionContext): IntentActivityResult {
        return IntentHostCoordinator.startForResult(
            ctx.appContext,
            Intent(MediaStore.ACTION_IMAGE_CAPTURE),
        )
    }
}
