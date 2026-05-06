package api

import error.ActionError

sealed class ActionResult {
    data class Success(val data: ByteArray) : ActionResult()
    data class Failure(val error: ActionError) : ActionResult()
}
