package api

import android.content.Context

data class ActionContext(
    val appContext: Context,
    val requestId: String,
    val nodeId: String,
    val deadline: Long,
    val metadata: Map<String, String> = emptyMap(),
)
