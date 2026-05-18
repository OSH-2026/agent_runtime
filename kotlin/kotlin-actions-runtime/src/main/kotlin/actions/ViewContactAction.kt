package actions

import android.content.Intent
import android.net.Uri
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class ViewContactInput(val contactUri: String)

class ViewContactAction : Action<ViewContactInput, LaunchResult> {
    override suspend fun execute(input: ViewContactInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(input.contactUri))
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
