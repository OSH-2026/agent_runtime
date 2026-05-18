package actions

import android.content.Intent
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import runtime.IntentHostCoordinator

@Serializable
data class OpenDocumentInput(val mimeType: String = "*/*")

class OpenDocumentAction : Action<OpenDocumentInput, IntentActivityResult> {
    override suspend fun execute(input: OpenDocumentInput, ctx: ActionContext): IntentActivityResult {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            type = input.mimeType
            addCategory(Intent.CATEGORY_OPENABLE)
        }
        return IntentHostCoordinator.startForResult(ctx.appContext, intent)
    }
}
