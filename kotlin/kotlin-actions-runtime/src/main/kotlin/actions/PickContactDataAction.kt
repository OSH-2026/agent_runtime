package actions

import android.content.Intent
import android.provider.ContactsContract
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import runtime.IntentHostCoordinator

@Serializable
data class PickContactDataInput(val mimeType: String = ContactsContract.CommonDataKinds.Phone.CONTENT_TYPE)

class PickContactDataAction : Action<PickContactDataInput, IntentActivityResult> {
    override suspend fun execute(input: PickContactDataInput, ctx: ActionContext): IntentActivityResult {
        val intent = Intent(Intent.ACTION_PICK).apply {
            type = input.mimeType
        }
        return IntentHostCoordinator.startForResult(ctx.appContext, intent)
    }
}
