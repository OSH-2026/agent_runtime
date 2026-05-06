package api

import error.ActionError

data class ActionResponse(
    val success: Boolean,
    val result: ByteArray? = null,
    val error: ActionError? = null,
)
