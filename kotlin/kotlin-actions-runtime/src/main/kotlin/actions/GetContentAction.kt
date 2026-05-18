package actions

import android.content.Intent
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import runtime.IntentHostCoordinator

@Serializable
data class GetContentInput(val mimeType: String = "*/*")

class GetContentAction : Action<GetContentInput, IntentActivityResult> {
    override suspend fun execute(input: GetContentInput, ctx: ActionContext): IntentActivityResult {
        val intent = Intent(Intent.ACTION_GET_CONTENT).apply {
            type = input.mimeType
            addCategory(Intent.CATEGORY_OPENABLE)
        }
        return IntentHostCoordinator.startForResult(ctx.appContext, intent)
    }
}
