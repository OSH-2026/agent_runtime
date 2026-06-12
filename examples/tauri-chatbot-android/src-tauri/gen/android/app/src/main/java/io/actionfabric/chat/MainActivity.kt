package io.actionfabric.chat

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import runtime.ActionRuntimeService

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    ActionRuntimeService.start(applicationContext)
  }
}
