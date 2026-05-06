package actions

import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class LlmInput(val prompt: String)

@Serializable
data class LlmOutput(val completion: String)

class LlmAction : Action<LlmInput, LlmOutput> {
    override suspend fun execute(input: LlmInput, ctx: ActionContext): LlmOutput {
        return LlmOutput("not implemented")
    }
}
