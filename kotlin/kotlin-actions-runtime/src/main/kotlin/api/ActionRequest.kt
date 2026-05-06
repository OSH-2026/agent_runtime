package api

data class ActionRequest(
    val actionName: String,
    val payload: ByteArray,
    val metadata: Map<String, String>,
)
