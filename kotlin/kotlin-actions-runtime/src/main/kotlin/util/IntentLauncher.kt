package util

import actions.LaunchResult
import android.content.Context
import android.content.Intent
import error.ActionException
import error.ErrorCode

object IntentLauncher {
    fun launch(context: Context, intent: Intent): LaunchResult {
        val pm = context.packageManager
        val resolvedPackage = resolveActivityPackage(pm, intent)
            ?: throw ActionException(
                code = ErrorCode.UNAVAILABLE,
                message = "No app can handle intent: ${intent.action}",
                retryable = false,
            )
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(intent)
        return LaunchResult(
            launched = true,
            resolvedPackage = resolvedPackage,
            message = "Launched ${intent.action}",
        )
    }
}
