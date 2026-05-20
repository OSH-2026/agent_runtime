package runtime

import android.app.Activity
import android.content.Intent
import android.media.projection.MediaProjectionManager
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts

class MediaProjectionPermissionActivity : ComponentActivity() {
    private val launcher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        MediaProjectionCoordinator.complete(result.resultCode, result.data)
        finish()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val manager = getSystemService(MediaProjectionManager::class.java)
        if (manager == null) {
            MediaProjectionCoordinator.complete(Activity.RESULT_CANCELED, null)
            finish()
            return
        }
        launcher.launch(manager.createScreenCaptureIntent())
    }

    override fun onDestroy() {
        super.onDestroy()
        if (isFinishing) {
            return
        }
    }

    companion object {
        const val RESULT_CANCELED = Activity.RESULT_CANCELED
    }
}
