package error

data class ActionError(
    val code: ErrorCode,
    val message: String,
    val retryable: Boolean,
) {
    companion object {
        fun from(exception: Exception): ActionError {
            return when (exception) {
                is ActionException -> ActionError(
                    code = exception.code,
                    message = exception.message,
                    retryable = exception.retryable,
                )
                else -> ActionError(
                    code = ErrorCode.INTERNAL,
                    message = exception.message ?: "unknown error",
                    retryable = false,
                )
            }
        }
    }
}
