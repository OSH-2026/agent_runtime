package actions

import android.content.Intent
import android.net.Uri
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class ComposeEmailInput(
    val to: String,
    val subject: String = "",
    val body: String = "",
)

class ComposeEmailAction : Action<ComposeEmailInput, LaunchResult> {
    override suspend fun execute(input: ComposeEmailInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_SENDTO).apply {
            data = Uri.parse("mailto:${Uri.encode(input.to)}")
            putExtra(Intent.EXTRA_SUBJECT, input.subject)
            putExtra(Intent.EXTRA_TEXT, input.body)
        }
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
