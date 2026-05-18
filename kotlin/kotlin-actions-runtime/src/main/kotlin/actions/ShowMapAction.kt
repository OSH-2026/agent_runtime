package actions

import android.content.Intent
import android.net.Uri
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class ShowMapInput(val geoUri: String)

class ShowMapAction : Action<ShowMapInput, LaunchResult> {
    override suspend fun execute(input: ShowMapInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(input.geoUri))
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
