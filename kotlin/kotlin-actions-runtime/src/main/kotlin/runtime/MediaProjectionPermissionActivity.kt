package runtime

import android.app.Activity
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts

class MediaProjectionPermissionActivity : ComponentActivity() {
    private var pendingGrant: MediaProjectionGrant? = null

    private val launcher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        finishWithGrant(result.resultCode, result.data)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val manager = getSystemService(MediaProjectionManager::class.java)
        if (manager == null) {
            finishWithGrant(Activity.RESULT_CANCELED, null)
            return
        }
        launcher.launch(manager.createScreenCaptureIntent())
    }

    override fun onDestroy() {
        val grant = pendingGrant
        pendingGrant = null
        super.onDestroy()
        if (grant != null) {
            MediaProjectionCoordinator.complete(grant.resultCode, grant.data)
        }
    }

    private fun finishWithGrant(resultCode: Int, data: Intent?) {
        pendingGrant = MediaProjectionGrant(resultCode, data)
        finishAndRemoveTask()
        overridePendingTransition(0, 0)
    }
}
