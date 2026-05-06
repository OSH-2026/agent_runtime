package actions

import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.PowerManager
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable

@Serializable
data class PowerStatusInput(val includeDetails: Boolean = true)

@Serializable
data class PowerStatusOutput(
    val batteryPercent: Int,
    val charging: Boolean,
    val powerSaveMode: Boolean,
    val plugged: String,
)

class PowerStatusAction : Action<PowerStatusInput, PowerStatusOutput> {
    override suspend fun execute(input: PowerStatusInput, ctx: ActionContext): PowerStatusOutput {
        val context = ctx.appContext
        val batteryManager = context.getSystemService(BatteryManager::class.java)
            ?: return PowerStatusOutput(0, false, false, "UNKNOWN")
        val powerManager = context.getSystemService(PowerManager::class.java)
            ?: return PowerStatusOutput(0, false, false, "UNKNOWN")
        val statusIntent = context.registerReceiver(
            null,
            IntentFilter(Intent.ACTION_BATTERY_CHANGED),
        )
        val status = statusIntent?.getIntExtra(BatteryManager.EXTRA_STATUS, -1) ?: -1
        val pluggedValue = statusIntent?.getIntExtra(BatteryManager.EXTRA_PLUGGED, -1) ?: -1
        val charging = status == BatteryManager.BATTERY_STATUS_CHARGING ||
            status == BatteryManager.BATTERY_STATUS_FULL
        val plugged = when (pluggedValue) {
            BatteryManager.BATTERY_PLUGGED_AC -> "AC"
            BatteryManager.BATTERY_PLUGGED_USB -> "USB"
            BatteryManager.BATTERY_PLUGGED_WIRELESS -> "WIRELESS"
            else -> "NONE"
        }

        return PowerStatusOutput(
            batteryPercent = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY),
            charging = charging,
            powerSaveMode = powerManager.isPowerSaveMode,
            plugged = plugged,
        )
    }
}
