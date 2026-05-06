package actions

import android.os.Build
import api.Action
import api.ActionContext
import kotlinx.serialization.Serializable
import java.util.TimeZone

@Serializable
data class DeviceInfoInput(val includeHardware: Boolean = true)

@Serializable
data class DeviceInfoOutput(
    val brand: String,
    val model: String,
    val manufacturer: String,
    val device: String,
    val product: String,
    val sdkInt: Int,
    val release: String,
    val securityPatch: String,
    val supportedAbis: List<String>,
    val screenWidthPx: Int,
    val screenHeightPx: Int,
    val densityDpi: Int,
    val locale: String,
    val timeZone: String,
)

class DeviceInfoAction : Action<DeviceInfoInput, DeviceInfoOutput> {
    override suspend fun execute(input: DeviceInfoInput, ctx: ActionContext): DeviceInfoOutput {
        val metrics = ctx.appContext.resources.displayMetrics
        val locales = ctx.appContext.resources.configuration.locales
        val localeTag = if (!locales.isEmpty) locales[0].toLanguageTag() else ""

        return DeviceInfoOutput(
            brand = Build.BRAND ?: "",
            model = Build.MODEL ?: "",
            manufacturer = Build.MANUFACTURER ?: "",
            device = Build.DEVICE ?: "",
            product = Build.PRODUCT ?: "",
            sdkInt = Build.VERSION.SDK_INT,
            release = Build.VERSION.RELEASE ?: "",
            securityPatch = Build.VERSION.SECURITY_PATCH ?: "",
            supportedAbis = Build.SUPPORTED_ABIS?.toList() ?: emptyList(),
            screenWidthPx = metrics.widthPixels,
            screenHeightPx = metrics.heightPixels,
            densityDpi = metrics.densityDpi,
            locale = localeTag,
            timeZone = TimeZone.getDefault().id,
        )
    }
}
