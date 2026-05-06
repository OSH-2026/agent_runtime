package error

class ActionException(
    val code: ErrorCode,
    override val message: String,
    val retryable: Boolean,
    override val cause: Throwable? = null,
) : RuntimeException(message, cause)
