package actions

import android.content.Intent
import android.provider.ContactsContract
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import util.IntentLauncher

@Serializable
data class InsertContactInput(val unused: Boolean = true)

class InsertContactAction : Action<InsertContactInput, LaunchResult> {
    override suspend fun execute(input: InsertContactInput, ctx: ActionContext): LaunchResult {
        val intent = Intent(Intent.ACTION_INSERT, ContactsContract.Contacts.CONTENT_URI)
        return IntentLauncher.launch(ctx.appContext, intent)
    }
}
