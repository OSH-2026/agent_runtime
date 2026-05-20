package actions

import android.telephony.SmsManager
import android.provider.Telephony
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class ReadSmsInput(
    val box: String = "inbox",
    val limit: Int = 50,
)

@Serializable
data class SmsMessageItem(
    val id: Long,
    val address: String,
    val body: String,
    val dateMs: Long,
    val read: Boolean,
    val type: Int,
)

@Serializable
data class ReadSmsOutput(val messages: List<SmsMessageItem>)

class ReadSmsAction : Action<ReadSmsInput, ReadSmsOutput> {
    override suspend fun execute(input: ReadSmsInput, ctx: ActionContext): ReadSmsOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.READ_SMS)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "READ_SMS permission required",
                retryable = false,
            )
        }
        val uri = when (input.box.lowercase()) {
            "sent" -> Telephony.Sms.Sent.CONTENT_URI
            "draft" -> Telephony.Sms.Draft.CONTENT_URI
            "outbox" -> Telephony.Sms.Outbox.CONTENT_URI
            "all" -> Telephony.Sms.CONTENT_URI
            else -> Telephony.Sms.Inbox.CONTENT_URI
        }
        val projection = arrayOf(
            Telephony.Sms._ID,
            Telephony.Sms.ADDRESS,
            Telephony.Sms.BODY,
            Telephony.Sms.DATE,
            Telephony.Sms.READ,
            Telephony.Sms.TYPE,
        )
        val messages = mutableListOf<SmsMessageItem>()
        context.contentResolver.query(
            uri,
            projection,
            null,
            null,
            Telephony.Sms.DEFAULT_SORT_ORDER,
        )?.use { cursor ->
            val idIndex = cursor.getColumnIndex(Telephony.Sms._ID)
            val addressIndex = cursor.getColumnIndex(Telephony.Sms.ADDRESS)
            val bodyIndex = cursor.getColumnIndex(Telephony.Sms.BODY)
            val dateIndex = cursor.getColumnIndex(Telephony.Sms.DATE)
            val readIndex = cursor.getColumnIndex(Telephony.Sms.READ)
            val typeIndex = cursor.getColumnIndex(Telephony.Sms.TYPE)
            while (cursor.moveToNext() && messages.size < input.limit) {
                messages.add(
                    SmsMessageItem(
                        id = cursor.getLong(idIndex),
                        address = cursor.getString(addressIndex) ?: "",
                        body = cursor.getString(bodyIndex) ?: "",
                        dateMs = cursor.getLong(dateIndex),
                        read = cursor.getInt(readIndex) == 1,
                        type = cursor.getInt(typeIndex),
                    ),
                )
            }
        }
        return ReadSmsOutput(messages)
    }
}

@Serializable
data class SendSmsInput(
    val address: String,
    val body: String,
)

@Serializable
data class SendSmsOutput(val sent: Boolean)

class SendSmsAction : Action<SendSmsInput, SendSmsOutput> {
    override suspend fun execute(input: SendSmsInput, ctx: ActionContext): SendSmsOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.SEND_SMS)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "SEND_SMS permission required",
                retryable = false,
            )
        }
        val smsManager = SmsManager.getDefault()
        smsManager.sendTextMessage(input.address, null, input.body, null, null)
        return SendSmsOutput(sent = true)
    }
}
