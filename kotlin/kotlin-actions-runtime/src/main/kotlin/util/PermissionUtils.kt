package util

import android.content.Context
import androidx.core.content.ContextCompat

object PermissionUtils {
    fun hasPermission(context: Context, permission: String): Boolean {
        return ContextCompat.checkSelfPermission(
            context,
            permission,
        ) == android.content.pm.PackageManager.PERMISSION_GRANTED
    }

    fun anyGranted(context: Context, permissions: List<String>): Boolean {
        return permissions.any { hasPermission(context, it) }
    }
}
