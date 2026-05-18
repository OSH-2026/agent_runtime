package actions

import android.content.Intent
import android.net.Uri
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class PlayMediaInput(val contentUri: String)

class PlayMediaAction : Action<PlayMediaInput, LaunchResult> {
    override suspend fun execute(input: PlayMediaInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(Uri.parse(input.contentUri), "audio/*")
        }
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
