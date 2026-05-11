package example

import android.Manifest
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import api.ActionRequest
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.serializer
import actions.DeviceInfoInput
import actions.DeviceInfoOutput
import actions.ForegroundAppInput
import actions.ForegroundAppOutput
import actions.HttpRequest
import actions.HttpResponse
import actions.LocationInput
import actions.LocationOutput
import actions.NetworkStatusInput
import actions.NetworkStatusOutput
import actions.PermissionStatusInput
import actions.PermissionStatusOutput
import actions.PowerStatusInput
import actions.PowerStatusOutput
import actions.ReadFileInput
import actions.ReadFileOutput
import actions.StorageInfoInput
import actions.StorageInfoOutput
import example.runtime.R
import runtime.ActionExecutor
import runtime.ActionRuntimeService
import transport.serialization.JsonCodec
import java.io.File

class MainActivity : ComponentActivity() {

    private val codec = JsonCodec()
    private val registry by lazy { buildSmokeActionRegistry() }
    private val executor by lazy { ActionExecutor(applicationContext, registry, codec) }

    private lateinit var resultText: TextView
    private lateinit var smokeTestFile: File

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) { grants ->
        appendLog("permission result: $grants")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        smokeTestFile = File(filesDir, "smoke-test.txt")
        if (!smokeTestFile.exists()) {
            smokeTestFile.writeText("hello from smoke test")
        }

        resultText = findViewById(R.id.resultText)

        findViewById<Button>(R.id.btnRequestPermissions).setOnClickListener {
            val permissions = buildList {
                add(Manifest.permission.ACCESS_FINE_LOCATION)
                add(Manifest.permission.ACCESS_COARSE_LOCATION)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    add(Manifest.permission.POST_NOTIFICATIONS)
                }
            }.toTypedArray()
            permissionLauncher.launch(permissions)
        }

        findViewById<Button>(R.id.btnStartService).setOnClickListener {
            ActionRuntimeService.start(applicationContext)
            appendLog("started ActionRuntimeService (gRPC)")
        }

        findViewById<Button>(R.id.btnStopService).setOnClickListener {
            ActionRuntimeService.stop(applicationContext)
            appendLog("stopped ActionRuntimeService")
        }

        findViewById<Button>(R.id.btnDeviceInfo).setOnClickListener {
            runSmoke("device_info") {
                val input = DeviceInfoInput(includeHardware = true)
                val payload = codec.encode(input, serializer<DeviceInfoInput>())
                val req = request("device_info", payload)
                val resp = executor.execute(req)
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<DeviceInfoOutput>())
                    "OK device_info: brand=${out.brand} model=${out.model} sdk=${out.sdkInt}"
                } else {
                    "FAIL device_info: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnNetworkStatus).setOnClickListener {
            runSmoke("network_status") {
                val input = NetworkStatusInput()
                val payload = codec.encode(input, serializer<NetworkStatusInput>())
                val resp = executor.execute(request("network_status", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<NetworkStatusOutput>())
                    "OK network_status: connected=${out.connected} transports=${out.transports}"
                } else {
                    "FAIL network_status: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnPowerStatus).setOnClickListener {
            runSmoke("power_status") {
                val input = PowerStatusInput()
                val payload = codec.encode(input, serializer<PowerStatusInput>())
                val resp = executor.execute(request("power_status", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<PowerStatusOutput>())
                    "OK power_status: pct=${out.batteryPercent} charging=${out.charging}"
                } else {
                    "FAIL power_status: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnStorageInfo).setOnClickListener {
            runSmoke("storage_info") {
                val input = StorageInfoInput()
                val payload = codec.encode(input, serializer<StorageInfoInput>())
                val resp = executor.execute(request("storage_info", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<StorageInfoOutput>())
                    "OK storage_info: internalAvail=${out.internalAvailableBytes} internalTotal=${out.internalTotalBytes}"
                } else {
                    "FAIL storage_info: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnLocation).setOnClickListener {
            runSmoke("get_location") {
                val input = LocationInput()
                val payload = codec.encode(input, serializer<LocationInput>())
                val resp = executor.execute(request("get_location", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<LocationOutput>())
                    "OK get_location: lat=${out.latitude} lon=${out.longitude} provider=${out.provider}"
                } else {
                    "FAIL get_location: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnForegroundApp).setOnClickListener {
            runSmoke("foreground_app") {
                val input = ForegroundAppInput()
                val payload = codec.encode(input, serializer<ForegroundAppInput>())
                val resp = executor.execute(request("foreground_app", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<ForegroundAppOutput>())
                    "OK foreground_app: pkg=${out.packageName} available=${out.available}"
                } else {
                    "FAIL foreground_app: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnCheckPermissions).setOnClickListener {
            runSmoke("check_permissions") {
                val input = PermissionStatusInput(
                    permissions = listOf(
                        Manifest.permission.ACCESS_FINE_LOCATION,
                        Manifest.permission.POST_NOTIFICATIONS,
                    ),
                )
                val payload = codec.encode(input, serializer<PermissionStatusInput>())
                val resp = executor.execute(request("check_permissions", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<PermissionStatusOutput>())
                    "OK check_permissions: ${out.granted}"
                } else {
                    "FAIL check_permissions: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnReadFile).setOnClickListener {
            runSmoke("read_file") {
                val input = ReadFileInput(path = smokeTestFile.absolutePath)
                val payload = codec.encode(input, serializer<ReadFileInput>())
                val resp = executor.execute(request("read_file", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<ReadFileOutput>())
                    "OK read_file: ${out.content}"
                } else {
                    "FAIL read_file: ${resp.error}"
                }
            }
        }

        findViewById<Button>(R.id.btnHttpCall).setOnClickListener {
            runSmoke("http_call") {
                val input = HttpRequest(url = "https://example.com")
                val payload = codec.encode(input, serializer<HttpRequest>())
                val resp = executor.execute(request("http_call", payload))
                if (resp.success && resp.result != null) {
                    val out = codec.decode(resp.result!!, serializer<HttpResponse>())
                    val preview = out.body.take(120).replace("\n", " ")
                    "OK http_call: status=${out.status} bodyPreview=$preview"
                } else {
                    "FAIL http_call: ${resp.error}"
                }
            }
        }
    }

    private fun request(actionName: String, payload: ByteArray): ActionRequest {
        return ActionRequest(
            actionName = actionName,
            payload = payload,
            metadata = mapOf(
                "requestId" to "smoke-${System.currentTimeMillis()}",
                "nodeId" to "demo",
            ),
        )
    }

    private fun runSmoke(label: String, block: suspend () -> String) {
        lifecycleScope.launch {
            appendLog("--- $label ---")
            val line = try {
                withContext(Dispatchers.Default) {
                    block()
                }
            } catch (e: Exception) {
                "EXCEPTION $label: ${e.message}"
            }
            appendLog(line)
        }
    }

    private fun appendLog(line: String) {
        resultText.append(line)
        resultText.append("\n\n")
    }
}
