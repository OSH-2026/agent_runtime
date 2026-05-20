package actions

import android.bluetooth.BluetoothManager
import android.content.Context
import android.net.wifi.WifiManager
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class WifiToggleInput(val enabled: Boolean)

@Serializable
data class WifiToggleOutput(val enabled: Boolean)

class WifiToggleAction : Action<WifiToggleInput, WifiToggleOutput> {
    override suspend fun execute(input: WifiToggleInput, ctx: ActionContext): WifiToggleOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.CHANGE_WIFI_STATE)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "CHANGE_WIFI_STATE permission required",
                retryable = false,
            )
        }
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as? WifiManager
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "WifiManager unavailable",
                retryable = true,
            )
        val success = try {
            wifiManager.setWifiEnabled(input.enabled)
        } catch (ex: SecurityException) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "Wi-Fi toggle not permitted",
                retryable = false,
                cause = ex,
            )
        }
        if (!success && wifiManager.isWifiEnabled != input.enabled) {
            throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "Wi-Fi toggle not permitted on this device",
                retryable = false,
            )
        }
        return WifiToggleOutput(enabled = wifiManager.isWifiEnabled)
    }
}

@Serializable
data class BluetoothToggleInput(val enabled: Boolean)

@Serializable
data class BluetoothToggleOutput(val enabled: Boolean)

class BluetoothToggleAction : Action<BluetoothToggleInput, BluetoothToggleOutput> {
    override suspend fun execute(input: BluetoothToggleInput, ctx: ActionContext): BluetoothToggleOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.BLUETOOTH_CONNECT)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "BLUETOOTH_CONNECT permission required",
                retryable = false,
            )
        }
        val manager = context.getSystemService(BluetoothManager::class.java)
            ?: throw ActionException(
                code = ErrorCode.INTERNAL,
                message = "BluetoothManager unavailable",
                retryable = true,
            )
        val adapter = manager.adapter
            ?: throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "Bluetooth adapter unavailable",
                retryable = false,
            )
        val success = try {
            if (input.enabled) adapter.enable() else adapter.disable()
        } catch (ex: SecurityException) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "Bluetooth toggle not permitted",
                retryable = false,
                cause = ex,
            )
        }
        if (!success && adapter.isEnabled != input.enabled) {
            throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "Bluetooth toggle not permitted on this device",
                retryable = false,
            )
        }
        return BluetoothToggleOutput(enabled = adapter.isEnabled)
    }
}
