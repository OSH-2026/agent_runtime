package actions

import android.content.Intent
import android.provider.MediaStore
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class PlayMediaSearchInput(
    val query: String,
    val mediaFocus: String = "vnd.android.cursor.item/*",
)

class PlayMediaSearchAction : Action<PlayMediaSearchInput, LaunchResult> {
    override suspend fun execute(input: PlayMediaSearchInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(MediaStore.INTENT_ACTION_MEDIA_PLAY_FROM_SEARCH).apply {
            putExtra(MediaStore.EXTRA_MEDIA_FOCUS, input.mediaFocus)
            putExtra(Intent.EXTRA_TEXT, input.query)
            putExtra("query", input.query)
        }
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
