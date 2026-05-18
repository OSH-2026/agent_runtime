package actions

import kotlinx.serialization.Serializable

@Serializable
data class LaunchResult(
    val launched: Boolean,
    val resolvedPackage: String? = null,
    val message: String = "",
)

@Serializable
data class IntentActivityResult(
    val launched: Boolean,
    val resultCode: Int = 0,
    val dataUri: String? = null,
    val resolvedPackage: String? = null,
    val message: String = "",
)
