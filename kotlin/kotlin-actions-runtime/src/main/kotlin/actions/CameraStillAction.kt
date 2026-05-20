package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class CameraStillInput(val lens: String = "back")

class CameraStillAction : Action<CameraStillInput, TakePhotoOutput> {
    override suspend fun execute(input: CameraStillInput, ctx: ActionContext): TakePhotoOutput {
        return TakePhotoAction().execute(TakePhotoInput(lens = input.lens), ctx)
    }
}
