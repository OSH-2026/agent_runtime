package actions

import android.content.Intent
import android.net.Uri
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class EditContactInput(val contactUri: String)

class EditContactAction : Action<EditContactInput, LaunchResult> {
    override suspend fun execute(input: EditContactInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_EDIT, Uri.parse(input.contactUri))
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
