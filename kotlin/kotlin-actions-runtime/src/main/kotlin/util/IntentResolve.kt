package util

import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build

internal fun resolveActivityPackage(pm: PackageManager, intent: Intent): String? {
    val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        pm.resolveActivity(
            intent,
            PackageManager.ResolveInfoFlags.of(PackageManager.MATCH_DEFAULT_ONLY.toLong()),
        )
    } else {
        @Suppress("DEPRECATION")
        pm.resolveActivity(intent, PackageManager.MATCH_DEFAULT_ONLY)
    }
    return info?.activityInfo?.packageName
}
