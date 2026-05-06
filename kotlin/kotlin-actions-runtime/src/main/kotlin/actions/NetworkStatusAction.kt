package actions

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import util.PermissionUtils

@Serializable
data class NetworkStatusInput(val includeDetails: Boolean = true)

@Serializable
data class NetworkStatusOutput(
    val connected: Boolean,
    val transports: List<String>,
    val metered: Boolean,
    val downstreamKbps: Int,
    val upstreamKbps: Int,
)

class NetworkStatusAction : Action<NetworkStatusInput, NetworkStatusOutput> {
    override suspend fun execute(input: NetworkStatusInput, ctx: ActionContext): NetworkStatusOutput {
        val context = ctx.appContext
        if (!PermissionUtils.hasPermission(context, android.Manifest.permission.ACCESS_NETWORK_STATE)) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "ACCESS_NETWORK_STATE permission required",
                retryable = false,
            )
        }
        val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val network = cm.activeNetwork
        val caps = network?.let { cm.getNetworkCapabilities(it) }
        if (caps == null) {
            return NetworkStatusOutput(
                connected = false,
                transports = emptyList(),
                metered = cm.isActiveNetworkMetered,
                downstreamKbps = 0,
                upstreamKbps = 0,
            )
        }
        val transports = buildList {
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) add("WIFI")
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR)) add("CELLULAR")
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)) add("ETHERNET")
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_VPN)) add("VPN")
            if (caps.hasTransport(NetworkCapabilities.TRANSPORT_BLUETOOTH)) add("BLUETOOTH")
        }
        return NetworkStatusOutput(
            connected = caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET),
            transports = transports,
            metered = cm.isActiveNetworkMetered,
            downstreamKbps = caps.linkDownstreamBandwidthKbps,
            upstreamKbps = caps.linkUpstreamBandwidthKbps,
        )
    }
}
