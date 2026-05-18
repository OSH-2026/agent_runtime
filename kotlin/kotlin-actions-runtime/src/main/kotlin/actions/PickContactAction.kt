package actions

import android.content.Intent
import android.provider.ContactsContract
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import runtime.IntentHostCoordinator

@Serializable
data class PickContactInput(val unused: Boolean = true)

class PickContactAction : Action<PickContactInput, IntentActivityResult> {
    override suspend fun execute(input: PickContactInput, ctx: ActionContext): IntentActivityResult {
        val intent = Intent(Intent.ACTION_PICK, ContactsContract.Contacts.CONTENT_URI)
        return IntentHostCoordinator.startForResult(ctx.appContext, intent)
    }
}
