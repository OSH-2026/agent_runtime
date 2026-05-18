package actions

import android.content.Intent
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class CreateNoteInput(
    val title: String = "",
    val text: String = "",
)

class CreateNoteAction : Action<CreateNoteInput, LaunchResult> {
    override suspend fun execute(input: CreateNoteInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(ACTION_CREATE_NOTE).apply {
            type = "text/plain"
            putExtra(EXTRA_NOTE_NAME, input.title)
            putExtra(EXTRA_NOTE_TEXT, input.text)
        }
        return IntentLauncher.launch(ctx.appContext, intent)
    }

    companion object {
        private const val ACTION_CREATE_NOTE = "com.google.android.gms.actions.CREATE_NOTE"
        private const val EXTRA_NOTE_NAME = "com.google.android.gms.actions.extra.NAME"
        private const val EXTRA_NOTE_TEXT = "com.google.android.gms.actions.extra.TEXT"
    }
}
