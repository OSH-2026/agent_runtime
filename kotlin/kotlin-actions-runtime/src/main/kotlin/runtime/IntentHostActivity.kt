package runtime

import actions.IntentActivityResult
import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts

class IntentHostActivity : ComponentActivity() {
    private val forwardLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        val resolvedPackage = intent.getStringExtra(EXTRA_RESOLVED_PACKAGE)
        IntentHostCoordinator.complete(
            IntentActivityResult(
                launched = true,
                resultCode = result.resultCode,
                dataUri = result.data?.data?.toString(),
                resolvedPackage = resolvedPackage,
                message = when (result.resultCode) {
                    Activity.RESULT_OK -> "Intent completed"
                    Activity.RESULT_CANCELED -> "Intent canceled by user"
                    else -> "Intent finished with code ${result.resultCode}"
                },
            ),
        )
        finish()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val forward = readForwardIntent()
        if (forward == null) {
            IntentHostCoordinator.fail("Missing forward intent")
            finish()
            return
        }
        forwardLauncher.launch(forward)
    }

    private fun readForwardIntent(): Intent? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(EXTRA_FORWARD_INTENT, Intent::class.java)
        } else {
            @Suppress("DEPRECATION")
            intent.getParcelableExtra(EXTRA_FORWARD_INTENT)
        }
    }

    companion object {
        const val EXTRA_FORWARD_INTENT = "runtime.intent.forward"
        const val EXTRA_RESOLVED_PACKAGE = "runtime.intent.resolved_package"
    }
}
