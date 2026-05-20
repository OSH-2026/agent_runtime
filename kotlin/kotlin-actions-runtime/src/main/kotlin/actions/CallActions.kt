package actions

import android.net.Uri
import android.provider.CallLog
import android.telecom.TelecomManager
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class ReadCallLogInput(val limit: Int = 50)

@Serializable
data class CallLogItem(
    val id: Long,
    val number: String,
    val type: Int,
    val dateMs: Long,
    val durationSeconds: Long,
)

@Serializable
data class ReadCallLogOutput(val calls: List<CallLogItem>)

class ReadCallLogAction : Action<ReadCallLogInput, ReadCallLogOutput> {
    override suspend fun execute(input: ReadCallLogInput, ctx: ActionContext): ReadCallLogOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.READ_CALL_LOG)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "READ_CALL_LOG permission required",
                retryable = false,
            )
        }
        val projection = arrayOf(
            CallLog.Calls._ID,
            CallLog.Calls.NUMBER,
            CallLog.Calls.TYPE,
            CallLog.Calls.DATE,
            CallLog.Calls.DURATION,
        )
        val calls = mutableListOf<CallLogItem>()
        context.contentResolver.query(
            CallLog.Calls.CONTENT_URI,
            projection,
            null,
            null,
            CallLog.Calls.DEFAULT_SORT_ORDER,
        )?.use { cursor ->
            val idIndex = cursor.getColumnIndex(CallLog.Calls._ID)
            val numberIndex = cursor.getColumnIndex(CallLog.Calls.NUMBER)
            val typeIndex = cursor.getColumnIndex(CallLog.Calls.TYPE)
            val dateIndex = cursor.getColumnIndex(CallLog.Calls.DATE)
            val durationIndex = cursor.getColumnIndex(CallLog.Calls.DURATION)
            while (cursor.moveToNext() && calls.size < input.limit) {
                calls.add(
                    CallLogItem(
                        id = cursor.getLong(idIndex),
                        number = cursor.getString(numberIndex) ?: "",
                        type = cursor.getInt(typeIndex),
                        dateMs = cursor.getLong(dateIndex),
                        durationSeconds = cursor.getLong(durationIndex),
                    ),
                )
            }
        }
        return ReadCallLogOutput(calls)
    }
}

@Serializable
data class PlaceCallInput(val phoneNumber: String)

@Serializable
data class PlaceCallOutput(val placed: Boolean)

class PlaceCallAction : Action<PlaceCallInput, PlaceCallOutput> {
    override suspend fun execute(input: PlaceCallInput, ctx: ActionContext): PlaceCallOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.CALL_PHONE)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "CALL_PHONE permission required",
                retryable = false,
            )
        }
        val telecomManager = context.getSystemService(TelecomManager::class.java)
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "TelecomManager unavailable",
                retryable = true,
            )
        val uri = Uri.fromParts("tel", input.phoneNumber, null)
        try {
            telecomManager.placeCall(uri, null)
        } catch (ex: SecurityException) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "Place call not permitted",
                retryable = false,
                cause = ex,
            )
        }
        return PlaceCallOutput(placed = true)
    }
}
