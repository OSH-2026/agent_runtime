package actions

import android.location.Location
import android.location.LocationManager
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class LocationInput(val allowStaleMs: Long = 10 * 60 * 1000)

@Serializable
data class LocationOutput(
    val latitude: Double,
    val longitude: Double,
    val accuracyMeters: Float,
    val provider: String,
    val timestampMs: Long,
)

class LocationAction : Action<LocationInput, LocationOutput> {
    @Suppress("MissingPermission")
    override suspend fun execute(input: LocationInput, ctx: ActionContext): LocationOutput {
        val context = ctx.appContext
        val permissions = listOf(
            android.Manifest.permission.ACCESS_FINE_LOCATION,
            android.Manifest.permission.ACCESS_COARSE_LOCATION,
        )
        if (!PermissionUtils.anyGranted(context, permissions)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "Location permission required",
                retryable = false,
            )
        }
        val manager = context.getSystemService(LocationManager::class.java)
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "LocationManager unavailable",
                retryable = true,
            )
        val candidates = listOfNotNull(
            manager.getLastKnownLocation(LocationManager.GPS_PROVIDER),
            manager.getLastKnownLocation(LocationManager.NETWORK_PROVIDER),
        )
        val now = System.currentTimeMillis()
        val location = candidates
            .filter { now - it.time <= input.allowStaleMs }
            .minByOrNull { it.accuracy }
            ?: candidates.maxByOrNull { it.time }
            ?: throw ActionException(
                code = ErrorCode.VALIDATION,
                message = "No location available",
                retryable = true,
            )

        return location.toOutput()
    }
}

private fun Location.toOutput(): LocationOutput {
    return LocationOutput(
        latitude = latitude,
        longitude = longitude,
        accuracyMeters = accuracy,
        provider = provider ?: "",
        timestampMs = time,
    )
}
