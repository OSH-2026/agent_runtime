package actions

import android.provider.ContactsContract
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class SearchContactsInput(
    val query: String,
    val limit: Int = 20,
)

@Serializable
data class ContactResult(
    val contactId: Long,
    val displayName: String,
    val phoneNumber: String,
)

@Serializable
data class SearchContactsOutput(val results: List<ContactResult>)

class SearchContactsAction : Action<SearchContactsInput, SearchContactsOutput> {
    override suspend fun execute(input: SearchContactsInput, ctx: ActionContext): SearchContactsOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.READ_CONTACTS)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "READ_CONTACTS permission required",
                retryable = false,
            )
        }
        val selection = "${ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME} LIKE ? OR ${ContactsContract.CommonDataKinds.Phone.NUMBER} LIKE ?"
        val arg = "%${input.query}%"
        val projection = arrayOf(
            ContactsContract.CommonDataKinds.Phone.CONTACT_ID,
            ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME,
            ContactsContract.CommonDataKinds.Phone.NUMBER,
        )
        val results = mutableListOf<ContactResult>()
        context.contentResolver.query(
            ContactsContract.CommonDataKinds.Phone.CONTENT_URI,
            projection,
            selection,
            arrayOf(arg, arg),
            ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME + " ASC",
        )?.use { cursor ->
            val idIndex = cursor.getColumnIndex(ContactsContract.CommonDataKinds.Phone.CONTACT_ID)
            val nameIndex = cursor.getColumnIndex(ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME)
            val numberIndex = cursor.getColumnIndex(ContactsContract.CommonDataKinds.Phone.NUMBER)
            while (cursor.moveToNext() && results.size < input.limit) {
                results.add(
                    ContactResult(
                        contactId = cursor.getLong(idIndex),
                        displayName = cursor.getString(nameIndex) ?: "",
                        phoneNumber = cursor.getString(numberIndex) ?: "",
                    ),
                )
            }
        }
        return SearchContactsOutput(results)
    }
}
