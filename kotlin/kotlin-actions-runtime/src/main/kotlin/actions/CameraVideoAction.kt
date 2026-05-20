package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class CameraVideoInput(
    val durationSeconds: Int = 10,
    val lens: String = "back",
    val withAudio: Boolean = false,
)

class CameraVideoAction : Action<CameraVideoInput, RecordVideoOutput> {
    override suspend fun execute(input: CameraVideoInput, ctx: ActionContext): RecordVideoOutput {
        return RecordVideoAction().execute(
            RecordVideoInput(
                durationSeconds = input.durationSeconds,
                lens = input.lens,
                withAudio = input.withAudio,
            ),
            ctx,
        )
    }
}
